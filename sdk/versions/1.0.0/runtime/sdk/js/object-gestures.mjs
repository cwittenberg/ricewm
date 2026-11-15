import { visualBounds } from './visual-geometry.mjs';
/** View-independent gesture transaction. The host owns pointer capture. */
export class ObjectGestureController {
    static controlFactory = Object.freeze({
        topics: ['visual', 'scene'],
        constructor:
            'new ObjectGestureController({threshold:4,grid:0,onPreview,onCommit,onCancel,acceptDrop})',
        methods:
            'begin({point,objects,mode:"move"|"resize"|"rotate",handle:"nw"|"n"|"ne"|"e"|"se"|"s"|"sw"|"w",targetId?,pivot?:{x,y}}), update(point,{scale,constrain,dropTarget}), finish(), cancel(); targetId resizes only that selected object; an explicit rotation pivot rotates object positions and angles together; omitted pivot retains each object center. Locked objects are excluded. Corner and edge resizing keeps the opposite edge/corner fixed through rotation; one commit per completed gesture',
    });
    constructor({
        threshold = 4,
        grid = 0,
        onPreview = () => {},
        onCommit = () => {},
        onCancel = () => {},
        acceptDrop = () => true,
    } = {}) {
        if (
            ![threshold, grid].every((v) => Number.isFinite(v) && v >= 0) ||
            ![onPreview, onCommit, onCancel, acceptDrop].every((f) => typeof f === 'function')
        )
            throw TypeError('Invalid gesture options');
        Object.assign(this, { threshold, grid, onPreview, onCommit, onCancel, acceptDrop });
        this.active = null;
    }
    begin({ point, objects, mode = 'move', handle = 'se', targetId = null, pivot = null } = {}) {
        if (
            !point ||
            ![point.x, point.y].every(Number.isFinite) ||
            !Array.isArray(objects) ||
            !objects.length ||
            !['move', 'resize', 'rotate'].includes(mode) ||
            !['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'].includes(handle) ||
            (targetId !== null && (mode !== 'resize' || typeof targetId !== 'string' || !objects.some(o => o.id === targetId))) ||
            (pivot !== null && (mode !== 'rotate' || ![pivot.x, pivot.y].every(Number.isFinite)))
        )
            throw TypeError('Invalid gesture');
        this.cancel();
        this.active = {
            point: { ...point },
            objects: objects.filter((o) => !o.locked && (targetId === null || o.id === targetId)).map((o) => ({ ...o, ...visualBounds(o) })),
            mode,
            handle,
            pivot: pivot && { x: pivot.x, y: pivot.y },
            moved: false,
            preview: null,
            dropTarget: null,
        };
        return this.active;
    }
    update(point, { scale = 1, constrain = false, dropTarget = null } = {}) {
        const a = this.active;
        if (!a) return null;
        if (![point?.x, point?.y, scale].every(Number.isFinite) || scale <= 0)
            throw TypeError('Invalid gesture point');
        let dx = point.x - a.point.x,
            dy = point.y - a.point.y;
        if (!a.moved && Math.hypot(dx, dy) * scale < this.threshold) return null;
        a.moved = true;
        if (a.mode === 'move' && constrain) {
            if (Math.abs(dx) > Math.abs(dy)) dy = 0;
            else dx = 0;
        }
        const snap = (value) => (this.grid ? Math.round(value / this.grid) * this.grid : value);
        let rotationDelta = a.pivot ? (Math.atan2(point.y-a.pivot.y,point.x-a.pivot.x)-Math.atan2(a.point.y-a.pivot.y,a.point.x-a.pivot.x))*180/Math.PI : null;
        a.preview = a.objects.map((o) => {
            if (a.mode === 'move') {
                const anchor = a.objects[0];
                return {
                    ...o,
                    x: o.x + snap(anchor.x + dx) - anchor.x,
                    y: o.y + snap(anchor.y + dy) - anchor.y,
                };
            }
            if (a.mode === 'rotate') {
                if (a.pivot) {
                    let rotation = (o.rotation || 0) + rotationDelta;
                    if (constrain) { rotation = Math.round(rotation / 15) * 15; rotationDelta = rotation - (o.rotation || 0); }
                    const radians = rotationDelta * Math.PI / 180, cos = Math.cos(radians), sin = Math.sin(radians),
                        x = o.x + o.width / 2 - a.pivot.x, y = o.y + o.height / 2 - a.pivot.y;
                    return { ...o, rotation, x: a.pivot.x + cos*x - sin*y - o.width/2, y: a.pivot.y + sin*x + cos*y - o.height/2 };
                }
                const cx = o.x + o.width / 2,
                    cy = o.y + o.height / 2,
                    angle =
                        ((Math.atan2(point.y - cy, point.x - cx) -
                            Math.atan2(a.point.y - cy, a.point.x - cx)) *
                            180) /
                            Math.PI +
                        (o.rotation || 0);
                return { ...o, rotation: constrain ? Math.round(angle / 15) * 15 : angle };
            }
            const angle = ((o.rotation || 0) * Math.PI) / 180,
                cos = Math.cos(angle),
                sin = Math.sin(angle),
                localX = cos * dx + sin * dy,
                localY = -sin * dx + cos * dy,
                west = a.handle.includes('w'),
                north = a.handle.includes('n'),
                horizontal = west || a.handle.includes('e'),
                vertical = north || a.handle.includes('s');
            let width = horizontal ? Math.max(16, snap(o.width + (west ? -localX : localX))) : o.width,
                height = vertical ? Math.max(16, snap(o.height + (north ? -localY : localY))) : o.height;
            if (constrain) {
                const ratio = o.width / o.height;
                if(!horizontal){height=Math.max(height,16/ratio);width=height*ratio;}
                else {width = Math.max(width, 16 * ratio);height = width / ratio;}
            }
            // Keep the opposite corner fixed, including rotated geometry.
            const shiftX = horizontal ? ((width - o.width) * (west ? -1 : 1)) / 2 : 0,
                shiftY = vertical ? ((height - o.height) * (north ? -1 : 1)) / 2 : 0;
            return {
                ...o,
                width,
                height,
                x: o.x + o.width / 2 + cos * shiftX - sin * shiftY - width / 2,
                y: o.y + o.height / 2 + sin * shiftX + cos * shiftY - height / 2,
            };
        });
        a.dropTarget = dropTarget && this.acceptDrop(a.objects, dropTarget) ? dropTarget : null;
        this.onPreview(a.preview, { mode: a.mode, dropTarget: a.dropTarget });
        return a.preview;
    }
    finish() {
        const a = this.active;
        this.active = null;
        if (!a?.moved || !a.preview) {
            this.onCancel();
            return false;
        }
        this.onCommit(a.preview, { mode: a.mode, dropTarget: a.dropTarget });
        return true;
    }
    cancel() {
        if (!this.active) return false;
        this.active = null;
        this.onCancel();
        return true;
    }
    destroy() {
        this.cancel();
    }
}
