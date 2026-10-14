import { multiplyAffine } from './affine.mjs';
export { multiplyAffine, inverseAffine } from './affine.mjs';
/** Conservative one-to-one correspondence. Explicit !! names never fall back
 * to ordinary matching; ambiguous identities remain separate fading objects. */
export function matchMorphObjects(before, after) {
    const matches = new Map(), used = new Set();
    const forced = object => object.morphSource?.name?.startsWith('!!');
    function pass(key) {
        const left = new Map(), right = new Map();
        for (const [items, map, excluded] of [[before, left, object => used.has(object.id)], [after, right, object => matches.has(object.id)]]) {
            for (const object of items) {
                if (excluded(object)) continue;
                const value = key(object); if (!value) continue;
                const identity = `${object.kind}\0${value}`;
                map.set(identity, map.has(identity) ? null : object);
            }
        }
        for (const [identity, old] of left) {
            const next = right.get(identity);
            if (old && next) { matches.set(next.id, old); used.add(old.id); }
        }
    }
    pass(object => forced(object) ? object.morphSource.name : null);
    pass(object => !forced(object) ? object.morphSource?.creationId : null);
    pass(object => {
        const source = object.morphSource;
        return !forced(object) && source?.sourceId && source?.name ? `${source.sourceId}\0${source.name}` : null;
    });
    return { matches, unmatched: before.filter(object => !used.has(object.id)) };
}

export function objectAffine(object) {
    const b = object.bounds, m = object.transform;
    return b && m && b.width > 0 && b.height > 0
        ? multiplyAffine(m, [b.width, 0, 0, b.height, b.x, b.y]) : null;
}
/** Decompose rotation so a half-turn does not collapse a shape to a line. */
export function interpolateAffine(from, to, progress) {
    const parts = m => {
        const sx = Math.hypot(m[0], m[1]);
        if (sx < 1e-10) return null;
        return [Math.atan2(m[1], m[0]), sx, (m[0]*m[3]-m[1]*m[2])/sx,
            (m[0]*m[2]+m[1]*m[3])/sx, m[4], m[5]];
    };
    const a = parts(from), b = parts(to), lerp = (x,y) => x+(y-x)*progress;
    if (!a || !b) return from.map((x,i) => lerp(x,to[i]));
    const turn = Math.atan2(Math.sin(b[0]-a[0]), Math.cos(b[0]-a[0]));
    const angle = a[0]+turn*progress, sx=lerp(a[1],b[1]), sy=lerp(a[2],b[2]), shear=lerp(a[3],b[3]);
    const c=Math.cos(angle), s=Math.sin(angle);
    const linear=[c*sx,s*sx,c*shear-s*sy,s*shear+c*sy];
    // Interpolate the visual center, not the rotated top-left origin. A
    // stationary shape must rotate in place rather than orbit its corner.
    const center=[4,5].map((offset,i)=>lerp(from[offset]+(from[i]+from[i+2])/2,to[offset]+(to[i]+to[i+2])/2));
    return [...linear,center[0]-(linear[0]+linear[2])/2,center[1]-(linear[1]+linear[3])/2];
}
