import {shapedTextPresentation} from '../sdk/versions/1.0.0/runtime/sdk/js/shaped-text-presentation.mjs';
import {ShapedTextLayout} from '../sdk/versions/1.0.0/runtime/sdk/js/shaped-text-layout.mjs';
import {parsePresentation} from './rice-native-editor-renderer.mjs';
import {applyDocumentFont,documentFontMetrics} from './rice-document-text-metrics.mjs';
import {rgbaColor} from './rice-native-canvas-primitives.mjs';
import {documentEditorLayout,documentEditorShapedReference} from '../sdk/versions/1.0.0/runtime/sdk/js/document-editor.mjs';

export const isDocumentEditor = command => command.documentEditor === true && command.text.startsWith('editor:document~');
/** Versionless bounded metadata remains inside the native editor presentation;
 * text, caret and selection continue to use its established scalar protocol. */
export function documentEditorMetadata(command) {
    if(!isDocumentEditor(command))return null;
    const presentation=parsePresentation(command.text),hex=presentation.language.slice('document~'.length);
    if(hex.length>4096||!hex.length||!/^(?:[0-9a-f]{2})+$/u.test(hex))throw new RangeError('Invalid document editor metadata');
    const data=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(Uint8Array.from(hex.match(/../gu),value=>parseInt(value,16))));
    if(!data||typeof data!=='object'||Array.isArray(data)||Object.keys(data).some(key=>!['id','layout','zoom','point','placeholder','shaped'].includes(key)))throw new RangeError('Invalid document editor metadata');
    if(data.placeholder!==undefined&&(typeof data.placeholder!=='string'||new TextEncoder().encode(data.placeholder).length>512))throw new RangeError('Invalid document editor placeholder');
    const layout=documentEditorLayout(data.layout===undefined?{}:data.layout),zoom=data.zoom===undefined?100:data.zoom;
    if(typeof data.id!=='string'||!/^\d{1,20}$/u.test(data.id)||BigInt(data.id)>0xffffffffffffffffn)throw new RangeError('Invalid editor identity');
    if(!layout||!['left','right','top','bottom'].every(key=>Number.isFinite(layout[key])&&layout[key]>=0&&layout[key]<=65536)||!['left','center','right'].includes(layout.align)||!['top','middle','bottom'].includes(layout.vertical)||!Number.isFinite(zoom)||zoom<1||zoom>1600)throw new RangeError('Invalid document editor layout');
    const line=layout.lineHeight;
    if(!line||!['normal','multiple','exact'].includes(line.kind)||!Number.isFinite(line.value)||line.value<0||line.value>(line.kind==='exact'?65536:100)||(line.kind==='normal'&&line.value!==1))throw new RangeError('Invalid document editor spacing');
    const point=data.point===undefined?null:data.point;
    if(point!==null&&(!point||![point.x,point.y].every(value=>Number.isFinite(value)&&Math.abs(value)<=16)||!['click','down','move','up'].includes(point.phase)||typeof point.extend!=='boolean'||!Number.isSafeInteger(point.revision)||point.revision<1))throw new RangeError('Invalid editor point request');
    if(point?.origin!=null&&(![point.origin.x,point.origin.y].every(value=>Number.isFinite(value)&&Math.abs(value)<=16)||typeof point.origin.extend!=='boolean'))throw new RangeError('Invalid editor pointer origin');
    const shaped=data.shaped===undefined?null:documentEditorShapedReference(data.shaped);
    return {presentation,layout,scale:zoom/100,point,...(shaped?{shaped}:{}),id:BigInt(data.id),placeholder:data.placeholder||''};
}

const shapedLayouts=new WeakMap();
export function documentEditorShapedGeometry(command) {
    const metadata=documentEditorMetadata(command);if(!metadata?.shaped)return null;
    const data=command.shapedEditor;
    if(command.shapedEditorPending||!data||data.token!==metadata.shaped.token||data.text!==metadata.presentation.document)return null;
    let layout=shapedLayouts.get(data.map);
    if(!layout){layout=new ShapedTextLayout(data.text,data.map);shapedLayouts.set(data.map,layout);}
    if(layout.caret(data.caret).scalar!==metadata.presentation.caret||layout.caret(data.anchor).scalar!==metadata.presentation.anchor)return null;
    const presentation=shapedTextPresentation(data.presentation??null);
    let transform=[metadata.scale,0,0,metadata.scale,command.rect.x,command.rect.y];
    if(presentation){
        const scale=Math.min(command.rect.width/presentation.width,command.rect.height/presentation.height),m=presentation.transform;
        transform=[m[0]*scale,m[1]*scale,m[2]*scale,m[3]*scale,
            command.rect.x+(command.rect.width-presentation.width*scale)/2+m[4]*scale,
            command.rect.y+(command.rect.height-presentation.height*scale)/2+m[5]*scale];
    }
    return {...metadata,layout,transform,caretColor:presentation?.caretColor,caret:data.caret,anchor:data.anchor};
}
export function documentEditorShapedHit(command,x,y) {
    const data=documentEditorShapedGeometry(command);if(!data)return null;
    const [a,b,c,d,e,f]=data.transform,det=a*d-b*c,dx=x-e,dy=y-f;
    const stop=data.layout.hit(Math.max(-1e6,Math.min(1e6,(d*dx-c*dy)/det)),Math.max(-1e6,Math.min(1e6,(a*dy-b*dx)/det)));
    return stop===null?null:{stop,scalar:data.layout.caret(stop).scalar,token:data.shaped.token};
}
function paintShapedDocumentEditor(context,command,selectionOnly=false) {
    const data=documentEditorShapedGeometry(command);if(!data)return true;
    context.beginPath();context.rect(command.rect.x,command.rect.y,command.rect.width,command.rect.height);context.clip();
    context.transform(...data.transform);
    context.fillStyle=rgbaColor(command.selection?.color||[184,214,255,255]);
    if(selectionOnly||!command.shapedSelectionBelow)for(const rect of data.layout.selectionRects(data.anchor,data.caret))context.fillRect(rect.x,rect.y,rect.width,rect.height);
    if(!selectionOnly&&command.caret){const caret=data.layout.caret(data.caret);context.fillStyle=data.caretColor||rgbaColor(command.color);context.fillRect(caret.x,caret.y0,1/Math.hypot(data.transform[0],data.transform[1]),Math.max(1,caret.y1-caret.y0));}
    return true;
}

export function paintShapedTextSelection(context,command) {
    context.save();try{return paintShapedDocumentEditor(context,command,true);}finally{context.restore();}
}

/** One measured layout supplies painting, caret placement and pointer selection.
 * Only uniform scalar text is represented; imported rich spans remain in source. */
export function documentEditorGeometry(context,command) {
    const metadata=documentEditorMetadata(command);if(!metadata)return null;
    const {presentation,layout,scale}=metadata,characters=Array.from(presentation.document),font={...command,sizePx:command.sizePx*scale};
    applyDocumentFont(context,font);context.direction='ltr';context.fontKerning='normal';
    const precise=documentFontMetrics(command),metrics=context.measureText('Mg'),ascent=precise?precise.ascent*font.sizePx:metrics.fontBoundingBoxAscent||font.sizePx*.8,descent=precise?precise.descent*font.sizePx:metrics.fontBoundingBoxDescent||font.sizePx*.2;
    const line=layout.lineHeight,lineHeight=line.kind==='exact'?line.value*scale:line.kind==='multiple'?font.sizePx*line.value:ascent+descent+(precise?.gap||0)*font.sizePx;
    const box={x:command.rect.x+layout.left*scale,y:command.rect.y+layout.top*scale,width:Math.max(1,command.rect.width-(layout.left+layout.right)*scale),height:Math.max(1,command.rect.height-(layout.top+layout.bottom)*scale)};
    const rows=[],measure=(start,end)=>context.measureText(characters.slice(start,end).join('')).width;
    let start=0;
    while(start<=characters.length){
        let paragraphEnd=start;while(paragraphEnd<characters.length&&characters[paragraphEnd]!=='\n')paragraphEnd++;
        if(start===paragraphEnd){rows.push({start,end:start,width:0});}
        else while(start<paragraphEnd){
            let end=paragraphEnd;
            if(presentation.wrap!=='none'){
                let low=start,high=Math.min(paragraphEnd,start+1),step=1;
                while(high<paragraphEnd&&measure(start,high)<=box.width){low=high;step*=2;high=Math.min(paragraphEnd,start+step);}
                if(measure(start,high)<=box.width)low=high;
                else while(low+1<high){const middle=(low+high)>>1;if(measure(start,middle)<=box.width)low=middle;else high=middle;}
                end=Math.max(start+1,low);
                if(presentation.wrap==='word'&&end<paragraphEnd){for(let at=end;at>start;at--)if(/\s/u.test(characters[at-1])){end=at;break;}}
            }
            let paintedEnd=end;while(paintedEnd>start&&/\s/u.test(characters[paintedEnd-1]))paintedEnd--;
            rows.push({start,end,width:measure(start,paintedEnd)});start=end;
        }
        if(paragraphEnd===characters.length)break;
        start=paragraphEnd+1;
    }
    const caret=Math.max(0,Math.min(characters.length,presentation.caret??0)),anchor=Math.max(0,Math.min(characters.length,presentation.anchor??caret));
    const caretRow=rows.findIndex(row=>caret>=row.start&&caret<=row.end),height=rows.length*lineHeight;
    const offset=layout.vertical==='middle'?(box.height-height)/2:layout.vertical==='bottom'?box.height-height:0;
    const rowTop=offset+Math.max(0,caretRow)*lineHeight;
    const scroll=command.caret?Math.min(0,rowTop)<0?rowTop:Math.max(0,rowTop+lineHeight-box.height):0;
    rows.forEach((row,index)=>{row.x=box.x+(layout.align==='center'?(box.width-row.width)/2:layout.align==='right'?box.width-row.width:0);row.y=box.y+offset+index*lineHeight-scroll;row.baseline=row.y+(lineHeight-ascent-descent)/2+ascent;});
    return {...metadata,characters,font,rows,box,caret,anchor,caretRow:Math.max(0,caretRow),lineHeight,ascent,descent,measure};
}

export function documentEditorHit(context,command,x,y) {
    context.save();try{
        if(documentEditorMetadata(command)?.shaped)return documentEditorShapedHit(command,x,y)?.scalar??null;
        const data=documentEditorGeometry(context,command);if(!data)return null;
        const row=data.rows.reduce((best,row)=>Math.abs(y-row.y-data.lineHeight/2)<Math.abs(y-best.y-data.lineHeight/2)?row:best,data.rows[0]);
        if(x<=row.x)return row.start;
        let low=row.start,high=row.end;
        while(low<high){const middle=(low+high)>>1,before=data.measure(row.start,middle),after=data.measure(row.start,middle+1);if(x-row.x<(before+after)/2)high=middle;else low=middle+1;}
        return low;
    }finally{context.restore();}
}

export function paintDocumentEditor(context,command) {
    context.save();try{
        if(documentEditorMetadata(command)?.shaped)return paintShapedDocumentEditor(context,command);
        const data=documentEditorGeometry(context,command);if(!data)return false;
        const {rows,characters,caret,anchor,lineHeight,font}=data;
        context.beginPath();context.rect(command.rect.x,command.rect.y,command.rect.width,command.rect.height);context.clip();context.textAlign='left';context.textBaseline='alphabetic';
        const from=Math.min(caret,anchor),to=Math.max(caret,anchor);
        if(!characters.length&&data.placeholder){context.fillStyle='rgba(128,128,128,1)';context.fillText(data.placeholder,rows[0].x,rows[0].baseline);}
        for(const row of rows){
            if(row.y+lineHeight<command.rect.y||row.y>command.rect.y+command.rect.height)continue;
            if(to>row.start&&from<=row.end){const start=Math.max(from,row.start),end=Math.min(to,row.end);context.fillStyle=rgbaColor(command.selection?.color||[184,214,255,255]);context.fillRect(row.x+data.measure(row.start,start),row.y,Math.max(1,data.measure(row.start,end)-data.measure(row.start,start)),Math.max(1,lineHeight));}
            const text=characters.slice(row.start,row.end).join('');context.fillStyle=rgbaColor(command.color);context.fillText(text,row.x,row.baseline);
            if(font.font.underline&&text)context.fillRect(row.x,row.baseline+data.descent*.4,row.width,Math.max(1,font.sizePx/14));
        }
        if(command.caret){const row=rows[data.caretRow];context.fillStyle=rgbaColor(command.color);context.fillRect(row.x+data.measure(row.start,caret),row.y,1,Math.max(1,lineHeight));}
        return true;
    }finally{context.restore();}
}
