/** Bounded host-owned silhouettes, expressed in client pixels. */
export function checkedEditorRegions(regions) {
    let count=0;
    if(!Array.isArray(regions)||regions.length<1||regions.length>4)throw TypeError('Invalid editor client regions');
    return Object.freeze(regions.map(r=>{
        if(!r||Object.keys(r).some(k=>!['x','y','width','height','polygons'].includes(k))||
            ![r.x,r.y,r.width,r.height].every(Number.isInteger)||r.x<0||r.y<0||r.width<1||r.height<1||r.x+r.width>4096||r.y+r.height>4096)throw TypeError('Invalid editor client regions');
        let polygons;
        if(r.polygons!==undefined){
            if(!Array.isArray(r.polygons)||!r.polygons.length||r.polygons.length>1024)throw TypeError('Invalid editor polygon');
            polygons=Object.freeze(r.polygons.map(p=>{
                if(!Array.isArray(p)||p.length<3||p.length>1024||(count+=p.length)>8192||p.some(point=>!Array.isArray(point)||point.length!==2||!point.every(n=>Number.isInteger(n)&&Math.abs(n)<=4096)))throw TypeError('Invalid editor polygon');
                return Object.freeze(p.map(point=>Object.freeze([...point])));
            }));
        }
        return Object.freeze({x:r.x,y:r.y,width:r.width,height:r.height,...(polygons?{polygons}:{})});
    }));
}
export function nativeWindowRegions(window) {
    if(!window?.frame)return [];
    const frame=window.frame;if(!window.editorRegions?.length||window.dialog||frame.titleHeight)return [frame];
    return window.editorRegions.map(r=>({x:frame.x+r.x,y:frame.y+r.y,width:Math.max(0,Math.min(r.width,frame.width-r.x)),height:Math.max(0,Math.min(r.height,frame.height-r.y)),
        ...(r.polygons?{polygons:r.polygons.map(p=>p.map(([x,y])=>[frame.x+r.x+x,frame.y+r.y+y]))}:{})})).filter(r=>r.width>0&&r.height>0);
}
export function pointInRegion(rect,x,y) {
    if(x<rect.x||y<rect.y||x>=rect.x+rect.width||y>=rect.y+rect.height)return false;
    if(!rect.polygons)return true;
    return rect.polygons.some(p=>{let inside=false;for(let i=0,j=p.length-1;i<p.length;j=i++){
        const [ax,ay]=p[i],[bx,by]=p[j];if((ay>y)!==(by>y)&&x<(bx-ax)*(y-ay)/(by-ay)+ax)inside=!inside;
    }return inside;});
}
/** Polygon/rectangle intersection for popup occlusion without filling skin holes. */
export function clipRegionPolygon(polygon,rect) {
    let points=polygon;
    for(const [axis,edge,sign]of [[0,rect.x,1],[0,rect.x+rect.width,-1],[1,rect.y,1],[1,rect.y+rect.height,-1]]){
        const result=[];for(let i=0;i<points.length;i++){
            const a=points[i],b=points[(i+1)%points.length],ain=(a[axis]-edge)*sign>=0,bin=(b[axis]-edge)*sign>=0;
            if(ain)result.push(a);if(ain!==bin){const t=(edge-a[axis])/(b[axis]-a[axis]);result.push([a[0]+t*(b[0]-a[0]),a[1]+t*(b[1]-a[1])]);}
        }points=result;if(!points.length)break;
    }return points;
}
