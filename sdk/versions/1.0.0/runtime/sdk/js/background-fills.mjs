import {checkedResolvedPaint} from './color-references.mjs';
/** Shared background paint values and bounded scene rendering. Persisted
 * gradients and patterns remain editable paints; sampled pixels are previews. */
export const BACKGROUND_PATTERNS = Object.freeze({
    pct5: [0x80,0,0,0,0x08,0,0,0], pct20: [0x88,0,0x22,0,0x88,0,0x22,0],
    pct50: [0xaa,0x55,0xaa,0x55,0xaa,0x55,0xaa,0x55],
    ltHorz: [255,0,0,0,255,0,0,0], ltVert: Array(8).fill(0x88),
    smGrid: [255,0x88,0x88,0x88,255,0x88,0x88,0x88],
    dkDnDiag: [0xcc,0x66,0x33,0x99,0xcc,0x66,0x33,0x99],
    wave: [0,0x18,0x25,0xc0,0,0x18,0x25,0xc0],
    dotGrid: [0xaa,0,0x80,0,0x80,0,0x80,0], weave: [0x88,0x54,0x22,0x45,0x88,0x14,0x22,0x51],
});
for (const tile of Object.values(BACKGROUND_PATTERNS)) Object.freeze(tile);
const exact = (value, keys, optional = []) => value && typeof value === 'object' && !Array.isArray(value) &&
    keys.every(key => Object.hasOwn(value,key)) && Object.keys(value).every(key => keys.includes(key) || optional.includes(key));
const between = (v,lo,hi) => typeof v === 'number' && Number.isFinite(v) && v >= lo && v <= hi;
const paint = v => {try{checkedResolvedPaint(v);return true;}catch{return false;}};
const stopPaint = ({offset,...value})=>value;
export function checkedGradientValue(value) {
    if (!exact(value,['kind','angle','stops'],['focus','scaled','tile']) || !['linear','radial','rectangular','path'].includes(value.kind) ||
        !between(value.angle,-36000,36000) || !Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 64 ||
        value.stops.some(stop => !exact(stop,['offset','color','opacity'],['source']) || !paint(stopPaint(stop)) || !between(stop.offset,0,1)) ||
        value.focus !== undefined && (!Array.isArray(value.focus) || value.focus.length !== 4 || value.focus.some(v=>!between(v,0,1))) ||
        value.tile !== undefined && (!Array.isArray(value.tile) || value.tile.length !== 4 || value.tile.some(v=>!between(v,-10,10))) ||
        value.scaled !== undefined && typeof value.scaled !== 'boolean') throw TypeError('Invalid gradient');
    return {kind:value.kind,angle:value.angle,stops:value.stops.map(s=>({offset:s.offset,...checkedResolvedPaint(stopPaint(s))})).sort((a,b)=>a.offset-b.offset),
        ...(value.focus ? {focus:[...value.focus]} : {}), ...(value.tile ? {tile:[...value.tile]} : {}),
        ...(value.scaled === undefined ? {} : {scaled:value.scaled})};
}
/** Canonical DrawingML persistence geometry. Source colour transforms remain
 * untouched; this is an owner-selected precision policy, never a preview edit. */
export function checkedDrawingmlGradientValue(raw) {
    const value=checkedGradientValue(raw),percent=n=>Math.round(n*100000)/100000;
    value.stops=value.stops.map(stop=>({...stop,offset:percent(stop.offset),opacity:stop.source?stop.opacity:percent(stop.opacity)}));
    if(value.tile)value.tile=value.tile.map(percent);
    if(value.kind==='linear'){
        value.angle=(Math.round(((value.angle%360+360)%360)*60000)%21600000)/60000;
        value.scaled=value.scaled??false;delete value.focus;
    }else{
        value.angle=0;value.focus=(value.focus??[.5,.5,.5,.5]).map(percent);delete value.scaled;
    }
    return checkedGradientValue(value);
}
export function checkedBackgroundFill(value) {
    if (value?.kind === 'solid' && exact(value,['kind','color','opacity'],['source']) && paint((({kind,...p})=>p)(value)))
        return {kind:'solid',...checkedResolvedPaint((({kind,...p})=>p)(value))};
    if (value?.kind === 'gradient' && exact(value,['kind','gradient'])) return {kind:'gradient',gradient:checkedGradientValue(value.gradient)};
    if (value?.kind === 'pattern' && exact(value,['kind','preset','foreground','background']) && Object.hasOwn(BACKGROUND_PATTERNS,value.preset) && paint(value.foreground) && paint(value.background))
        return {...value,foreground:checkedResolvedPaint(value.foreground),background:checkedResolvedPaint(value.background)};
    throw TypeError('Invalid background fill');
}
const channels = p => [1,3,5].map(i=>parseInt(p.color.slice(i,i+2),16)).concat(Math.round(p.opacity*255));
const clamp = v => Math.max(0,Math.min(1,v));
const linear = n => n <= .04045 ? n/12.92 : ((n+.055)/1.055)**2.4;
const srgb = n => (n <= .0031308 ? 12.92*n : 1.055*n**(1/2.4)-.055)*255;
// Measured two-colour DrawingML ramp. Multi-stop paints interpolate in sRGB.
const weights = [0,.0205,.0469,.0878,.1431,.2143,.2989,.3972,.5,.6028,.7011,.7857,.8569,.9122,.9531,.9795,1];
export function gradientColorAt(gradient, position) {
    const stops=gradient.stops, t=clamp(position);
    let i=0; while(i<stops.length-1 && stops[i+1].offset<=t)i++;
    const a=stops[i], b=stops[Math.min(i+1,stops.length-1)], ca=channels(a), cb=channels(b);
    let share=b.offset===a.offset?0:clamp((t-a.offset)/(b.offset-a.offset));
    const special=stops.length===2 && stops[0].offset<=1e-6 && stops[1].offset>=1-1e-6;
    if(special){const x=share*16,j=Math.min(15,Math.floor(x));share=weights[j]+(weights[j+1]-weights[j])*(x-j);}
    return ca.map((v,k)=>Math.round(special && k<3 ? srgb(linear(v/255)*(1-share)+linear(cb[k]/255)*share) : v*(1-share)+cb[k]*share));
}
export function gradientPosition(gradient,x,y,width,height) {
    const tile=gradient.tile || [0,0,0,0]; let left=tile[0]*width,top=tile[1]*height,right=(1-tile[2])*width,bottom=(1-tile[3])*height;
    if(right<=left || bottom<=top){left=top=0;right=width;bottom=height;}
    if(gradient.kind==='linear'){
        let angle=gradient.angle*Math.PI/180;
        if(gradient.scaled)angle=Math.atan2(height*Math.sin(angle),width*Math.cos(angle));
        const dx=Math.cos(angle),dy=Math.sin(angle),span=(right-left)*Math.abs(dx)+(bottom-top)*Math.abs(dy);
        return clamp(.5+((x-(left+right)/2)*dx+(y-(top+bottom)/2)*dy)/Math.max(span,1e-9));
    }
    const f=gradient.focus || [.5,.5,.5,.5]; let x0=f[0]*width,y0=f[1]*height,x1=(1-f[2])*width,y1=(1-f[3])*height;
    if(x1<x0 || y1<y0){x0=x1=(x0+x1)/2;y0=y1=(y0+y1)/2;}
    if(gradient.kind==='radial'){
        const cx=(left+right)/2,cy=(top+bottom)/2,r=Math.max(1e-6,Math.hypot(right-left,bottom-top)/2);
        let fx=(x0+x1)/2,fy=(y0+y1)/2,dist=Math.hypot(fx-cx,fy-cy);
        if(dist>r*.999){fx=cx+(fx-cx)*r*.999/dist;fy=cy+(fy-cy)*r*.999/dist;}
        const vx=x-fx,vy=y-fy,dx=cx-fx,dy=cy-fy;
        const a=dx*dx+dy*dy-r*r,b=-2*(vx*dx+vy*dy),c=vx*vx+vy*vy;
        return clamp((-b-Math.sqrt(Math.max(0,b*b-4*a*c)))/(2*a));
    }
    // A slide's outline is its rectangle, so Path and Rectangular coincide.
    const axis=(v,lo,hi,max)=>v<lo?(lo-v)/Math.max(lo,1e-9):v>hi?(v-hi)/Math.max(max-hi,1e-9):0;
    return clamp(Math.max(axis(x,x0,x1,width),axis(y,y0,y1,height)));
}
const cache=new Map();
/** Budgeted preview sampling; at most four immutable rasters are retained. */
export function backgroundFillRaster(raw,width,height) {
    const fill=checkedBackgroundFill(raw);
    if(!between(width,1e-6,1e6)||!between(height,1e-6,1e6))throw RangeError('Invalid background dimensions');
    const key=JSON.stringify([fill,width,height]);if(cache.has(key))return cache.get(key);
    if(fill.kind==='pattern'&&(width>16384||height>32768))throw RangeError('Background pattern exceeds scene dimensions');
    let w,h;
    if(fill.kind==='pattern'){w=Math.min(16384,Math.max(8,Math.ceil(width)));h=8;}
    else{const scale=Math.min(1,768/Math.max(width,height));w=Math.max(1,Math.round(width*scale));h=Math.max(1,Math.round(height*scale));}
    const rgba=new Uint8Array(w*h*4),fg=fill.kind==='pattern'?channels(fill.foreground):null,bg=fill.kind==='pattern'?channels(fill.background):null;
    // A ramp lookup avoids colour-space exponentiation for every raster pixel.
    const ramp=fill.kind==='gradient'?Array.from({length:4097},(_,i)=>gradientColorAt(fill.gradient,i/4096)):null;
    for(let y=0;y<h;y++)for(let x=0;x<w;x++){
        const color=fill.kind==='pattern'?((BACKGROUND_PATTERNS[fill.preset][y%8]>>(7-x%8))&1?fg:bg)
            :fill.kind==='solid'?channels(fill):ramp[Math.round(gradientPosition(fill.gradient,(x+.5)*width/w,(y+.5)*height/h,width,height)*4096)];
        rgba.set(color,(y*w+x)*4);
    }
    const result={width:w,height:h,rgba};cache.set(key,result);if(cache.size>4)cache.delete(cache.keys().next().value);return result;
}
export function paintBackgroundFill(builder,fill,rect,{imageId=0xfffffffe}={}) {
    const [x,y,width,height]=rect,resource=backgroundFillRaster(fill,width,height);
    builder.defineImage(imageId,resource.width,resource.height,resource.rgba);
    if(fill.kind!=='pattern')builder.image(imageId,rect);
    else{
        const rows=Math.ceil(height/8);if(rows>4096)throw RangeError('Background pattern exceeds scene budget');
        builder.pushClip(rect);
        for(let row=0;row<rows;row++)builder.image(imageId,[x,y+row*8,resource.width,8],{smoothing:false});
        builder.popClip();
    }
    return builder;
}
paintBackgroundFill.controlFactory=Object.freeze({topics:['drawing','documents'],constructor:'paintBackgroundFill(builder,fill,[0,0,width,height])',methods:'Draw an editable background paint preview with bounded cached raster resources. Linear, radial, rectangular and path gradients; ten DrawingML patterns. Does not edit a document or add selectable objects. Reserve the imageId in the caller resource namespace.'});

checkedDrawingmlGradientValue.controlFactory=Object.freeze({topics:['drawing','documents'],constructor:'checkedDrawingmlGradientValue(gradient)',methods:'Canonical DrawingML geometry and coordinate precision. Use as GradientEditor normalizeValue before persistence. Retains source colour transforms and their resolved alpha; performs no document edit.'});
