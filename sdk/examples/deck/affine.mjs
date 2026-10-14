/** Shared object/scene affine geometry. */
export const multiplyAffine = (a, b) => [
    a[0]*b[0]+a[2]*b[1], a[1]*b[0]+a[3]*b[1], a[0]*b[2]+a[2]*b[3], a[1]*b[2]+a[3]*b[3],
    a[0]*b[4]+a[2]*b[5]+a[4], a[1]*b[4]+a[3]*b[5]+a[5],
];
export function inverseAffine(a) {
    const determinant = a[0]*a[3]-a[1]*a[2];
    if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-10) return null;
    return [a[3]/determinant, -a[1]/determinant, -a[2]/determinant, a[0]/determinant,
        (a[2]*a[5]-a[3]*a[4])/determinant, (a[1]*a[4]-a[0]*a[5])/determinant];
}
export const transformPoint = (m, [x,y]) => [m[0]*x+m[2]*y+m[4], m[1]*x+m[3]*y+m[5]];
export function transformRect(m, rect) {
    if (!rect) return null;
    const [x,y,w,h]=rect, points=[[x,y],[x+w,y],[x,y+h],[x+w,y+h]].map(p=>transformPoint(m,p));
    const xs=points.map(p=>p[0]), ys=points.map(p=>p[1]);
    const left=Math.min(...xs), top=Math.min(...ys);
    return [left,top,Math.max(...xs)-left,Math.max(...ys)-top];
}
export function unionRects(a,b) {
    if (!a) return b; if (!b) return a;
    const x=Math.min(a[0],b[0]), y=Math.min(a[1],b[1]);
    return [x,y,Math.max(a[0]+a[2],b[0]+b[2])-x,Math.max(a[1]+a[3],b[1]+b[3])-y];
}
