export const CONTENT_SHADOW_RENDERER_VERSION = 1;
import { beginContentShadow, endContentShadow } from './rice-native-content-shadow.mjs';
import { renderRiceBoxShadow } from './rice-native-box-shadow.mjs';
import { mailDocumentHtml } from "./rice-mail-document-surface.mjs";
import {
    packedRgba,
    roundedPath,
} from "./rice-native-canvas-primitives.mjs";

// Current frames are self-contained because native UI snapshots may be
// coalesced before they reach the compositor. Keep decoded textures by native
// web surface so a repeated definition can still avoid recreating Canvas
// image state after those bytes arrive.
const retainedSceneTextures = new WeakMap();
const MAX_RETAINED_TEXTURE_BYTES = 64 * 1024 * 1024;
const MAX_RETAINED_TEXTURE_SURFACES = 16;

export function releaseRiceWebSceneTextures(context, namespace = null) {
    if(namespace===null){retainedSceneTextures.delete(context);return;}
    const surfaces=retainedSceneTextures.get(context);
    if(!surfaces)return;
    const prefix=`${namespace}:`;
    for(const key of surfaces.keys())if(key.startsWith(prefix))surfaces.delete(key);
    if(!surfaces.size)retainedSceneTextures.delete(context);
}

function retainedTextures(context, key) {
    if (key === null) return new Map();
    let surfaces = retainedSceneTextures.get(context);
    if (!surfaces) { surfaces = new Map(); retainedSceneTextures.set(context, surfaces); }
    let textures = surfaces.get(key);
    if (textures) {
        surfaces.delete(key);
        surfaces.set(key, textures);
        return textures;
    }
    textures = new Map();
    surfaces.set(key, textures);
    while (surfaces.size > MAX_RETAINED_TEXTURE_SURFACES) {
        surfaces.delete(surfaces.keys().next().value);
    }
    return textures;
}

function equalPixels(left, right) {
    if (!left || left.byteLength !== right.byteLength) return false;
    for (let index = 0; index < right.byteLength; index += 1) {
        if (left[index] !== right[index]) return false;
    }
    return true;
}

function retainedImageSurface(context, width, height, rgba) {
    let surface = null;
    if (typeof globalThis.OffscreenCanvas === "function") {
        surface = new globalThis.OffscreenCanvas(width, height);
    } else {
        const document = context.canvas?.ownerDocument;
        if (document?.createElement) {
            surface = document.createElement("canvas");
            surface.width = width;
            surface.height = height;
        }
    }
    const target = surface?.getContext?.("2d");
    if (!target?.createImageData || !target?.putImageData) return null;
    const image = target.createImageData(width, height);
    image.data.set(rgba);
    target.putImageData(image, 0, 0);
    return surface;
}

function recordReader(view, offset) {
    const number = at => view.getFloat32(offset + 8 + at, true);
    return {
        number,
        rectangle: at => ({
            x: number(at),
            y: number(at + 4),
            width: number(at + 8),
            height: number(at + 12),
        }),
    };
}

function replayRetainedPath(context, segments) {
    context.beginPath();
    for (const segment of segments) {
        const c = segment.coordinates;
        if (segment.verb === 0) context.moveTo(c[0], c[1]);
        else if (segment.verb === 1) context.lineTo(c[0], c[1]);
        else if (segment.verb === 2) context.quadraticCurveTo(c[0], c[1], c[2], c[3]);
        else if (segment.verb === 3) {
            context.bezierCurveTo(c[0], c[1], c[2], c[3], c[4], c[5]);
        } else context.closePath();
    }
}

function renderPathRecord(context, view, offset, opcode, recordBytes, read, paths) {
    if (opcode === 19 && recordBytes >= 44) {
        const path = view.getUint32(offset + 8, true);
        const count = view.getUint32(offset + 12, true);
        if (path === 0 || count === 0 || recordBytes !== 16 + count * 28) {
            throw new RangeError("invalid Rice retained path");
        }
        const segments = [];
        for (let index = 0; index < count; index += 1) {
            const position = offset + 16 + index * 28;
            const coordinates = [];
            for (let coordinate = 0; coordinate < 6; coordinate += 1) {
                coordinates.push(view.getFloat32(position + 4 + coordinate * 4, true));
            }
            segments.push({ verb: view.getUint8(position), coordinates });
        }
        paths.set(path, segments);
        return { handled: true, depthDelta: 0 };
    }
    if (![20, 21, 22].includes(opcode)) return { handled: false, depthDelta: 0 };
    const path = paths.get(view.getUint32(offset + 8, true));
    if (!path) throw new RangeError("undefined Rice retained path");
    const transform = [4, 8, 12, 16, 20, 24].map(read.number);
    context.save();
    context.transform(...transform);
    replayRetainedPath(context, path);
    if (opcode === 20 && recordBytes === 40) {
        context.clip(view.getUint8(offset + 36) === 0 ? "nonzero" : "evenodd");
        const [a, b, c, d, e, f] = transform;
        const determinant = a * d - b * c;
        if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-12) {
            context.restore();
            context.save();
            context.beginPath();
            context.rect(0, 0, 0, 0);
            context.clip();
        } else {
            context.transform(
                d / determinant,
                -b / determinant,
                -c / determinant,
                a / determinant,
                (c * f - d * e) / determinant,
                (b * e - a * f) / determinant,
            );
        }
        return { handled: true, depthDelta: 1 };
    }
    if (opcode === 21 && recordBytes === 44) {
        context.fillStyle = packedRgba(view.getUint32(offset + 36, true));
        context.fill(view.getUint8(offset + 40) === 0 ? "nonzero" : "evenodd");
    } else if (opcode === 22 && recordBytes === 52) {
        context.strokeStyle = packedRgba(view.getUint32(offset + 36, true));
        context.lineWidth = read.number(32);
        context.miterLimit = read.number(36);
        context.lineCap = ["butt", "round", "square"][view.getUint8(offset + 48)];
        context.lineJoin = ["bevel", "miter", "round"][view.getUint8(offset + 49)];
        context.stroke();
    } else {
        context.restore();
        throw new RangeError("invalid Rice retained path operation");
    }
    context.restore();
    return { handled: true, depthDelta: 0 };
}

function rememberImageDefinition(resident, bytes, offset, epoch) {
    resident.source = epoch !== undefined && typeof WeakRef === 'function' ? new WeakRef(bytes) : null;
    resident.sourceOffset = offset;
    resident.sourceEpoch = epoch;
}

function renderImageRecord(context, bytes, view, offset, end, opcode, recordBytes, read, textures, usedTextures, epoch) {
    if (opcode === 14 && recordBytes >= 24) {
        const texture = view.getUint32(offset + 8, true);
        usedTextures.add(texture);
        const width = view.getUint32(offset + 12, true);
        const height = view.getUint32(offset + 16, true);
        const byteLength = view.getUint32(offset + 20, true);
        if (
            texture === 0 ||
            width === 0 ||
            height === 0 ||
            byteLength !== width * height * 4 ||
            recordBytes !== 24 + byteLength
        ) {
            throw new RangeError("invalid Rice retained image");
        }
        const resident = textures.get(texture);
        // Native snapshots own immutable byte arrays. The bridge reuses that
        // exact array for an unchanged epoch (both Bowl and SceneSurface). Weak
        // identity avoids retaining another complete multi-megabyte frame.
        // New arrays, new epochs and ownerless/legacy frames still compare pixels.
        if (epoch !== undefined && resident?.source?.deref() === bytes &&
            resident.sourceOffset === offset && resident.sourceEpoch === epoch &&
            resident.width === width && resident.height === height) return true;
        const rgba = bytes.subarray(offset + 24, end);
        if (
            resident?.width === width &&
            resident?.height === height &&
            equalPixels(resident.rgba, rgba)
        ) {
            rememberImageDefinition(resident, bytes, offset, epoch);
            return true;
        }
        const surface = retainedImageSurface(
            context,
            width,
            height,
            rgba,
        );
        if (surface) {
            const value = { surface, width, height, rgba: Uint8Array.from(rgba) };
            rememberImageDefinition(value, bytes, offset, epoch);
            textures.set(texture, value);
        }
        return true;
    }
    if (opcode !== 7 || recordBytes !== 52) return false;
    const textureId = view.getUint32(offset + 40, true);
    usedTextures.add(textureId);
    const texture = textures.get(textureId);
    if (!texture) {
        // A malformed or legacy stateful frame must not turn every desktop
        // repaint into an exception loop. Current Bowl frames carry their own
        // definitions, so this remains a visible diagnostic without taking
        // the compositor down repeatedly.
        textures.missing ??= new Set();
        if (!textures.missing.has(textureId)) {
            textures.missing.add(textureId);
            console.warn(
                `[RiceWM web scene] skipped undefined retained image ${textureId}`,
            );
        }
        return true;
    }
    const rect = read.rectangle(0);
    const u0 = read.number(16);
    const v0 = read.number(20);
    const u1 = read.number(24);
    const v1 = read.number(28);
    context.save();
    context.globalAlpha *= read.number(36);
    context.imageSmoothingEnabled = view.getUint8(offset + 48) === 1;
    context.drawImage(
        texture.surface,
        u0 * texture.width,
        v0 * texture.height,
        (u1 - u0) * texture.width,
        (v1 - v0) * texture.height,
        rect.x,
        rect.y,
        rect.width,
        rect.height,
    );
    context.restore();
    return true;
}

function renderGlyphRecord(context, view, offset, recordBytes, read) {
    if (recordBytes < 28) return;
    const fontInstance = view.getUint32(offset + 8, true);
    const count = view.getUint32(offset + 24, true);
    if (recordBytes !== 28 + count * 12) {
        throw new RangeError("invalid Rice web glyph run");
    }
    const size = Math.max(1, fontInstance & 0xff);
    const weight = (fontInstance & (1 << 8)) === 0 ? 400 : 700;
    const italic = (fontInstance & (1 << 9)) === 0 ? "" : "italic ";
    const family = (fontInstance >> 10) & 3;
    const cssFamily = family === 1
        ? '"RiceWM Serif", Georgia, serif'
        : family === 2
            ? '"RiceWM Mono", ui-monospace, monospace'
            : family === 3
                ? '"RiceWM Cursive", cursive'
                : '"RiceWM Sans", sans-serif';
    context.font = `${italic}${weight} ${size}px ${cssFamily}`;
    context.textAlign = "left";
    context.textBaseline = "alphabetic";
    context.fillStyle = packedRgba(view.getUint32(offset + 20, true));
    for (let glyph = 0; glyph < count; glyph += 1) {
        const position = offset + 28 + glyph * 12;
        const scalar = view.getUint32(position, true);
        if (
            scalar > 0x10ffff ||
            (scalar >= 0xd800 && scalar <= 0xdfff)
        ) {
            throw new RangeError("invalid Rice web glyph scalar");
        }
        context.fillText(
            String.fromCodePoint(scalar),
            read.number(4) + view.getFloat32(position + 4, true),
            read.number(8) + view.getFloat32(position + 8, true),
        );
    }
}

function renderShapeRecord(context, view, offset, opcode, recordBytes, read) {
    if (opcode === 5 && recordBytes === 28) {
        const rect = read.rectangle(0);
        context.fillStyle = packedRgba(view.getUint32(offset + 24, true));
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
    } else if (opcode === 6 && recordBytes >= 44) {
        const rect = read.rectangle(0);
        const gradient = context.createLinearGradient(
            read.number(16),
            read.number(20),
            read.number(24),
            read.number(28),
        );
        const count = view.getUint16(offset + 40, true);
        for (let stop = 0; stop < count; stop += 1) {
            const position = offset + 44 + stop * 8;
            gradient.addColorStop(
                view.getFloat32(position, true),
                packedRgba(view.getUint32(position + 4, true)),
            );
        }
        context.fillStyle = gradient;
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
    } else if (opcode === 25 && recordBytes >= 52) {
        const rect = read.rectangle(0);
        const gradient = context.createRadialGradient(
            read.number(16),
            read.number(20),
            read.number(24),
            read.number(28),
            read.number(32),
            read.number(36),
        );
        const count = view.getUint16(offset + 48, true);
        for (let stop = 0; stop < count; stop += 1) {
            const position = offset + 52 + stop * 8;
            gradient.addColorStop(
                view.getFloat32(position, true),
                packedRgba(view.getUint32(position + 4, true)),
            );
        }
        context.fillStyle = gradient;
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
    } else if (opcode === 26 && recordBytes >= 48) {
        const rect = read.rectangle(0);
        let startAngle = read.number(24);
        let endAngle = read.number(28);
        const extend = view.getUint8(offset + 40);
        const count = view.getUint16(offset + 44, true);
        let stops = Array.from({ length: count }, (_, stop) => {
            const position = offset + 48 + stop * 8;
            return {
                offset: view.getFloat32(position, true),
                color: packedRgba(view.getUint32(position + 4, true)),
            };
        });
        if (endAngle < startAngle) {
            [startAngle, endAngle] = [endAngle, startAngle];
            stops = stops
                .map(stop => ({ ...stop, offset: 1 - stop.offset }))
                .reverse();
        }
        const gradient = context.createConicGradient(
            startAngle,
            read.number(16),
            read.number(20),
        );
        const tau = Math.PI * 2;
        const period = Math.min(1, Math.max(0, (endAngle - startAngle) / tau));
        let additions = 0;
        let finalColor = stops.at(-1)?.color || "rgba(0, 0, 0, 0)";
        const add = (position, color) => {
            if (additions >= 256 || position < 0 || position > 1) return;
            gradient.addColorStop(position, color);
            finalColor = color;
            additions += 1;
        };
        if (period > 0 && (extend === 1 || extend === 2) && period < 1) {
            const cycles = Math.min(256, Math.ceil(1 / period));
            for (let cycle = 0; cycle < cycles && additions < 256; cycle += 1) {
                const reflected = extend === 2 && cycle % 2 === 1;
                const cycleStops = reflected ? [...stops].reverse() : stops;
                for (const stop of cycleStops) {
                    const local = reflected ? 1 - stop.offset : stop.offset;
                    add((cycle + local) * period, stop.color);
                }
            }
        } else {
            for (const stop of stops) add(stop.offset * (period || 1), stop.color);
        }
        if (additions === 0) add(0, finalColor);
        if (period < 1) add(1, finalColor);
        context.fillStyle = gradient;
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
    } else if (opcode === 9 && recordBytes === 72) {
        const rect = read.rectangle(0);
        const widths = [16, 20, 24, 28].map(read.number);
        const colors = [40, 44, 48, 52].map(at =>
            packedRgba(view.getUint32(offset + at, true))
        );
        context.fillStyle = colors[0];
        context.fillRect(rect.x, rect.y, rect.width, widths[0]);
        context.fillStyle = colors[1];
        context.fillRect(
            rect.x + rect.width - widths[1],
            rect.y,
            widths[1],
            rect.height,
        );
        context.fillStyle = colors[2];
        context.fillRect(
            rect.x,
            rect.y + rect.height - widths[2],
            rect.width,
            widths[2],
        );
        context.fillStyle = colors[3];
        context.fillRect(rect.x, rect.y, widths[3], rect.height);
    } else if (opcode === 10 && recordBytes === 48) {
        renderRiceBoxShadow(context, read.rectangle(0), [read.number(16), read.number(20)],
            read.number(24), read.number(28), view.getUint32(offset + 40, true), view.getUint8(offset + 44) === 1);
    } else if (opcode === 11 && recordBytes === 36) {
        context.beginPath();
        context.moveTo(read.number(0), read.number(4));
        context.lineTo(read.number(8), read.number(12));
        context.lineWidth = read.number(16);
        context.strokeStyle = packedRgba(view.getUint32(offset + 28, true));
        const style = view.getUint8(offset + 32);
        context.setLineDash(style === 1
            ? [context.lineWidth, context.lineWidth]
            : style === 2
            ? [context.lineWidth * 3, context.lineWidth * 2]
            : []);
        context.stroke();
        context.setLineDash([]);
    } else if (opcode === 12 && recordBytes >= 44) {
        const count = view.getUint16(offset + 16, true);
        const flags = view.getUint8(offset + 18);
        context.beginPath();
        for (let point = 0; point < count; point += 1) {
            const position = offset + 20 + point * 8;
            const x = view.getFloat32(position, true);
            const y = view.getFloat32(position + 4, true);
            if (point === 0) context.moveTo(x, y);
            else context.lineTo(x, y);
        }
        if ((flags & 2) !== 0) context.closePath();
        const color = packedRgba(view.getUint32(offset + 8, true));
        if ((flags & 1) !== 0) {
            context.fillStyle = color;
            context.fill();
        } else {
            context.lineWidth = read.number(4);
            context.strokeStyle = color;
            context.stroke();
        }
    } else if (opcode === 13 && recordBytes === 44) {
        const rect = read.rectangle(0);
        context.beginPath();
        context.ellipse(
            rect.x + rect.width / 2,
            rect.y + rect.height / 2,
            Math.abs(rect.width / 2),
            Math.abs(rect.height / 2),
            0,
            read.number(16) * Math.PI / 180,
            read.number(20) * Math.PI / 180,
        );
        const color = packedRgba(view.getUint32(offset + 36, true));
        if (view.getUint8(offset + 40) === 1) {
            context.fillStyle = color;
            context.fill();
        } else {
            context.lineWidth = read.number(24);
            context.strokeStyle = color;
            context.stroke();
        }
    }
}

function renderStackRecord(context, opcode, recordBytes, read, depth) {
    if (opcode === 1 && recordBytes === 36) {
        context.save();
        context.transform(
            read.number(0),
            read.number(4),
            read.number(8),
            read.number(12),
            read.number(16),
            read.number(20),
        );
        context.globalAlpha *= read.number(24);
        return depth + 1;
    }
    if (opcode === 2 && recordBytes === 8) {
        if (depth < 1) throw new RangeError("unbalanced Rice web stacking context");
        context.restore();
        return depth - 1;
    }
    if (opcode === 3 && recordBytes === 40) {
        context.save();
        roundedPath(context, read.rectangle(0), Math.max(
            read.number(16),
            read.number(20),
            read.number(24),
            read.number(28),
        ));
        context.clip();
        return depth + 1;
    }
    if (opcode === 4 && recordBytes === 8) {
        if (depth < 1) throw new RangeError("unbalanced Rice web clip");
        context.restore();
        return depth - 1;
    }
    return depth;
}

const contentShadowBudgets = new WeakMap();
function contentShadowBudget(bytes, view) {
    if (contentShadowBudgets.has(bytes)) return contentShadowBudgets.get(bytes);
    // Frames are immutable. Scan once, then share an 8MP capture sampling
    // budget across their shadow scopes; ordinary repaints incur no rescan.
    let count = 0;
    for (let offset = 0; offset < bytes.length;) {
        if (offset + 8 > bytes.length) throw new RangeError('Truncated scene record');
        const compact = view.getUint16(offset + 2, true), size = compact ? compact * 4 : view.getUint32(offset + 4, true);
        if (size < 8 || offset + size > bytes.length) throw new RangeError('Invalid scene record');
        if (bytes[offset] === 29 && ++count > 128) throw new RangeError('Too many content shadows');
        offset += size;
    }
    const budget = Math.min(1024 * 1024, Math.floor(8 * 1024 * 1024 / Math.max(1, count)));
    contentShadowBudgets.set(bytes, budget); return budget;
}

function replayRiceWebScene(context, scene, bytes, textures, mediaSurface, usedTextures) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const width = Math.max(1, scene.viewportWidth);
    const height = Math.max(1, scene.viewportHeight);
    const paths = new Map();
    const mediaRegions = new Map((scene.mediaRegions || []).map(region => [region.offset, region.request]));
    let depth = 0;
    const compositeLayers = [];
    const shadowLayers = [];
    const shadowPixelBudget = contentShadowBudget(bytes, view);
    context.save();
    context.beginPath();
    context.rect(scene.rect.x, scene.rect.y, scene.rect.width, scene.rect.height);
    context.clip();
    context.translate(scene.rect.x, scene.rect.y);
    // Web documents reflow to their logical viewport width. Scaling the two
    // axes independently makes text visibly short or tall whenever native
    // layout exposes less than the full document height. Preserve the page's
    // pixel aspect ratio and let the native surface clip the vertical extent.
    const scale = scene.vectorScene
        ? Math.min(scene.rect.width / width, scene.rect.height / height)
        : scene.rect.width / width;
    if (scene.vectorScene) context.translate((scene.rect.width - width * scale) / 2, (scene.rect.height - height * scale) / 2);
    context.scale(scale, scale);
    for (let offset = 0; offset < bytes.byteLength;) {
        if (offset + 8 > bytes.byteLength) {
            throw new RangeError("truncated Rice web display-list header");
        }
        const opcode = view.getUint8(offset);
        const compactWords = view.getUint16(offset + 2, true);
        const recordBytes = compactWords === 0
            ? view.getUint32(offset + 4, true)
            : compactWords * 4;
        const end = offset + recordBytes;
        if (recordBytes < 8 || end > bytes.byteLength) {
            throw new RangeError("invalid Rice web display-list record");
        }
        const read = recordReader(view, offset);
        if (opcode === 29 && (recordBytes === 40 || recordBytes === 64)) {
            if (shadowLayers.length >= 4) throw new RangeError('Too many nested content shadows');
            const layer = beginContentShadow(context, Array.from({ length: 4 }, (_, i) => read.number(i * 4)),
                [read.number(16), read.number(20)], read.number(24), view.getUint32(offset + 36, true), shadowPixelBudget,
                recordBytes === 64 ? Array.from({length: 6}, (_, i) => read.number(32 + i * 4)) : null);
            layer.depth = ++depth; shadowLayers.push(layer); context = layer.context;
            offset = end; continue;
        }
        if (opcode === 30 && recordBytes === 8) {
            if (shadowLayers.at(-1)?.depth !== depth) throw new RangeError('Unbalanced content shadow');
            context = endContentShadow(shadowLayers.pop()); depth -= 1;
            offset = end; continue;
        }
        if (opcode === 1 && recordBytes === 40) {
            const parent = context;
            const surface = typeof globalThis.OffscreenCanvas === "function"
                ? new OffscreenCanvas(parent.canvas.width, parent.canvas.height)
                : parent.canvas.ownerDocument.createElement("canvas");
            surface.width = parent.canvas.width;
            surface.height = parent.canvas.height;
            context = surface.getContext("2d");
            context.setTransform(parent.getTransform());
            context.transform(...Array.from({ length: 6 }, (_, i) => read.number(i * 4)));
            compositeLayers.push({ parent, surface, opacity: read.number(24),
                mix: view.getUint8(offset + 36), compose: view.getUint8(offset + 37),
                depth: ++depth });
            offset = end;
            continue;
        }
        if (opcode === 2 && compositeLayers.at(-1)?.depth === depth) {
            const layer = compositeLayers.pop();
            context = layer.parent;
            context.save();
            context.setTransform(1, 0, 0, 1, 0, 0);
            context.globalAlpha *= layer.opacity;
            const mixes = ["source-over", "multiply", "screen", "overlay", "darken", "lighten",
                "color-dodge", "color-burn", "hard-light", "soft-light", "difference", "exclusion",
                "hue", "saturation", "color", "luminosity"];
            const compositions = [null, "copy", null, "source-over", "destination-over",
                "source-in", "destination-in", "source-out", "destination-out", "source-atop",
                "destination-atop", "xor", "lighter", "lighter"];
            if (layer.compose === 0) {
                context.clearRect(0, 0, layer.surface.width, layer.surface.height);
            } else if (layer.compose !== 2) {
                context.globalCompositeOperation = layer.compose === 3
                    ? mixes[layer.mix] : compositions[layer.compose];
                context.drawImage(layer.surface, 0, 0);
            }
            context.restore();
            // Release the backing store after this isolated CSS group is composed.
            layer.surface.width = 0;
            layer.surface.height = 0;
            depth -= 1;
            offset = end;
            continue;
        }
        if (opcode === 27 && mediaRegions.has(offset)) {
            const surface = mediaSurface(BigInt(mediaRegions.get(offset)));
            const rect = read.rectangle(0);
            const sourceWidth = Number(surface?.displayWidth || surface?.videoWidth || surface?.naturalWidth || surface?.width);
            const sourceHeight = Number(surface?.displayHeight || surface?.videoHeight || surface?.naturalHeight || surface?.height);
            if (Number.isFinite(sourceWidth) && sourceWidth > 0 &&
                Number.isFinite(sourceHeight) && sourceHeight > 0) {
                const fit = Math.min(rect.width / sourceWidth, rect.height / sourceHeight);
                const width = sourceWidth * fit, height = sourceHeight * fit;
                context.drawImage(surface, rect.x + (rect.width - width) / 2,
                    rect.y + (rect.height - height) / 2, width, height);
            }
        }
        const pathResult = renderPathRecord(
            context,
            view,
            offset,
            opcode,
            recordBytes,
            read,
            paths,
        );
        let nextDepth = depth + pathResult.depthDelta;
        if (!pathResult.handled) nextDepth = renderStackRecord(
            context,
            opcode,
            recordBytes,
            read,
            depth,
        );
        if (!pathResult.handled && nextDepth === depth) {
            if (opcode === 8) {
                renderGlyphRecord(context, view, offset, recordBytes, read);
            } else if (!renderImageRecord(
                context,
                bytes,
                view,
                offset,
                end,
                opcode,
                recordBytes,
                read,
                textures,
                usedTextures,
                scene.epoch,
            )) {
                renderShapeRecord(
                    context,
                    view,
                    offset,
                    opcode,
                    recordBytes,
                    read,
                );
            }
        }
        depth = nextDepth;
        offset = end;
    }
    while (depth > 0) {
        context.restore();
        depth -= 1;
    }
    context.restore();
}

export function renderRiceWebScene(context, scene, mediaSurface = () => null) {
    if (mailDocumentHtml(scene) !== null) {
        // Live documents sit below the desktop canvas, like terminal portals.
        // Erase the native WebSurface placeholder so the DOM remains visible.
        context.clearRect(scene.rect.x, scene.rect.y, scene.rect.width, scene.rect.height);
        return;
    }
    const bytes = scene.displayList instanceof Uint8Array
        ? scene.displayList
        : Uint8Array.from(scene.displayList);
    // Ownerless frames are intentionally ephemeral: using their byte array as
    // a strong Map key would retain a second complete frame outside byte limits.
    const textureKey = scene.surface === undefined
        ? null : `${scene.cacheNamespace || ""}:${scene.surface}`;
    const textures = retainedTextures(context, textureKey);
    if (bytes.byteLength === 0) return;
    // Replay directly; compositing groups and bounded shadow masks are the only
    // isolated surfaces. The rest of the document remains retained primitives.
    // Replaying the retained display list directly keeps
    // glyphs at the final device transform and preserves the live document's
    // semantic frame/input identity for every embedding application.
    const usedTextures = new Set();
    replayRiceWebScene(context, scene, bytes, textures, mediaSurface, usedTextures);
    for (const id of textures.keys()) if (!usedTextures.has(id)) textures.delete(id);
    if (textures.missing) for (const id of textures.missing) if (!usedTextures.has(id)) textures.missing.delete(id);
    // Count both the retained RGBA comparison bytes and the decoded canvas.
    // Frames are self-contained, so eviction is safe on the next repaint.
    const surfaces = retainedSceneTextures.get(context);
    if (surfaces) {
        const cost = () => [...surfaces.values()].reduce((sum, entries) => sum +
            [...entries.values()].reduce((bytes, entry) => bytes + entry.rgba.byteLength * 2, 0), 0);
        while (surfaces.size && cost() > MAX_RETAINED_TEXTURE_BYTES) surfaces.delete(surfaces.keys().next().value);
    }
}
