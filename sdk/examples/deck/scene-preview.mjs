import { paintTable, paintTableSelection } from './table-render.mjs';
import { SceneBuilder } from '@ricewm/app';
import { objectInkBounds } from './scene-ink-bounds.mjs';
import { transformRect, unionRects } from './affine.mjs';
import { objectShadowPlan, shadowVisualBounds, pushObjectShadow, popObjectShadow } from './shadow-render.mjs';
import { objectCorners, linePoints, rotationHandle, previewObjects, canResize } from './canvas-geometry.mjs';

import { textClipRect } from './text-overflow.mjs';
import { shapeById } from './shape-catalog.mjs';
import { paintBackgroundPicture, paintBackgroundGradient } from './background-render.mjs';

const packed = ({ color, opacity }) => {
    const rgb = Number.parseInt(color.slice(1), 16);
    return (((Math.round(opacity * 255) << 24) | ((rgb & 255) << 16) | (rgb & 0xff00) | (rgb >>> 16)) >>> 0);
};

/** Paint source-resolved vectors, native text outlines and decoded pictures. */
export function geometryFrame(scene, { selected = [], boxes = new Map(), textFrames = new Map(), imageFrames = new Map(), lines = new Map(), rotations = new Map(), placeholderGuides = false, preview = null, selectedCell = null, editingCell = null, captureLayers = false, objectOpacity = new Map() } = {}) {
    if (!scene.size) throw new Error('Slide size is inherited or unavailable; geometry cannot be placed yet');
    const ratio = Math.min(1, 4096 / Math.max(scene.size.width, scene.size.height));
    const width = Math.max(1, Math.round(scene.size.width * ratio)), height = Math.max(1, Math.round(scene.size.height * ratio));
    const builder = new SceneBuilder(width, height).rect([0, 0, width, height], 0xffffffff);
    if (scene.background) builder.rect([0, 0, width, height], packed(scene.background));
    builder
        .pushTransform([width / scene.size.width, 0, 0, height / scene.size.height, 0, 0]);
    // Every frame owns its definitions; identical resources are emitted once.
    const resourceStart = builder.bytes;
    const textures = new Map();
    for (const resource of imageFrames.values()) {
        if (textures.has(resource.key)) continue;
        const id = textures.size + 1; textures.set(resource.key, id);
        builder.defineImage(id, resource.width, resource.height, resource.rgba);
    }
    const resourceEnd = builder.bytes;
    paintBackgroundGradient(builder, scene, packed);
    paintBackgroundPicture(builder, scene, imageFrames, textures);
    const backgroundEnd = builder.bytes, objectLayers = [], layerTree = [];
    let painted = 0, nextPath = 0x80000001;
    const projected = previewObjects(preview ? [...scene.objects, preview] : scene.objects, { boxes, lines, rotations });
    const byId = new Map(projected.map(object => [object.id, object]));
    const children = object => (object.group?.children || []).map(id => byId.get(id)).filter(Boolean);
    const ink = new Map(), plans = new Map(), visiting = new Set(), diagnostics = [];
    const unsupportedShadow = (object, message) => diagnostics.push({ objectId: object.id, code: 'SHADOW_RENDER_LIMIT', message });
    function prepare(object) {
        if (ink.has(object.id)) return ink.get(object.id);
        if (visiting.has(object.id)) throw new Error('Cyclic slide group');
        visiting.add(object.id);
        let bounds = null, local = null;
        if (object.kind === 'group') {
            for (const child of children(object)) bounds = unionRects(bounds, prepare(child));
        } else {
            local = objectInkBounds(scene, object, { textFrames, imageFrames });
            if (local && object.transform) bounds = transformRect(object.transform, local);
        }
        let plan = null;
        try {
            plan = objectShadowPlan(object, bounds, local);
            if (plan) new SceneBuilder(1,1).pushDropShadow(plan.bounds,packed(object.outerShadow),
                { transform:plan.transform,offset:plan.offset,blur:plan.blur });
        } catch (error) {
            if (!(error instanceof RangeError)) throw error;
            unsupportedShadow(object, error.message); plan = null;
        }
        plans.set(object.id, plan);
        bounds = unionRects(bounds, shadowVisualBounds(plan, bounds));
        ink.set(object.id, bounds); visiting.delete(object.id);
        return bounds;
    }
    // Retain complete group command ranges so Morph never splits a shadow scope.
    const roots = projected.filter(object => !object.parentId || !byId.has(object.parentId));
    const affectedRoots = new Set();
    for (const object of projected) {
        if (!object.outerShadow) continue;
        let ancestor = object, count = 0;
        while (ancestor.parentId && byId.has(ancestor.parentId)) {
            if (++count > projected.length) throw new Error('Cyclic slide group');
            ancestor = byId.get(ancestor.parentId);
        }
        affectedRoots.add(ancestor.id);
    }
    for (const object of roots) if (affectedRoots.has(object.id)) prepare(object);
    let shadowScopes = 0;
    function paintObject(object, depth = 0, parentId = null) {
        const layer = captureLayers ? { id: object.id, parentId, start: builder.bytes, end: 0 } : null;
        if (layer) layerTree.push(layer);
        const opacity = objectOpacity.get(object.id) ?? 1;
        if (!Number.isFinite(opacity) || opacity < 0 || opacity > 1) throw new RangeError('Invalid object presentation opacity');
        if (opacity !== 1) builder.pushTransform([1, 0, 0, 1, 0, 0], opacity);
        if (layer) layer.contentStart = builder.bytes;
        let plan = plans.get(object.id);
        if (plan && (depth >= 4 || shadowScopes >= 128)) {
            unsupportedShadow(object, 'Composite shadow scope budget exceeded'); plan = null;
        }
        if (plan) shadowScopes++;
        if (plan) pushObjectShadow(builder, plan, packed(object.outerShadow));
        if (object.kind === 'group') {
            for (const child of children(object)) paintObject(child, depth + (plan ? 1 : 0), object.id);
        } else paintLeaf(object);
        if (plan) popObjectShadow(builder);
        if (layer) layer.contentEnd = builder.bytes;
        if (opacity !== 1) builder.popTransform();
        if (layer) layer.end = builder.bytes;
        return layer;
    }
    function paintLeaf(object) {
        if (object.geometry === 'line') {
            const points = linePoints(object);
            if (object.stroke && points.length === 2) { builder.line(...points, object.stroke.width, packed(object.stroke)); painted++; }
            return;
        }
        if (!object.bounds || !object.transform) return;
        const { x, y, width: w, height: h } = object.bounds, rect = [x, y, w, h];
        builder.pushTransform(object.transform);
        if (object.table) {
            paintTable(builder, object, textFrames, null, editingCell); painted++;
        } else if (object.paths?.length) {
            for (const path of object.paths) {
                const id = nextPath++;
                builder.definePath(id, path.commands);
                if (path.fill && object.fill) builder.fillPath(id, packed(object.fill));
                if (path.stroke && object.stroke) builder.strokePath(id, object.stroke.width, packed(object.stroke));
            }
        } else if (object.geometry === 'rect') {
            if (object.fill) builder.rect(rect, packed(object.fill));
            if (object.stroke) {
                const color = packed(object.stroke), stroke = object.stroke.width;
                builder.line([x, y], [x + w, y], stroke, color).line([x + w, y], [x + w, y + h], stroke, color)
                    .line([x + w, y + h], [x, y + h], stroke, color).line([x, y + h], [x, y], stroke, color);
            }
        } else if (object.geometry === 'ellipse') {
            if (object.fill) builder.ellipse(rect, packed(object.fill));
            if (object.stroke) builder.ellipse(rect, packed(object.stroke), { filled: false, width: object.stroke.width });
        } else {
            const shape = shapeById(object.geometry);
            if (shape?.path.length) {
                // Text paths use low IDs; shape resources occupy the upper half.
                const id = nextPath++;
                const path = shape.path.map(([verb, ...values]) => [verb, ...values.map((v, i) => v / 1000 * (i % 2 ? h : w) + (i % 2 ? y : x))]);
                builder.definePath(id, path);
                if (object.fill) builder.fillPath(id, packed(object.fill));
                if (object.stroke) builder.strokePath(id, object.stroke.width, packed(object.stroke));
            }
        }
        const picture = imageFrames.get(object.id);
        if (picture && object.image) {
            const { left, top, right, bottom } = object.image.crop;
            builder.pushClip(rect).image(textures.get(picture.key), rect, { uv: [left, top, 1 - right, 1 - bottom] }).popClip();
            painted++;
        }
        const text = textFrames.get(object.id);
        if (text && !text.placeholder) {
            const clip = textClipRect(scene, object);
            if (clip) builder.pushClip(clip);
            builder.append(text.displayList);
            if (clip) builder.popClip();
        }
        builder.popTransform();
        if (object.fill || object.stroke) painted++;
    }
    for (const object of roots) {
        const start = builder.bytes;
        const layer = paintObject(object);
        if (captureLayers) objectLayers.push({ id: object.id, start, end: builder.bytes, contentStart: layer.contentStart, contentEnd: layer.contentEnd });
    }
    // Editor prompts, cell selection and handles never cast document shadows.
    for (const object of projected) {
        if (!object.bounds || !object.transform || object.kind === 'group' ||
            (!placeholderGuides && selectedCell?.object.id !== object.id)) continue;
        const text = textFrames.get(object.id);
        if (!(placeholderGuides && (text?.placeholder || object.placeholder && !object.text?.text)) &&
            selectedCell?.object.id !== object.id) continue;
        const { x, y, width: w, height: h } = object.bounds;
        builder.pushTransform(object.transform);
        if (object.table) paintTableSelection(builder, object, selectedCell);
        if (placeholderGuides && text?.placeholder) {
            const clip = textClipRect(scene, object);
            if (clip) builder.pushClip(clip);
            builder.append(text.displayList);
            if (clip) builder.popClip();
        }
        if (placeholderGuides && object.placeholder && !object.text?.text) {
            const points = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
            for (let edge = 0; edge < 4; edge++) {
                const start = points[edge], end = points[(edge + 1) % 4];
                const length = Math.hypot(end[0] - start[0], end[1] - start[1]);
                const count = Math.max(1, Math.min(256, Math.ceil(length * ratio / 8)));
                for (let part = 0; part < count; part++) builder.line(
                    start.map((value, axis) => value + (end[axis] - value) * part / count),
                    start.map((value, axis) => value + (end[axis] - value) * (part + .5) / count), 1 / ratio, 0xff999999);
            }
        }
        builder.popTransform();
    }
    for (const object of projected.filter(object => selected.includes(object.id))) {
        const points = objectCorners(object);
        if (!points.length) continue;
        for (let index = 0; index < (points.length === 2 ? 1 : 4); index++) builder.line(points[index], points[(index + 1) % points.length], 1.5 / ratio, 0xffc47b25);
        const handle = rotationHandle(object, ratio);
        if (handle) {
            builder.line(handle.top, handle.point, 1.5 / ratio, 0xffc47b25);
            builder.ellipse([handle.point[0]-5/ratio, handle.point[1]-5/ratio, 10/ratio, 10/ratio], 0xffffffff);
            builder.ellipse([handle.point[0]-5/ratio, handle.point[1]-5/ratio, 10/ratio, 10/ratio], 0xffc47b25, { filled: false, width: 1.5/ratio });
        }
        if (canResize(object)) for (const [x, y] of points) {
            builder.rect([x - 3 / ratio, y - 3 / ratio, 6 / ratio, 6 / ratio], 0xffc47b25);
        }
    }
    builder.popTransform();
    return { frame: builder.finish(), painted, ratio, diagnostics, ...(captureLayers ? { layers: { resourceStart, resourceEnd, backgroundEnd, objects: objectLayers, tree: layerTree } } : {}) };
}
