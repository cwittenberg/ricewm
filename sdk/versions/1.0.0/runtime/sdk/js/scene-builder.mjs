import { MAX_SCENE_BYTES, copySceneFrame } from './scene-surface.mjs';

const finite = value => {
    if (!Number.isFinite(value) || !Number.isFinite(Math.fround(value))) throw new RangeError('Scene coordinate must be finite f32');
    return value;
};
const color32 = value => {
    if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) throw new RangeError('Expected packed RGBA color');
    return value;
};

const coordinates = (values, count) => {
    if (!Array.isArray(values) || values.length !== count) throw new TypeError('Invalid scene coordinate array');
    return Array.from(values, finite);
};
const gradientStops = stops => {
    if (!Array.isArray(stops) || stops.length < 2 || stops.length > 64) throw new RangeError('Expected 2 to 64 gradient stops');
    let previous = -1;
    return Array.from(stops, stop => {
        if (!stop || !Number.isFinite(stop.offset) || stop.offset < 0 || stop.offset > 1 || stop.offset < previous) {
            throw new RangeError('Gradient stops must have nondecreasing offsets from zero to one');
        }
        previous = stop.offset;
        return { offset: stop.offset, color: color32(stop.color) };
    });
};
const writeGradientStops = (view, offset, stops) => {
    view.setUint16(offset, stops.length, true);
    // The following two padding bytes remain zero.
    stops.forEach((stop, index) => {
        view.setFloat32(offset + 4 + index * 8, stop.offset, true);
        view.setUint32(offset + 8 + index * 8, stop.color, true);
    });
};

/** Bounded builder for the common native paint protocol. Colors are packed
 * AABBGGRR (RGBA bytes on the wire), coordinates are viewport units. Each
 * finish() is self-contained. */
export class SceneBuilder {
    constructor(width, height, { maximumBytes = MAX_SCENE_BYTES } = {}) {
        copySceneFrame({ width, height, displayList: new Uint8Array() });
        if (!Number.isInteger(maximumBytes) || maximumBytes < 28 || maximumBytes > MAX_SCENE_BYTES) throw new RangeError('Invalid scene budget');
        this.width = width; this.height = height; this.maximumBytes = maximumBytes;
        this.bytes = 0; this.commands = [];
    }
    command(opcode, payloadBytes, write) {
        const length = payloadBytes + 8;
        if (this.bytes + length > this.maximumBytes) throw new RangeError('Scene exceeds builder budget');
        const bytes = new Uint8Array(length), view = new DataView(bytes.buffer);
        bytes[0] = opcode; view.setUint32(4, length, true);
        write?.(view);
        this.commands.push(bytes); this.bytes += length;
        return this;
    }
    floats(view, offset, values, count) {
        if (!Array.isArray(values) || values.length !== count) throw new TypeError('Invalid scene coordinate array');
        values.forEach((value, index) => view.setFloat32(offset + index * 4, finite(value), true));
    }
    pushTransform(transform, opacity = 1) {
        if (!(opacity >= 0 && opacity <= 1)) throw new RangeError('Invalid scene opacity');
        // The compact opaque transform uses save/transform directly. Only an
        // opacity group needs an isolated compositing surface in RiceDisplay.
        return this.command(1, opacity === 1 ? 28 : 32, view => {
            this.floats(view, 8, transform, 6); view.setFloat32(32, opacity, true);
            if (opacity !== 1) view.setUint8(37, 3); // Porter-Duff source-over.
        });
    }
    popTransform() { return this.command(2, 0); }
    pushClip(rect) { return this.command(3, 32, view => this.floats(view, 8, rect, 4)); }
    popClip() { return this.command(4, 0); }
    rect(rect, color) {
        return this.command(5, 20, view => { this.floats(view, 8, rect, 4); view.setUint32(24, color32(color), true); });
    }
    /** Retained linear fill, with absolute scene-coordinate start/end points. */
    linearGradient(rect, from, to, stops) {
        const checked = gradientStops(stops);
        rect = coordinates(rect, 4); from = coordinates(from, 2); to = coordinates(to, 2);
        return this.command(6, 36 + checked.length * 8, view => {
            this.floats(view, 8, rect, 4); this.floats(view, 24, from, 2); this.floats(view, 32, to, 2);
            writeGradientStops(view, 40, checked);
        });
    }
    /** Retained two-circle radial fill; circles are [x, y, radius]. */
    radialGradient(rect, innerCircle, outerCircle, stops) {
        const checked = gradientStops(stops);
        rect = coordinates(rect, 4);
        innerCircle = coordinates(innerCircle, 3); outerCircle = coordinates(outerCircle, 3);
        for (const circle of [innerCircle, outerCircle]) {
            if (finite(circle[2]) < 0) throw new RangeError('Gradient radius must be nonnegative');
        }
        return this.command(25, 44 + checked.length * 8, view => {
            this.floats(view, 8, rect, 4); this.floats(view, 24, innerCircle, 3); this.floats(view, 36, outerCircle, 3);
            writeGradientStops(view, 48, checked);
        });
    }
    /** Outer shadow of the enclosed paint alpha, including transparent images,
     * paths, strokes and text. Bounds must contain all enclosed visual content.
     * Optional transform maps scope-local alpha to shadow-local coordinates;
     * offset follows it, and Gaussian sigma is applied after the affine.
     * Foreground paint is unchanged. Inner/perspective effects are unsupported. */
    pushDropShadow(bounds, color, options = {}) {
        if (!options || typeof options !== 'object' || Array.isArray(options) ||
            Object.keys(options).some(key => !['offset', 'blur', 'transform'].includes(key))) {
            throw new TypeError('Content shadows support only outer offset, blur and affine transform');
        }
        let { offset = [0, 0], blur = 0, transform = null } = options;
        bounds = coordinates(bounds, 4); offset = coordinates(offset, 2); color = color32(color);
        if (bounds.slice(0, 2).concat(offset).some(value => Math.abs(value) > 1e6) ||
            bounds.slice(2).some(value => value <= 0 || value > 16384) || finite(blur) < 0 || blur > 1e6) {
            throw new RangeError('Invalid content shadow bounds');
        }
        if (transform !== null) {
            transform = coordinates(transform, 6).map(Math.fround);
            if (transform.slice(0, 4).some(value => Math.abs(value) > 1024) ||
                transform.slice(4).some(value => Math.abs(value) > 1e6) ||
                transform[0] * transform[3] === transform[1] * transform[2]) throw new RangeError('Invalid shadow affine');
        }
        return this.command(29, transform ? 56 : 32, view => {
            this.floats(view, 8, bounds, 4); this.floats(view, 24, offset, 2);
            view.setFloat32(32, blur, true); view.setUint32(36, color, true);
            if (transform) this.floats(view, 40, transform, 6);
        });
    }
    popDropShadow() { return this.command(30, 0); }
    /** Rectangle-only shadow. Blur is Gaussian sigma in scene units. Outset
     * masks include their interior; paint foreground content afterwards. */
    boxShadow(rect, color, { offset = [0, 0], blur = 0, spread = 0, inset = false } = {}) {
        rect = coordinates(rect, 4); offset = coordinates(offset, 2);
        if (rect[2] < 0 || rect[3] < 0 || finite(blur) < 0 || typeof inset !== 'boolean') throw new RangeError('Invalid box shadow');
        finite(spread); color32(color);
        return this.command(10, 40, view => {
            this.floats(view, 8, rect, 4); this.floats(view, 24, offset, 2);
            view.setFloat32(32, blur, true); view.setFloat32(36, spread, true);
            view.setUint32(40, color, true); view.setUint8(44, inset ? 1 : 0);
        });
    }
    line(from, to, width, color) {
        if (finite(width) < 0) throw new RangeError('Invalid line width');
        return this.command(11, 28, view => {
            this.floats(view, 8, from, 2); this.floats(view, 16, to, 2);
            view.setFloat32(24, width, true); view.setUint32(28, color32(color), true);
        });
    }
    ellipse(rect, color, { filled = true, width = 1 } = {}) {
        if (finite(width) < 0) throw new RangeError('Invalid ellipse width');
        return this.command(13, 36, view => {
            this.floats(view, 8, rect, 4); view.setFloat32(28, 360, true);
            view.setFloat32(32, width, true); view.setUint32(36, color32(color), true); view.setUint8(40, filled ? 1 : 0);
        });
    }
    /** Reusable native vector path. Coordinates use the current scene units. */
    definePath(id, segments) {
        const verbs = { M: [0, 2], L: [1, 2], Q: [2, 4], C: [3, 6], Z: [4, 0] };
        if (!Number.isInteger(id) || id < 1 || id > 0xffffffff || !Array.isArray(segments) ||
            !segments.length || segments.length > 16384) throw new RangeError('Invalid scene path');
        const checked = segments.map(segment => {
            const spec = Array.isArray(segment) && verbs[segment[0]];
            if (!spec || segment.length !== spec[1] + 1) throw new TypeError('Invalid scene path segment');
            return [spec[0], ...segment.slice(1).map(finite)];
        });
        return this.command(19, 8 + checked.length * 28, view => {
            view.setUint32(8, id, true); view.setUint32(12, checked.length, true);
            checked.forEach(([verb, ...values], index) => {
                const offset = 16 + index * 28; view.setUint8(offset, verb);
                values.forEach((value, i) => view.setFloat32(offset + 4 + i * 4, value, true));
            });
        });
    }
    fillPath(id, color, { transform = [1, 0, 0, 1, 0, 0], evenOdd = false } = {}) {
        if (!Number.isInteger(id) || id < 1 || id > 0xffffffff || typeof evenOdd !== 'boolean') throw new RangeError('Invalid path fill');
        return this.command(21, 36, view => {
            view.setUint32(8, id, true); this.floats(view, 12, transform, 6);
            view.setUint32(36, color32(color), true); view.setUint8(40, evenOdd ? 1 : 0);
        });
    }
    strokePath(id, width, color, { transform = [1, 0, 0, 1, 0, 0], cap = 0, join = 0, miterLimit = 4 } = {}) {
        if (!Number.isInteger(id) || id < 1 || id > 0xffffffff || finite(width) < 0 || finite(miterLimit) < 0 ||
            ![0, 1, 2].includes(cap) || ![0, 1, 2].includes(join)) throw new RangeError('Invalid path stroke');
        return this.command(22, 44, view => {
            view.setUint32(8, id, true); this.floats(view, 12, transform, 6);
            view.setUint32(36, color32(color), true); view.setFloat32(40, width, true);
            view.setFloat32(44, miterLimit, true); view.setUint8(48, cap); view.setUint8(49, join);
        });
    }
    defineImage(id, width, height, rgba) {
        if (!Number.isInteger(id) || id < 1 || id > 0xffffffff ||
            !Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 ||
            width > 16384 || height > 16384 || !(rgba instanceof Uint8Array) || rgba.length !== width * height * 4) {
            throw new RangeError('Invalid scene image resource');
        }
        return this.command(14, 16 + rgba.length, view => {
            view.setUint32(8, id, true); view.setUint32(12, width, true); view.setUint32(16, height, true);
            view.setUint32(20, rgba.length, true);
            new Uint8Array(view.buffer, 24).set(rgba);
        });
    }
    image(id, rect, { uv = [0, 0, 1, 1], opacity = 1, smoothing = true } = {}) {
        if (!Number.isInteger(id) || id < 1 || id > 0xffffffff || !(opacity >= 0 && opacity <= 1) ||
            typeof smoothing !== 'boolean' || !Array.isArray(uv) || uv.length !== 4 ||
            uv.some(value => !Number.isFinite(value) || value < 0 || value > 1) || uv[2] <= uv[0] || uv[3] <= uv[1]) {
            throw new RangeError('Invalid scene image draw');
        }
        return this.command(7, 44, view => {
            this.floats(view, 8, rect, 4); this.floats(view, 24, uv, 4);
            view.setUint32(40, id, true); view.setFloat32(44, opacity, true); view.setUint8(48, smoothing ? 1 : 0);
        });
    }
    append(displayList) {
        const frame = copySceneFrame({ width: this.width, height: this.height, displayList });
        if (this.bytes + frame.displayList.length > this.maximumBytes) throw new RangeError('Scene exceeds builder budget');
        this.commands.push(frame.displayList); this.bytes += frame.displayList.length;
        return this;
    }
    finish() {
        const displayList = new Uint8Array(this.bytes);
        let offset = 0;
        for (const command of this.commands) { displayList.set(command, offset); offset += command.length; }
        return copySceneFrame({ width: this.width, height: this.height, displayList });
    }
}
