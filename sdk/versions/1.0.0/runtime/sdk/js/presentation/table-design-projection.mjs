import {readPptxTableDesignSource} from './table-design-package.mjs';
import {readPptxTextBoxSource} from './text-range-package.mjs';
import {readPptxSlideTheme} from './drawingml-theme.mjs';
import {presentationTableDefinitionCellStyle} from './table-design.mjs';
import {cellKey} from './table-values.mjs';

const children=(node,ns,name)=>node?.children.filter(n=>n.namespaceURI===ns&&n.localName===name)??[];
const one=(node,ns,name)=>{const found=children(node,ns,name);if(found.length>1)throw TypeError('Ambiguous table design '+name);return found[0]??null;};
const attr=(node,name)=>node?.attributes.find(a=>!a.namespaceURI&&a.localName===name)?.value;
const fillNames=new Set(['solidFill','noFill','gradFill','pattFill','blipFill','grpFill']);
const fail=message=>{throw TypeError('Presentation table design projection: '+message);};
const paint=(node,context)=>{
    if(node.localName==='noFill')return {understood:true,value:null};
    if(node.localName!=='solidFill'||node.children.length!==1)return {understood:false};
    const value=context.color(node.children[0]);return value?{understood:true,value}:{understood:false};
};
function directFill(node,namespace,context){
    const fills=node?.children.filter(n=>n.namespaceURI===namespace&&fillNames.has(n.localName))??[];
    if(fills.length>1)fail('ambiguous direct fill');return fills.length?paint(fills[0],context):null;
}
function textPatch(base,nodes,namespace,context,inheritedBold){
    const style={};let colorsUnderstood=true;
    style.bold=base.bold||inheritedBold;
    if(base.text){style.color=base.text.color;style.opacity=base.text.opacity;}
    for(const node of nodes.filter(Boolean)){
        const bold=attr(node,'b');if(bold!==undefined){if(['1','true'].includes(bold))style.bold=true;else if(['0','false'].includes(bold))style.bold=false;else delete style.bold;}
        if(one(node,namespace,'hlinkClick')){
            const color=context.resolveScheme('hlink');if(color){style.color=color.color;style.opacity=color.opacity;}else{delete style.color;delete style.opacity;colorsUnderstood=false;}
        }
        const fill=directFill(node,namespace,context);
        if(fill){
            if(fill.understood&&fill.value){style.color=fill.value.color;style.opacity=fill.value.opacity;}
            else{delete style.color;delete style.opacity;colorsUnderstood=false;}
        }
    }
    const fallbackColor=colorsUnderstood&&!Object.hasOwn(style,'color')?context.resolveScheme('tx1'):null;
    return {style,colorsUnderstood,fallbackColor};
}
function paragraphDefaults(body,row,namespace){
    const properties=one(row.node,namespace,'pPr'),level=Number(attr(properties,'lvl')??0);
    if(!Number.isInteger(level)||level<0||level>8)fail('invalid source paragraph level');
    const list=one(body,namespace,'lstStyle');
    return {level,nodes:[one(one(list,namespace,'defPPr'),namespace,'defRPr'),one(one(list,namespace,`lvl${level+1}pPr`),namespace,'defRPr'),one(properties,namespace,'defRPr')].filter(Boolean)};
}
function textProjection(source,base,namespace,context,inherited){
    const runs=[],paragraphs=[];let colorsUnderstood=true;
    for(const[index,row]of source.paragraphs.entries()){
        const own=paragraphDefaults(source.body,row,namespace),defaults=own.nodes,empty=row.atoms.length===0;
        let inheritedBold=false;
        for(const list of inherited)for(const name of ['defPPr',`lvl${own.level+1}pPr`]){
            const value=attr(one(one(list,namespace,name),namespace,'defRPr'),'b');if(value!==undefined&&['0','1','false','true'].includes(value))inheritedBold=['1','true'].includes(value);
        }
        const paragraphNodes=empty?[...defaults,one(row.node,namespace,'endParaRPr')]:defaults;
        const resolved=textPatch(base,paragraphNodes,namespace,context,inheritedBold);colorsUnderstood&&=resolved.colorsUnderstood;
        paragraphs.push({start:row.start,end:row.end,patch:resolved.style,fallbackColor:resolved.fallbackColor});
        if(index){
            const resolved=textPatch(base,defaults,namespace,context,inheritedBold);colorsUnderstood&&=resolved.colorsUnderstood;runs.push({text:'\n',separator:true,patch:resolved.style,fallbackColor:resolved.fallbackColor});
        }
        const atoms=empty?[null]:row.atoms;
        for(const atom of atoms){
            const nodes=atom?[...defaults,one(atom.node,namespace,'rPr')]:paragraphNodes;
            const resolved=textPatch(base,nodes,namespace,context,inheritedBold);colorsUnderstood&&=resolved.colorsUnderstood;runs.push({text:atom?.text??'',separator:false,patch:resolved.style,fallbackColor:resolved.fallbackColor});
        }
    }
    return {text:source.text,runs,paragraphs,colorsUnderstood};
}
function projectText(native,source){
    if(native.text!==source.text||native.paragraphs?.length!==source.paragraphs.length||native.runs.length!==source.runs.length)fail('native and source text differ');
    const paragraphs=source.paragraphs.map((row,index)=>{
        const paragraph=native.paragraphs[index];if(paragraph.start!==row.start||paragraph.end!==row.end)fail('native and source paragraphs differ');
        return {...paragraph,style:resolvedTextStyle(paragraph.style,row)};
    });
    const runs=source.runs.map((row,index)=>{
        const run=native.runs[index];if(run.text!==row.text||row.separator&&run.id!=='')fail('native and source text runs differ');
        return {...run,style:resolvedTextStyle(run.style,row)};
    });
    if(runs.map(run=>run.text).join('')!==native.text)fail('native text run coverage differs');
    const styled=runs.filter(run=>!(run.id===''&&run.text==='\n')),style=styled[0]?.style??native.style;
    const mixed=styled.some(run=>JSON.stringify(run.style)!==JSON.stringify(style));
    const complete=styled.every(run=>run.style.fontFamily&&run.style.fontSize&&run.style.color);
    const unsupported=native.unsupported.filter(code=>code!=='MIXED_TEXT_STYLE'&&!(code==='UNRESOLVED_TEXT_STYLE'&&complete)&&!(code==='UNSUPPORTED_TEXT_COLOR'&&source.colorsUnderstood&&complete));
    if(mixed)unsupported.push('MIXED_TEXT_STYLE');
    return {...native,style,runs,paragraphs,mixed,unsupported};
}
function resolvedTextStyle(native,row){
    const style={...native,...row.patch};
    if(style.color==null&&row.fallbackColor){style.color=row.fallbackColor.color;style.opacity=row.fallbackColor.opacity;}
    return style;
}

/** Read-only semantic enrichment over original native geometry and permission.
 * Nothing is materialized in the package or a second worker. The caller retains
 * the original document revision, source/signature guards and binary history. */
export function readPptxTableDesignProjection(input,slide,object){
    const state=readPptxTableDesignSource(input,slide,object);if(!state.design)return null;
    const {grid,parsed,styles,design}=state,namespace=grid.namespace;
    const relationship=styles.opc.relationships(parsed.path).find(rel=>rel.id===parsed.result.slides.find(row=>row.id===slide)?.relationship);
    if(!relationship?.part)fail('missing source slide');
    const p=namespace==='http://schemas.openxmlformats.org/drawingml/2006/main'?'http://schemas.openxmlformats.org/presentationml/2006/main':'http://purl.oclc.org/ooxml/presentationml/main';
    const context=readPptxSlideTheme(styles.opc,relationship.part,p),ranges=readPptxTextBoxSource(grid.source,slide);
    const master=context.owners.find(owner=>owner.root.localName==='sldMaster')?.root;
    const inherited=[one(one(one(context.themeRoot,namespace,'objectDefaults'),namespace,'txDef'),namespace,'lstStyle'),one(styles.opc.xml(parsed.path).root,p,'defaultTextStyle'),one(one(master,p,'txStyles'),p,'otherStyle')].filter(Boolean);
    const cells=grid.cells.map(cell=>{
        const geometry=Object.fromEntries(['row','column','rowSpan','columnSpan','covered'].map(key=>[key,cell[key]]));
        if(cell.covered)return {cell:geometry};
        const source=ranges.get(cellKey(object,cell.row,cell.column));if(!source||source.paragraphs.some(row=>row.atoms.some(atom=>atom.readonly)))fail('source cell text is not supported');
        const base=presentationTableDefinitionCellStyle(design,{row:cell.row,column:cell.column,rows:grid.rows,columns:grid.columns,rowSpan:cell.rowSpan,columnSpan:cell.columnSpan},context.resolveScheme);
        const properties=one(cell.node,namespace,'tcPr'),fill=directFill(properties,namespace,context),borders={};
        for(const[side,name]of [['left','lnL'],['right','lnR'],['top','lnT'],['bottom','lnB']]){
            const line=one(properties,namespace,name);if(!line){borders[side]={understood:true,value:base.border};continue;}
            const color=directFill(line,namespace,context),width=Number(attr(line,'w')??12700)/9525;
            borders[side]=color?.understood&&Number.isFinite(width)&&width>=0&&width<=1e6?{understood:true,value:color.value?{...color.value,width}:null}:{understood:false};
        }
        return {cell:geometry,text:textProjection(source,base,namespace,context,inherited),fill:fill??{understood:true,value:base.fill},borders};
    });
    // Retain only bounded plain projection data. Source ZIPs, DOMs and theme
    // contexts are released after this call, rather than cached per table.
    return projection(design,grid.rows,grid.columns,cells);
}
function projection(design,rows,columns,cells){
    return Object.freeze({design,rows,columns,cell(native){
        const row=cells[native.row*columns+native.column];
        if(!row||['row','column','rowSpan','columnSpan','covered'].some(key=>row.cell[key]!==native[key]))fail('native and source table geometry differ');
        if(native.covered)return native;
        const result={...native,borders:{...native.borders}};
        if(row.fill.understood){result.fill=row.fill.value;result.unsupported=native.unsupported.filter(code=>code!=='UNSUPPORTED_TABLE_FILL');}
        for(const[side,border]of Object.entries(row.borders))if(border.understood)result.borders[side]=border.value;
        if(native.text)result.text=projectText(native.text,row.text);
        return result;
    }});
}
