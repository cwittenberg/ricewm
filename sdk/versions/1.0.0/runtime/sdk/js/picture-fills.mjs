/** Bounded, document-independent picture fill geometry in 96-dpi coordinates. */
export const PICTURE_ALIGNMENTS = Object.freeze({tl:'Top Left',t:'Top',tr:'Top Right',l:'Left',ctr:'Center',r:'Right',bl:'Bottom Left',b:'Bottom',br:'Bottom Right'});
export const PICTURE_MIRRORS = Object.freeze({none:'None',x:'Horizontal',y:'Vertical',xy:'Both'});
const anchors={tl:[0,0],t:[.5,0],tr:[1,0],l:[0,.5],ctr:[.5,.5],r:[1,.5],bl:[0,1],b:[.5,1],br:[1,1]};
const number=(value,minimum,maximum)=>typeof value==='number'&&Number.isFinite(value)&&value>=minimum&&value<=maximum;
export function checkedPictureFillSettings(value) {
    if(!value||!['stretch','tile'].includes(value.mode)||!number(value.opacity,0,1)||
        !['scaleX','scaleY'].every(k=>number(value[k],.01,100))||!['offsetX','offsetY'].every(k=>number(value[k],-1e6,1e6))||
        !Object.hasOwn(PICTURE_ALIGNMENTS,value.alignment??'tl')||!Object.hasOwn(PICTURE_MIRRORS,value.mirror??'none'))throw TypeError('Invalid picture fill settings');
    return {mode:value.mode,opacity:value.opacity,scaleX:value.scaleX,scaleY:value.scaleY,offsetX:value.offsetX,offsetY:value.offsetY,alignment:value.alignment??'tl',mirror:value.mirror??'none'};
}
/** Each record has its actual tile index, so clipping never changes the phase
 * of alternate mirrored tiles. The original conservative 4096-tile limit holds. */
export function pictureFillTiles(size,image,value) {
    const settings=checkedPictureFillSettings(value);
    if(!size||!image||!['width','height'].every(k=>number(size[k],Number.MIN_VALUE,1e12)&&number(image[k],Number.MIN_VALUE,8192)))throw TypeError('Invalid picture fill dimensions');
    if(settings.mode==='stretch')return [{rect:[0,0,size.width,size.height],flipX:false,flipY:false,column:0,row:0}];
    const w=image.width*settings.scaleX,h=image.height*settings.scaleY;
    if((Math.ceil(size.width/w)+2)*(Math.ceil(size.height/h)+2)>4096)throw RangeError('Background texture needs a larger tile scale');
    const [ax,ay]=anchors[settings.alignment],x=(size.width-w)*ax+settings.offsetX,y=(size.height-h)*ay+settings.offsetY;
    const firstX=Math.floor(-x/w),firstY=Math.floor(-y/h),lastX=Math.ceil((size.width-x)/w),lastY=Math.ceil((size.height-y)/h);
    const tiles=[];
    for(let row=firstY;row<lastY;row++)for(let column=firstX;column<lastX;column++)tiles.push({rect:[x+column*w,y+row*h,w,h],
        flipX:settings.mirror.includes('x')&&Math.abs(column%2)===1,flipY:settings.mirror.includes('y')&&Math.abs(row%2)===1,column,row});
    return tiles;
}
/** Paint an existing texture through the shared vector scene, never objects. */
export function paintPictureFill(builder,texture,size,image,value,{uv=[0,0,1,1]}={}) {
    const settings=checkedPictureFillSettings(value),tiles=pictureFillTiles(size,image,settings);
    if(settings.opacity===0)return builder;
    builder.pushClip([0,0,size.width,size.height]);
    for(const {rect,flipX,flipY}of tiles){
        const [x,y,w,h]=rect;
        if(flipX||flipY){builder.pushTransform([flipX?-1:1,0,0,flipY?-1:1,x+(flipX?w:0),y+(flipY?h:0)]);builder.image(texture,[0,0,w,h],{uv,opacity:settings.opacity});builder.popTransform();}
        else builder.image(texture,rect,{uv,opacity:settings.opacity});
    }
    return builder.popClip();
}
checkedPictureFillSettings.controlFactory=Object.freeze({topics:['scene','visual','documents'],constructor:'checkedPictureFillSettings({mode,opacity,scaleX,scaleY,offsetX,offsetY,alignment?,mirror?})',methods:'Checked owned settings; alignment tl/t/tr/l/ctr/r/bl/b/br defaults tl, mirror none/x/y/xy defaults none. Scales .01–100, offsets ±1000000 in 96-dpi coordinates. Picture fill semantics stay with the document owner.'});
pictureFillTiles.controlFactory=Object.freeze({topics:['scene','visual'],constructor:'pictureFillTiles({width,height},imageDimensions,settings)',methods:'Visible rect/flipX/flipY/column/row records with alignment, translated phase and alternating mirrors. Preserves the 4096-tile command budget; never creates document objects.'});
paintPictureFill.controlFactory=Object.freeze({topics:['scene','visual'],constructor:'paintPictureFill(sceneBuilder,texture,size,imageDimensions,settings,{uv?})',methods:'Draws an existing texture with clipping, opacity, alignment and alternating mirroring. Resource validation and lifetime remain caller-owned.'});
