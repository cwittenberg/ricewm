import { inverseAffine, transformRect } from './affine.mjs';

const IDENTITY = Object.freeze([1,0,0,1,0,0]);
const origins = { tl:[0,0],t:[.5,0],tr:[1,0],l:[0,.5],ctr:[.5,.5],r:[1,.5],bl:[0,1],b:[.5,1],br:[1,1] };

/** Gaussian moment approximation of a triangular stack-blur radius. Keep the
 * original document radius untouched. Independent LibreOffice raster checks
 * bound the approximation; this is not an asserted PowerPoint kernel match. */
export const shadowSigma = radius => Math.sqrt(radius * (radius + 2) / 6);

/** Shadow scopes capture actual foreground alpha. A fixed shadow uses page
 * axes while retaining the rotated silhouette; a rotating shadow uses the
 * object's axes. Its affine never changes the enclosed foreground commands. */
export function objectShadowPlan(object, worldInk, localInk = null) {
    const shadow=object.outerShadow;
    if (!shadow || !worldInk || shadow.opacity===0 || shadow.scaleX===0 || shadow.scaleY===0) return null;
    const scope=shadow.rotateWithShape ? object.transform : IDENTITY;
    const inverse=scope && inverseAffine(scope);
    if (!inverse) throw new RangeError('Shadow object transform is singular');
    const bounds=shadow.rotateWithShape && localInk ? localInk : transformRect(inverse,worldInk);
    const b=object.bounds;
    const anchor=shadow.rotateWithShape ? [b.x,b.y,b.width,b.height] : transformRect(object.transform,[b.x,b.y,b.width,b.height]);
    const [ax,ay]=origins[shadow.alignment], x=anchor[0]+ax*anchor[2],y=anchor[1]+ay*anchor[3];
    const radians=Math.PI/180, a=shadow.scaleX,d=shadow.scaleY;
    // Scale then skew, around the alignment origin specified by DrawingML.
    const c=Math.tan(shadow.skewXDegrees*radians)*d, bb=Math.tan(shadow.skewYDegrees*radians)*a;
    const transform=[a,bb,c,d,x-a*x-c*y,y-bb*x-d*y];
    if (Math.fround(a)*Math.fround(d)===Math.fround(bb)*Math.fround(c)) return null;
    const angle=shadow.angleDegrees*radians, offset=[Math.cos(angle)*shadow.distance,Math.sin(angle)*shadow.distance];
    return { scope,inverse,bounds,transform,offset,blur:shadowSigma(shadow.blurRadius) };
}

/** Bounds for a parent's composite-alpha capture include child shadows. */
export function shadowVisualBounds(plan, worldInk) {
    if (!plan) return worldInk;
    const { transform,offset,blur,bounds,scope }=plan;
    const [x,y,w,h]=transformRect(transform,bounds), pad=Math.ceil(4*blur)+1;
    return transformRect(scope,[x+offset[0]-pad,y+offset[1]-pad,w+2*pad,h+2*pad]);
}

export function pushObjectShadow(builder,plan,color) {
    builder.pushTransform(plan.scope).pushDropShadow(plan.bounds,color,
        { transform:plan.transform,offset:plan.offset,blur:plan.blur }).pushTransform(plan.inverse);
}
export function popObjectShadow(builder) { builder.popTransform().popDropShadow().popTransform(); }
