// Emerald materialization samples the rendered application. No predetermined
// particle targets: changing a label, icon or layout changes the resulting cloud.
const clamp = t => Math.max(0, Math.min(1, t));
const smooth = t => { t=clamp(t); return t*t*(3-2*t); };
const glyphs = '01011001{}[]<>/\\|:;+=*';
const canvasFor = (w,h) => {
    const canvas = typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(w,h) : document.createElement('canvas');
    canvas.width=w;canvas.height=h;return canvas;
};
export function paintMatrixFrame(context, rect, surface, active = true) {
    const {x,y,width,height}=rect;
    context.save();
    const edge=context.createLinearGradient(x,y,x+width*.65,y+height);
    edge.addColorStop(0,active?'#b5ffe57a':'#b5ffe52a');
    edge.addColorStop(.35,'#56dba524');edge.addColorStop(1,'#56dba508');
    context.strokeStyle=edge;context.lineWidth=1;
    context.beginPath();context.roundRect(x+.5,y+.5,width-1,height-1,Math.min(10,height/4));context.stroke();
    context.restore();
}
export function createMatrixMorph(source) {
    const width=source.width,height=source.height;
    // Bounded readback, once per transition. Sample text/icon edges rather than
    // filling empty window backgrounds with arbitrary glyphs.
    const scale=Math.min(1,220/width,150/height);
    const sample=canvasFor(Math.max(1,Math.round(width*scale)),Math.max(1,Math.round(height*scale)));
    const ctx=sample.getContext('2d',{willReadFrequently:true});ctx.drawImage(source,0,0,sample.width,sample.height);
    let data;
    try { data=ctx.getImageData(0,0,sample.width,sample.height).data; }
    catch { return {source,width,height,points:[]}; }
    const points=[];
    const light=i=>data[i]*.2126+data[i+1]*.7152+data[i+2]*.0722;
    for(let y=1;y<sample.height-1;y++)for(let x=1;x<sample.width-1;x++) {
        const i=(y*sample.width+x)*4,l=light(i);
        const contrast=Math.max(Math.abs(l-light(i-4)),Math.abs(l-light(i+4)),Math.abs(l-light(i-sample.width*4)),Math.abs(l-light(i+sample.width*4)));
        if(data[i+3]<32 || contrast<17 || l<42)continue;
        const seed=((x*73856093)^(y*19349663))>>>0;
        points.push({x:x/scale,y:y/scale,r:data[i],g:data[i+1],b:data[i+2],seed,delay:(seed%101)/101*.16});
    }
    // Uniform thinning preserves coverage across the entire content.
    const stride=Math.max(1,Math.ceil(points.length/2600));
    return {source,width,height,points:points.filter((_,i)=>i%stride===0)};
}
export function paintMatrixMorph(context, frame, progress, model, {background=null, veil=false, particlesOnly=false}={}) {
    const t=clamp(progress),{x,y,width,height}=frame;
    if(t>=1)return;
    context.save();
    // Restore the complete rectangular capture BEFORE clipping particles. A
    // rounded clip here leaves the already-painted antialiased rim behind.
    if(background)context.drawImage(background,x,y,width,height);
    context.beginPath();context.rect(x,y,width,height);context.clip();
    const resolve=smooth((t-.64)/.34),alpha=context.globalAlpha;
    if(veil){context.fillStyle=`rgba(2,13,10,${1-resolve})`;context.fillRect(x,y,width,height);}
    else if(model.source && !particlesOnly){context.globalAlpha*=resolve;context.drawImage(model.source,x,y,width,height);context.globalAlpha=alpha;}
    // Drawing alpha is inherited from the compositor (including inactive opacity).
    context.globalAlpha=alpha;
    const sx=width/model.width,sy=height/model.height;
    context.textAlign='center';context.textBaseline='middle';
    const gather=smooth((t-.08)/.62),fade=1-smooth((t-.70)/.28);
    for(const p of model.points) {
        const local=smooth((gather-p.delay)/(1-p.delay));
        const column=Math.round(p.x/11)*11;
        const startY=-20-(p.seed%Math.max(1,Math.round(height)))*.7;
        const travel=startY+t*height*1.35;
        const px=column+(p.x-column)*local;
        const py=travel+(p.y-travel)*local;
        const size=11-7*local;
        context.font=`${size}px monospace`;
        context.globalAlpha=alpha*fade*(.45+.55*local)*smooth(t/.10);
        context.fillStyle=local>.7?`rgb(${p.r},${p.g},${p.b})`:(p.seed%9===0?'#cefff0':'#48db9e');
        if(local>.94)context.fillRect(x+px*sx,y+py*sy,Math.max(1,2*sx),Math.max(1,2*sy));
        else context.fillText(glyphs[(p.seed+Math.floor(t*19))%glyphs.length],x+px*sx,y+py*sy);
    }
    context.restore();
}

// Popup fallback: samples the actual just-painted menu, never a canned cloud.
export function paintMatrixCurtain(context, frame, reveal, accent, progress=reveal) {
    if(progress>=1)return;
    const source=canvasFor(Math.max(1,Math.ceil(frame.width)),Math.max(1,Math.ceil(frame.height)));
    source.getContext('2d').drawImage(context.canvas,frame.x,frame.y,frame.width,frame.height,0,0,source.width,source.height);
    paintMatrixMorph(context,frame,progress,createMatrixMorph(source),{veil:true});
}

/** DOM menus use their real visible text and control positions as particle targets. */
export function matrixDOMSource(element) {
    const bounds=element.getBoundingClientRect(),source=canvasFor(Math.max(1,Math.round(element.clientWidth)),Math.max(1,Math.round(element.clientHeight)));
    const ctx=source.getContext('2d');ctx.fillStyle='#020d0a';ctx.fillRect(0,0,source.width,source.height);
    const walk=element.ownerDocument.createTreeWalker(element,NodeFilter.SHOW_TEXT);
    while(walk.nextNode()) {
        const node=walk.currentNode;if(!node.textContent.trim() || ['STYLE','SCRIPT'].includes(node.parentElement?.tagName))continue;
        const style=getComputedStyle(node.parentElement);if(style.display==='none'||style.visibility==='hidden')continue;
        const range=element.ownerDocument.createRange();range.selectNodeContents(node);
        const rect=range.getBoundingClientRect();if(!rect.width||rect.bottom<bounds.top||rect.top>bounds.bottom)continue;
        ctx.font=style.font;ctx.fillStyle=style.color;ctx.textBaseline='top';
        ctx.fillText(node.textContent.trim(),rect.left-bounds.left,rect.top-bounds.top,rect.width);
    }
    return source;
}
