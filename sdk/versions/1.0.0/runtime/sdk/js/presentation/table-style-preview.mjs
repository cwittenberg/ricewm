import {presentationTableStyles} from './table-design.mjs';
import {resolveColorReference,checkedResolvedPaint} from '../color-references.mjs';
import {SceneBuilder} from '../scene-builder.mjs';

const fail=message=>{throw TypeError('Presentation table design: '+message);};
const size=(value,maximum)=>Number.isSafeInteger(value)&&value>=1&&value<=maximum;
const scheme=slot=>({kind:'scheme',value:slot,transforms:[]});
function resolve(source,resolver){
    if(typeof resolver!=='function')fail('a document theme resolver is required');
    return resolveColorReference(source,slot=>{const paint=resolver(slot);return paint?checkedResolvedPaint(paint):null;});
}

const packed=paint=>{
    const rgb=Number.parseInt(paint.color.slice(1),16);
    return ((Math.round(paint.opacity*255)<<24)|((rgb&255)<<16)|(rgb&0xff00)|(rgb>>>16))>>>0;
};
const mix=(a,b,fraction)=>({color:'#'+[1,3,5].map(at=>Math.round(parseInt(a.color.slice(at,at+2),16)*(1-fraction)+parseInt(b.color.slice(at,at+2),16)*fraction).toString(16).padStart(2,'0')).join('').toUpperCase(),opacity:a.opacity*(1-fraction)+b.opacity*fraction});

/** Compact four-band swatch used by the reference-style ribbon gallery. It
 * intentionally depicts the style family, independently of current row flags;
 * document rendering uses presentationTableCellStyle instead. */
export function presentationTableStylePreview(preset,{width=48,height=36,resolveScheme}={}) {
    const entry=presentationTableStyles().find(style=>style.id===preset);
    if(!entry||!size(width,256)||!size(height,256)||width<4||height<4)fail('invalid bounded style preview');
    // Swatches use the theme's dark colour for the tx1 entry; document cells
    // resolve tx1 through the slide's colour map as usual.
    const accent=resolve(scheme(entry.slot==='tx1'?'dk1':entry.slot),resolveScheme),white={color:'#FFFFFF',opacity:1},black={color:'#000000',opacity:1};
    const builder=new SceneBuilder(width,height),kind=entry.family;
    for(let row=0;row<4;row++){
        const fill=['none','grid'].includes(kind)||kind==='light1'&&row===0||kind==='light2'?white:
            kind==='dark1'?row===0?black:mix(accent,black,.3):row===0?accent:mix(accent,white,row%2?.6:.8);
        const y=row*height/4;
        builder.rect([0,y,width,height/4],packed(fill));
        builder.line([0,y+height/4],[width,y+height/4],.5,kind==='grid'?0xff808080:0xffffffff);
    }
    if(kind==='grid'||kind==='light2'){
        const color=kind==='grid'?0xff808080:packed(accent),d=.4;
        builder.line([d,d],[width-d,d],.8,color).line([width-d,d],[width-d,height-d],.8,color)
            .line([width-d,height-d],[d,height-d],.8,color).line([d,height-d],[d,d],.8,color);
    }
    return builder.finish();
}
presentationTableStylePreview.controlFactory=Object.freeze({topics:['tables','documents','visual'],constructor:'presentationTableStylePreview(...)',methods:'Build a bounded native SceneSurface swatch from a known style and document theme resolver. Separate compact family preview from actual cell appearance.'});
