import {openImageEngine} from './image-engine.mjs';
import {SceneBuilder} from './scene-builder.mjs';
import {SceneSurface,Container} from './controls.mjs';
/** A packaged SVG/PNG icon rendered through the same native image pipeline as
 * app artwork. Pass a shared engine to decode several icons efficiently. */
export async function createIconSurface({services,path,size=24,engine,...properties}={}){
    if(typeof path!=='string'||! /^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]+\.(svg|png)$/.test(path))throw new TypeError('Expected a packaged SVG/PNG icon path');
    if(!Number.isInteger(size)||size<8||size>256)throw new RangeError('Icon size must be 8..256');
    const stat=await services.assets.stat({path});
    if(!Number.isInteger(stat.size)||stat.size<1||stat.size>1024*1024)throw new RangeError('Icon file exceeds 1 MiB');
    const bytes=await services.assets.read({path,offset:0,length:stat.size});
    const decoder=engine||await openImageEngine({services});
    try{
        const image=await decoder.decode(bytes,{maximumWidth:size,maximumHeight:size,maximumPixels:size*size});
        const surface=new SceneSurface({backgroundColor:'transparent'}),scale=Math.min(size/image.width,size/image.height);
        const width=image.width*scale,height=image.height*scale;
        surface.submit(new SceneBuilder(size,size).defineImage(1,image.width,image.height,image.rgba)
            .image(1,[(size-width)/2,(size-height)/2,width,height]).finish());
        return new Container({text:`panel:icon:fixed:flush:width=${size}:accessory-height=${size}`,...properties},[surface]);
    }finally{if(!engine)await decoder.close();}
}
createIconSurface.controlFactory = Object.freeze({
    options: ['services', 'path', 'size', 'engine'],
    guidance: 'Async factory returning a native Container and SceneSurface for a packaged SVG/PNG. Bundle image resources, pass services and the project-relative asset path, and optionally share openImageEngine across icons. Review with native visual inspection.',
});
