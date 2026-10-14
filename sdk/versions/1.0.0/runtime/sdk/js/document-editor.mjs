/** Bounded document layout carried with the native multiline editor text.
 * Its trusted renderer and measured pointer path consume the same metadata. */
export function documentEditorLayout(value) {
    if(value===null)return null;
    const keys=['left','right','top','bottom','align','vertical','lineHeight'];
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(key=>!keys.includes(key)))throw new TypeError('Invalid document editor layout');
    const result={left:0,right:0,top:0,bottom:0,align:'left',vertical:'top',lineHeight:{kind:'normal',value:1},...value};
    if(!['left','right','top','bottom'].every(key=>Number.isFinite(result[key])&&result[key]>=0&&result[key]<=65536)||!['left','center','right'].includes(result.align)||!['top','middle','bottom'].includes(result.vertical))throw new RangeError('Invalid document editor bounds or alignment');
    const line=result.lineHeight;
    if(!line||typeof line!=='object'||Array.isArray(line)||Object.keys(line).some(key=>!['kind','value'].includes(key))||!['normal','multiple','exact'].includes(line.kind)||!Number.isFinite(line.value)||line.value<0||line.value>(line.kind==='exact'?65536:100)||(line.kind==='normal'&&line.value!==1))throw new RangeError('Invalid document editor line height');
    result.lineHeight=Object.freeze({...line});return Object.freeze(result);
}
export function documentEditorPresentation(control,presentation) {
    if(!control.values.documentFont)return presentation;
    const metadata={id:String(control.id),layout:control.values.documentLayout||documentEditorLayout({}),zoom:control.values.documentZoom,point:control.documentSelectionPoint||null};
    const hex=Array.from(new TextEncoder().encode(JSON.stringify(metadata)),byte=>byte.toString(16).padStart(2,'0')).join('');
    const result=presentation.replace('editor:plain:',`editor:document~${hex}:`);
    if(new TextEncoder().encode(result).length>16384)throw new RangeError('Document editor value and layout exceed native text budget');
    return result;
}
export function requestDocumentSelectionPoint(control,value) {
    if(!control.values.documentFont)throw new TypeError('Document selection requires a document font');
    const {x,y,phase='click',extend=false}=value||{};
    if(![x,y].every(n=>Number.isFinite(n)&&Math.abs(n)<=16)||!['click','down','move','up'].includes(phase)||typeof extend!=='boolean')throw new RangeError('Invalid document editor selection point');
    const previous=control.documentSelectionPoint;
    const origin=phase==='down'?Object.freeze({x,y,extend})
        : ['move','up'].includes(phase)&&previous?.phase!=='up'?previous?.origin||null:null;
    control.documentSelectionPoint=Object.freeze({x,y,phase,extend,origin,revision:(previous?.revision||0)+1});
    control.application?.invalidate(control);return control.documentSelectionPoint.revision;
}
