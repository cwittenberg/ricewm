// Bounded retained panel actor rasters. A frame is owned by an enabled realm;
// it carries geometry and pixels from the same original actor allocation.
export function riceGnomePanelSymbolLayout(frame) {
    if(Object.hasOwn(frame,'symbolLayout'))return frame.symbolLayout;
    let rgb=null,left=frame.width,top=frame.height,right=-1,bottom=-1;
    for(let i=0;i<frame.pixels.length;i++){
        const pixel=frame.pixels[i],alpha=pixel>>>24;
        if(!alpha)continue;
        if(alpha===255){if(rgb!==null&&rgb!==(pixel&0xffffff))return frame.symbolLayout=null;rgb=pixel&0xffffff;}
        const x=i%frame.width,y=Math.floor(i/frame.width);
        left=Math.min(left,x);right=Math.max(right,x);top=Math.min(top,y);bottom=Math.max(bottom,y);
    }
    if(rgb===null||right<left)return frame.symbolLayout=null;
    const width=right-left+1,height=bottom-top+1,scale=Math.min(1,16/height);
    return frame.symbolLayout={source:{x:left,y:top,width,height},width:Math.max(32,Math.ceil(width*scale)+16),
        glyphWidth:width*scale,glyphHeight:height*scale,color:[rgb>>>16,(rgb>>>8)&255,rgb&255,255]};
}

export function readRiceGnomePanelFrames(values, previous=new Map()) {
    if(!Array.isArray(values)||values.length>16)throw new TypeError('Invalid GNOME panel frames');
    const result=new Map();let bytes=0;
    for(const value of values){
        const {uuid,actorId,width,height,pixels,hits,revision,generation}=value||{};
        const key=value?.key||uuid;
        if(typeof uuid!=='string'||!uuid||uuid.length>128||typeof key!=='string'||!key||key.length>128||result.has(key)||
            !Number.isSafeInteger(generation)||generation<1||!Number.isSafeInteger(actorId)||actorId<1||
            !Number.isInteger(width)||width<1||width>2048||!Number.isInteger(height)||height<1||height>128||
            !(pixels instanceof Uint32Array)||pixels.length!==width*height||!Array.isArray(hits)||hits.length>2048||
            !Number.isSafeInteger(revision)||revision<1)throw new TypeError('Invalid GNOME panel frame');
        bytes+=pixels.byteLength;if(bytes>16*1024*1024)throw new RangeError('GNOME panel frame budget exceeded');
        for(const hit of hits)if(!Number.isSafeInteger(hit.actorId)||hit.actorId<1||
            !['x','y','width','height'].every(key=>Number.isFinite(hit[key]))||hit.x<0||hit.y<0||hit.width<=0||hit.height<=0||
            hit.x+hit.width>width||hit.y+hit.height>height)throw new TypeError('Invalid GNOME panel hit');
        const old=previous.get(key);
        result.set(key,{...value,key,surface:old?.uuid===uuid&&old.generation===generation&&old.revision===revision&&old.actorId===actorId&&old.width===width&&old.height===height?old.surface:null});
    }
    return result;
}

export function riceGnomePanelCanvas(frame, factory, tint=null) {
    if(tint&&tint.from.slice(0,3).some((c,i)=>c!==tint.to[i])){
        const key=tint.from.join(',')+':'+tint.to.join(',');
        if(frame.tint?.key===key)return frame.tint.surface;
        // Only a uniform symbolic/text mask can follow the wallpaper ink.
        // Active button backgrounds and original multicolor artwork stay intact.
        const rgb=(tint.from[0]<<16)|(tint.from[1]<<8)|tint.from[2];
        if(!frame.pixels.some(pixel=>(pixel>>>24)===255&&(pixel&0xffffff)!==rgb)){
            const surface=factory(frame.width,frame.height),context=surface.getContext('2d'),image=context.createImageData(frame.width,frame.height);
            for(let i=0;i<frame.pixels.length;i++){
                const offset=i*4;
                image.data[offset]=tint.to[0];image.data[offset+1]=tint.to[1];image.data[offset+2]=tint.to[2];image.data[offset+3]=frame.pixels[i]>>>24;
            }
            context.putImageData(image,0,0);frame.tint={key,surface};return surface;
        }
        const surface=riceGnomePanelCanvas(frame,factory);frame.tint={key,surface};return surface;
    }
    if(frame.surface)return frame.surface;
    const canvas=factory(frame.width,frame.height),context=canvas.getContext('2d'),image=context.createImageData(frame.width,frame.height);
    for(let i=0;i<frame.pixels.length;i++){
        const pixel=frame.pixels[i];image.data.set([(pixel>>>16)&255,(pixel>>>8)&255,pixel&255,pixel>>>24],i*4);
    }
    context.putImageData(image,0,0);return frame.surface=canvas;
}
