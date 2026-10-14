import {Column, Row, Stack, Container, ScrollBar, VirtualGrid, Table as LegacyTable} from './controls.mjs';
import {State, oneOf, numberInRange} from './control.mjs';
import {Message} from './messages.mjs';
import {validateGridStyle} from './grid-frame.mjs';
import {ChartDashboard} from './chart-dashboard.mjs';

const alignments = {left:0, center:1, right:2};
const immutable = value => Object.freeze(value);
const text = value => Array.from(String(value ?? '').replace(/[\u0000-\u0009\u000b-\u001f\u007f-\u009f]/gu, ' ')).slice(0,512).join('');
function columns(value) {
    if (!Array.isArray(value) || value.length > 64) throw RangeError('Table supports up to 64 columns');
    const keys = new Set();
    return immutable(value.map(entry => {
        const c = typeof entry === 'string' ? {key:entry, title:entry} : {...entry};
        if (typeof c.key !== 'string' || !c.key || c.key.length > 128 || keys.has(c.key)) throw TypeError('Table columns need unique field keys');
        keys.add(c.key);
        c.title = text(c.title ?? c.key);
        c.width = c.width ?? 160;
        if (!Number.isInteger(c.width) || c.width < 48 || c.width > 2000) throw RangeError('Table column width requires 48..2000 pixels');
        c.align = c.align ?? 'left';
        if (!Object.hasOwn(alignments,c.align)) throw TypeError('Table alignment requires left, center or right');
        if (c.format != null && typeof c.format !== 'function') throw TypeError('Table format must be a function');
        if (c.style) style(c.style);
        return immutable(c);
    }));
}
function rows(value) {
    if (!Array.isArray(value) || value.length > 100000 || value.some(r => !r || typeof r !== 'object' || Array.isArray(r))) throw TypeError('Table data requires up to 100000 row objects');
    return immutable(value.map(r => immutable({...r})));
}
function style(value = {}, alignment = 0) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw TypeError('Table style must be an object');
    const allowed = ['foreground','background','bold','italic','underline','fontFamily','fontSize','align','wrap','strike','borders'];
    if (Object.keys(value).some(k => !allowed.includes(k))) throw TypeError('Unknown Table style field');
    const result = {alignment:value.align == null ? alignment : alignments[value.align], role:0};
    if (['bold','italic','underline','fontFamily','fontSize'].some(k => value[k] != null)) result.font = {
        family:value.fontFamily ?? 'sans-serif', size_pt:value.fontSize ?? 10, weight:value.bold ? 700 : 400,
        italic:value.italic === true, underline:value.underline === true,
    };
    const paint = Object.fromEntries(['foreground','background','wrap','strike','borders'].filter(k => value[k] != null).map(k => [k,value[k]]));
    if (Object.keys(paint).length) result.paint = paint;
    return validateGridStyle(result);
}
function rules(value) {
    if (!Array.isArray(value) || value.length > 64) throw RangeError('Table supports up to 64 conditional rules');
    return immutable(value.map(r => {
        if (!r || typeof r.field !== 'string' || !r.field || !['eq','ne','gt','gte','lt','lte','between','contains'].includes(r.op)) throw TypeError('Conditional formatting needs field and op');
        if (r.op === 'between' && (!Array.isArray(r.value) || r.value.length !== 2 || !r.value.every(Number.isFinite) || r.value[0] > r.value[1])) throw TypeError('between requires an ordered numeric pair');
        if (r.scope != null && !['cell','row'].includes(r.scope)) throw TypeError('Conditional scope requires cell or row');
        style(r.style);
        return immutable({...r, value:Array.isArray(r.value) ? immutable([...r.value]) : r.value, style:immutable({...r.style})});
    }));
}
function matches(row, rule) {
    const v = row[rule.field], n = rule.value;
    switch (rule.op) {
        case 'eq': return v === n;
        case 'ne': return v !== n;
        case 'gt': return typeof v === 'number' && v > n;
        case 'gte': return typeof v === 'number' && v >= n;
        case 'lt': return typeof v === 'number' && v < n;
        case 'lte': return typeof v === 'number' && v <= n;
        case 'between': return typeof v === 'number' && v >= n[0] && v <= n[1];
        case 'contains': return String(v ?? '').includes(String(n));
    }
}
let nextTable = 1;

/** A data table using the native retained grid, with bounded visible frames. */
export class Table extends Column {
    static family = 'collection';
    static constructorOptions = immutable(['data','columns','rowKey','dashboard','id','filterField','height','headerStyle','cellStyle','conditionalFormats','selectionMode','rowHeight','rowHeaders','resizable','emptyText']);
    static authoring = immutable({
        constructor:'new Table({name:"orders",data:rows,columns:[{key:"customer",title:"Customer",width:220},{key:"amount",title:"Amount",align:"right",format:v=>v.toFixed(2)}],rowKey:"id",height:300}) renders a real native table. DataTable is an alias. Empty data shows emptyText. Legacy Table(properties,children) remains available for hand-authored native rows.',
        formatting:'headerStyle/cellStyle and column.style support bold, italic, underline, fontFamily, fontSize (points), align:left|center|right, foreground/background:#RRGGBB, wrap, strike and borders. Headers are bold and frozen; native defaults follow the active desktop theme. conditionalFormats:[{field:"amount",op:"lt",value:0,style:{foreground:"#b42318",bold:true},scope:"cell"}] supports eq/ne/gt/gte/lt/lte/between/contains and row scope; later rules win.',
        selection:'Click or drag cells; Shift extends the rectangular range. Click a row number or column heading to select rows/columns. selectionMode:cell|row|column|none controls body clicks. Arrow/Home/End/Page keys navigate; Shift extends; Ctrl+A selects all, Ctrl+Space a column, Shift+Space a row, Escape clears. selectCell/Row/Column/Range use zero-based displayed data indices. Message.SELECT gives {selection,rows,rowIds,columnKeys}; Message.ACTIVATE gives {row,rowId,columnKey,value} on Enter or double-click for app-owned drill-down.',
        linked:'Share new ChartDashboard(rows) with Chart and Table using unique stable ids. new Table({dashboard,id:"records",rowKey:"id",filterField:"region",columns:[...]}) follows chart filters; selecting table rows filters peer charts by region while retaining its own context. filterField defaults to rowKey. clearSelection() clears this table filter; dashboard.clear() clears all report filters. setData() updates shared data. Sorting retains contiguous selections by rowKey; clears a selection that becomes disjoint. Provide a primary-key field for stable navigation.',
        methods:'Double-click a heading to sort ascending/descending. setData(rows), setColumns(columns), sort(key,asc|desc|null), setConditionalFormats(rules), selectCell(row,column), selectRow(row), selectColumn(column), selectRange({firstRow,lastRow,firstColumn,lastColumn}), clearSelection(), getSelectedRows(), getSelection(), inspect(). table.grid is the native viewport for named sdk.interact coordinates. table.grid.hitTest(x,y) returns native logical rows (header=0, data=index+1), and columns include a frozen row-selector column when rowHeaders is true. Wheel and native scrollbars scroll without creating one control per cell.',
    });
    static properties = immutable({
        data:{default:immutable([]), validate:rows, reflect:false},
        columns:{default:immutable([]), validate:columns, reflect:false},
        conditionalFormats:{default:immutable([]), validate:rules, reflect:false},
        headerStyle:{default:immutable({bold:true}), validate:v => {style(v); return immutable({...v});}, reflect:false},
        cellStyle:{default:immutable({}), validate:v => {style(v); return immutable({...v});}, reflect:false},
        selectionMode:{default:'cell', validate:oneOf(['cell','row','column','none'],'selectionMode'), reflect:false},
        rowHeight:{default:30, validate:numberInRange(24,120,'rowHeight'), reflect:false},
        rowHeaders:{default:true, validate:Boolean, reflect:false},
        resizable:{default:true, validate:Boolean, reflect:false},
        emptyText:{default:'No records', validate:v => text(v), reflect:false},
    });
    constructor({dashboard=null,id=null,rowKey=null,filterField=null,height=300,...properties}={}, children=[]) {
        // Existing manually authored rows keep their native Table ABI/layout.
        if (children.length || ('columnWidths' in properties && !('data' in properties) && !dashboard)) return new LegacyTable(properties,children);
        if (dashboard !== null && !(dashboard instanceof ChartDashboard)) throw TypeError('Table dashboard must be a ChartDashboard');
        if (!Number.isInteger(height) || height < 80 || height > 2000) throw RangeError('Table height requires 80..2000 pixels');
        for (const v of [id,rowKey,filterField]) if (v !== null && (typeof v !== 'string' || !v || v.length > 128)) throw TypeError('Table ids and field keys require bounded text');
        super({text:'panel:fixed:flush',state:State.FLAT,...properties});
        this.tableId = id || this.name || `table-${nextTable++}`;
        this.dashboard = dashboard; this.rowKey = rowKey; this.filterField = filterField ?? rowKey;
        this.width=640; this.height=height; this.top=0; this.left=0; this.revision=0;
        this.selection=null; this.anchor=null; this.sortState=null;
        this.grid = new VirtualGrid({name:`${this.tableId}-grid`,accessibleName:this.values.accessibleName || 'Data table',state:State.KEY_NAVIGATION|State.EXPANSIVE});
        this.vertical = new ScrollBar({name:`${this.tableId}-vertical`,orientation:'vertical'});
        this.horizontal = new ScrollBar({name:`${this.tableId}-horizontal`,orientation:'horizontal'});
        this.verticalTrack=new Stack({text:'panel:fixed:width=16:flush',state:State.FLAT},[this.vertical]);
        this.horizontalTrack=new Container({text:'panel:fixed-height:16:flush',state:State.FLAT},[this.horizontal]);
        this.append(new Row({text:`panel:fixed-height:${height}:flush`,state:State.FLAT},[this.grid,this.verticalTrack]));
        this.append(this.horizontalTrack);
        this.grid.on(Message.SIZE_CHANGED,e => {const w=Math.round(e.detail?.width),h=Math.round(e.detail?.height);if(w>48&&h>40&&w<=4096&&h<=2048&&(w!==this.width||h!==this.height)){this.width=w;this.height=h;this.render();}});
        this.grid.on(Message.POINTER,e => this.pointer(e));
        this.grid.on(Message.KEY,e => this.key(e));
        this.grid.on(Message.BLUR,()=>{this.lastClick=null;this.drag=null;this.resize=null;});
        const fraction = e => {const [a,b]=String(e.detail?.value ?? '').split('/').map(Number);return b>0?a/b:Number(e.detail?.value)||0;};
        this.vertical.on(Message.VALUE_CHANGED,e => {this.top=Math.round(Math.max(0,Math.min(1,fraction(e)))*this.maxTop());this.render();});
        this.horizontal.on(Message.VALUE_CHANGED,e => {this.left=Math.round(Math.max(0,Math.min(1,fraction(e)))*this.maxLeft());this.render();});
        this.refresh();
        this.hadFilter=false;
        this.stopDashboard=dashboard?.subscribe(() => {if(!this.publishing){const has=Object.hasOwn(dashboard.filters,this.tableId);if(this.hadFilter&&!has){this.selection=null;this.anchor=null;this.cursor=null;}this.hadFilter=has;this.refresh();}});
    }
    setProperty(name,value,options) {
        const changed=super.setProperty(name,value,options);
        if(changed&&this.grid&&['data','columns','conditionalFormats','headerStyle','cellStyle','rowHeight','rowHeaders','emptyText'].includes(name)) {
            if(name==='data'||name==='columns')this.refresh();else this.render();
        }
        return changed;
    }
    get data(){return this.dashboard?.data??this.values.data;}
    setData(data){if(this.rowKey){const keys=new Set();for(const row of rows(data)){const k=row[this.rowKey];if(!['string','number','boolean'].includes(typeof k)||typeof k==='number'&&!Number.isFinite(k)||keys.has(k))throw TypeError('Table rowKey requires unique scalar values');keys.add(k);}}if(this.dashboard)this.dashboard.setData(rows(data));else this.setProperty('data',data);return this;}
    setColumns(value){this.setProperty('columns',value);return this;}
    setConditionalFormats(value){this.setProperty('conditionalFormats',value);return this;}
    identity(row){return this.rowKey ? row[this.rowKey] : row;}
    refresh() {
        const selected=this.getSelectedRows?.() ?? [],ids=new Set(selected.map(r=>this.identity(r))),old=this.selection;
        const source=this.dashboard ? this.dashboard.rows({except:this.tableId}) : this.values.data;
        if(this.rowKey){const keys=new Set();for(const row of source){const k=row[this.rowKey];if(!['string','number','boolean'].includes(typeof k)||typeof k==='number'&&!Number.isFinite(k)||keys.has(k))throw TypeError('Table rowKey requires unique scalar values');keys.add(k);}}
        this.view=[...source];
        if(this.sortState){const {key,direction}=this.sortState;this.view.sort((a,b)=>{const x=a[key],y=b[key];if(x==null||y==null)return x==null?(y==null?0:1):-1;return (typeof x==='number'&&typeof y==='number'?x-y:String(x).localeCompare(String(y),undefined,{numeric:true}))*(direction==='desc'?-1:1);});}
        this.selection=null;
        if(old?.kind==='column'||old?.kind==='all')this.selection=this.view.length?{...old,lastRow:this.view.length-1}:null;
        else if(ids.size){const found=this.view.map((r,i)=>ids.has(this.identity(r))?i:-1).filter(i=>i>=0);if(found.length && found.at(-1)-found[0]+1===found.length)this.selection={...old,firstRow:found[0],lastRow:found.at(-1)};}
        if(this.selection){this.selection.firstColumn=Math.min(this.selection.firstColumn,this.values.columns.length-1);this.selection.lastColumn=Math.min(this.selection.lastColumn,this.values.columns.length-1);if(this.selection.firstColumn<0)this.selection=null;}
        this.cursor=null;
        this.anchor=this.selection?{row:this.selection.firstRow,column:this.selection.firstColumn}:null;
        this.top=Math.min(this.top,this.maxTop());this.left=Math.min(this.left,this.maxLeft());this.render();
        if(old&&!this.selection)this.publishSelection();
    }
    sort(key,direction='asc') {
        if(!this.values.columns.some(c=>c.key===key)||!['asc','desc',null].includes(direction))throw TypeError('Table sort requires an existing column and asc, desc or null');
        this.sortState=direction?{key,direction}:null;this.refresh();return this;
    }
    maxTop(){return Math.max(0,(this.view?.length||0)*this.values.rowHeight-Math.max(0,this.height-this.values.rowHeight));}
    maxLeft(){return Math.max(0,this.values.columns.reduce((s,c)=>s+c.width,0)-this.width+(this.values.rowHeaders?40:0));}
    render() {
        if(!this.grid)return;
        const h=Math.min(this.height-1,Math.round(this.values.rowHeight)),gutter=this.values.rowHeaders?40:0,width=this.width-gutter;
        const tracks=this.values.rowHeaders?[{index:0,start:0,extent:gutter,clip_start:0,clip_end:gutter,frozen:true}]:[],rs=[{index:0,start:0,extent:h,clip_start:0,clip_end:h,frozen:true}];
        let x=gutter-this.left;
        for(const [index,c]of this.values.columns.entries()){if(x<this.width&&x+c.width>gutter)tracks.push({index:index+(this.values.rowHeaders?1:0),start:x,extent:c.width,clip_start:Math.max(gutter,x),clip_end:Math.min(this.width,x+c.width),frozen:false});x+=c.width;}
        if(!tracks.length||!this.values.columns.length)tracks.push({index:this.values.rowHeaders?1:0,start:gutter,extent:width,clip_start:gutter,clip_end:this.width,frozen:false});
        this.top=Math.min(this.top,this.maxTop());
        const start=Math.floor(this.top/h),limit=Math.min(511,Math.floor(4096/tracks.length)-1);
        for(let i=start;i<this.view.length&&rs.length<=limit;i++){const y=h+i*h-this.top;if(y>=this.height)break;rs.push({index:i+1,start:y,extent:h,clip_start:Math.max(h,y),clip_end:Math.min(this.height,y+h),frozen:false});}
        if(!this.view.length&&this.height>h)rs.push({index:1,start:h,extent:h,clip_start:h,clip_end:Math.min(this.height,2*h),frozen:false});
        const strings=[],cells=[],styles=[],palette=new Map();
        const stringBudget=Math.max(4,Math.floor(96000/(rs.length*tracks.length)));const encoder=new TextEncoder();
        const cellText=value=>{let v=text(value);while(encoder.encode(v).length>stringBudget)v=Array.from(v).slice(0,Math.max(0,Math.floor(Array.from(v).length*.75))).join('');return v;};
        const add=(r,c,value,format)=>{const native=style(format,alignments[this.values.columns[tracks[c].index-(this.values.rowHeaders?1:0)]?.align ?? 'left']),key=JSON.stringify(native);if(!palette.has(key)){palette.set(key,styles.length);styles.push(native);}cells.push([r,c,strings.length,palette.get(key)]);strings.push(native.paint?.wrap===true?cellText(value):cellText(value).replaceAll('\n',' '));};
        for(let r=0;r<rs.length;r++)for(let c=0;c<tracks.length;c++){
            if(this.values.rowHeaders&&tracks[c].index===0){add(r,c,r===0?'':this.view.length?rs[r].index:'',{align:'center'});continue;}
            const col=this.values.columns[tracks[c].index-(this.values.rowHeaders?1:0)];
            if(r===0){add(r,c,col?(col.title+(this.sortState?.key===col.key?(this.sortState.direction==='asc'?' ↑':' ↓'):'')):'',this.values.headerStyle);continue;}
            const row=this.view[rs[r].index-1];
            if(!row||!col){add(r,c,c===(this.values.rowHeaders?1:0)?this.values.emptyText:'',this.values.cellStyle);continue;}
            let format={...this.values.cellStyle,...col.style};
            for(const rule of this.values.conditionalFormats)if((rule.scope==='row'||rule.field===col.key)&&matches(row,rule))format={...format,...rule.style};
            add(r,c,col.format?col.format(row[col.key],row):row[col.key],format);
        }
        const s=this.selection,selectedRows=s?rs.map((t,i)=>i>0&&t.index-1>=s.firstRow&&t.index-1<=s.lastRow?i:-1).filter(i=>i>=0):[],selectedColumns=s?tracks.map((t,i)=>t.index-(this.values.rowHeaders?1:0)>=s.firstColumn&&t.index-(this.values.rowHeaders?1:0)<=s.lastColumn?i:-1).filter(i=>i>=0):[];
        const selection=selectedRows.length&&selectedColumns.length?{first_row:selectedRows[0],last_row:selectedRows.at(-1),first_column:selectedColumns[0],last_column:selectedColumns.at(-1)}:null;
        this.grid.setProperty('frame',{version:1,revision:++this.revision,sheet:0,row_count:Math.max(2,this.view.length+1),column_count:Math.max(1,this.values.columns.length)+(this.values.rowHeaders?1:0),width:this.width,height:this.height,row_header:0,column_header:0,rows:rs,columns:tracks,strings,styles,cells,selection,active:s?[s.firstRow+1,s.firstColumn+(this.values.rowHeaders?1:0)]:null});
        this.verticalTrack.visible=this.maxTop()>0;this.horizontalTrack.visible=this.maxLeft()>0;
        this.vertical.setProperty('value',this.top/Math.max(1,this.maxTop()));this.horizontal.setProperty('value',this.left/Math.max(1,this.maxLeft()));
    }
    getSelection(){return this.selection?immutable({...this.selection}):null;}
    getSelectedRows(){return this.selection&&this.view?this.view.slice(this.selection.firstRow,this.selection.lastRow+1):[];}
    selectRange({firstRow,lastRow=firstRow,firstColumn,lastColumn=firstColumn},kind='cell') {
        if(this.values.selectionMode==='none')return this;
        if(![firstRow,lastRow,firstColumn,lastColumn].every(Number.isInteger)||firstRow<0||lastRow<firstRow||lastRow>=this.view.length||firstColumn<0||lastColumn<firstColumn||lastColumn>=this.values.columns.length)throw RangeError('Table selection is outside displayed rows/columns');
        this.selection={kind,firstRow,lastRow,firstColumn,lastColumn};
        this.cursor={row:lastRow,column:lastColumn};
        this.anchor??={row:firstRow,column:firstColumn};
        this.render();this.publishSelection();return this;
    }
    selectCell(row,column){this.anchor={row,column};return this.selectRange({firstRow:row,firstColumn:column});}
    selectRow(row){this.anchor={row,column:0};return this.selectRange({firstRow:row,firstColumn:0,lastColumn:this.values.columns.length-1},'row');}
    selectColumn(column){this.anchor={row:0,column};return this.selectRange({firstRow:0,lastRow:this.view.length-1,firstColumn:column},'column');}
    clearSelection(){this.selection=null;this.anchor=null;this.cursor=null;this.render();this.publishSelection();return this;}
    publishSelection(){
        const selected=this.getSelectedRows(),s=this.getSelection();
        if(this.dashboard&&this.filterField){this.publishing=true;try{this.dashboard.setFilter(this.tableId,s&&['cell','row'].includes(s.kind)?{field:this.filterField,values:[...new Set(selected.map(r=>r[this.filterField]??null))]}:null);}finally{this.publishing=false;this.hadFilter=Object.hasOwn(this.dashboard.filters,this.tableId);}}
        this.emit(Message.SELECT,{selection:s,rows:selected,rowIds:selected.map(r=>this.rowKey?r[this.rowKey]:null),columnKeys:s?this.values.columns.slice(s.firstColumn,s.lastColumn+1).map(c=>c.key):[]});
    }
    activate(row,column){const item=this.view[row],c=this.values.columns[column];if(item&&c)this.emit(Message.ACTIVATE,{row:item,rowId:this.rowKey?item[this.rowKey]:null,columnKey:c.key,value:item[c.key]});}
    pointer(event){
        const d=event.detail;
        if(!this.enabled||this.values.selectionMode==='none')return;
        if(d?.kind==='wheel'){if((d.modifiers&1)||[6,7].includes(d.button))this.left=Math.max(0,Math.min(this.maxLeft(),this.left+([4,6].includes(d.button)?-80:80)));else this.top=Math.max(0,Math.min(this.maxTop(),this.top+([4,6].includes(d.button)?-3:3)*this.values.rowHeight));this.render();event.preventDefault();return;}
        if(d?.kind==='up'){this.drag=null;this.resize=null;return;}
        if(this.resize&&d?.kind==='move'){const cs=this.values.columns.map((c,i)=>i===this.resize.index?{...c,width:Math.round(Math.max(48,Math.min(2000,this.resize.width+d.x-this.resize.x)))}:c);this.setColumns(cs);event.preventDefault();return;}
        if(!['down','move'].includes(d?.kind)||d.kind==='down'&&d.button!==1||d.kind==='move'&&!this.drag)return;
        const hit=this.grid.hitTest(Math.round(d.x),Math.round(d.y));if(!hit||!this.view.length||!this.values.columns.length)return;
        if(d.kind==='down'&&hit.row===0&&this.values.resizable){const frame=this.grid.values.frame,boundary=frame.columns.find(t=>(!this.values.rowHeaders||t.index>0)&&Math.abs(d.x-frame.row_header-t.clip_end)<=5&&t.clip_end===t.start+t.extent);if(boundary){const index=boundary.index-(this.values.rowHeaders?1:0);this.resize={index,width:this.values.columns[index].width,x:d.x};event.preventDefault();return;}}
        const selector=this.values.rowHeaders&&hit.column===0;const row=Math.max(0,(hit.row??1)-1),column=Math.max(0,(hit.column??0)-(this.values.rowHeaders?1:0)),kind=hit.row===0?'column':selector||hit.column===null?'row':this.values.selectionMode;
        this.grid.setProperty('focused',true);
        if(d.kind==='down'){this.drag=kind;if(!(d.modifiers&1)||!this.anchor)this.anchor={row,column};}
        const a=this.anchor||{row,column},mode=this.drag;
        const firstRow=mode==='column'?0:Math.min(a.row,row),lastRow=mode==='column'?this.view.length-1:Math.max(a.row,row),firstColumn=mode==='row'?0:Math.min(a.column,column),lastColumn=mode==='row'?this.values.columns.length-1:Math.max(a.column,column);
        this.selectRange({firstRow,lastRow,firstColumn,lastColumn},mode);
        if(d.kind==='down'){const now=Date.now(),double=this.lastClick&&now-this.lastClick.time<400&&this.lastClick.row===row&&this.lastClick.column===column&&this.lastClick.header===(hit.row===0);if(double&&hit.row===0&&!selector){const key=this.values.columns[column].key;this.sort(key,this.sortState?.key===key&&this.sortState.direction==='asc'?'desc':'asc');}else if(double&&hit.row>0&&!selector&&hit.column!==null)this.activate(row,column);this.lastClick={time:now,row,column,header:hit.row===0};}
        event.preventDefault();
    }
    key(event){
        const d=event.detail;if(!this.enabled||d?.pressed===false||this.values.selectionMode==='none')return;
        const key=d.key||({113:'ArrowLeft',114:'ArrowRight',111:'ArrowUp',116:'ArrowDown',110:'Home',115:'End',112:'PageUp',117:'PageDown',36:'Enter',9:'Escape',38:'a',65:' '}[d.keyCode]);
        if(key==='Escape'){this.clearSelection();event.preventDefault();return;}
        if(!this.view.length||!this.values.columns.length)return;
        const s=this.selection,row=s?.firstRow??0,column=s?.firstColumn??0;
        if((key==='a'||key==='A')&&(d.modifiers&4)){this.selectRange({firstRow:0,lastRow:this.view.length-1,firstColumn:0,lastColumn:this.values.columns.length-1},'all');event.preventDefault();return;}
        if(key===' '||d.keyCode===65){if(d.modifiers&4)this.selectColumn(column);else if(d.modifiers&1)this.selectRow(row);else return;event.preventDefault();return;}
        if(key==='Enter'){this.activate(row,column);event.preventDefault();return;}
        if(!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End','PageUp','PageDown'].includes(key))return;
        let r=this.cursor?.row??row,c=this.cursor?.column??column;
        if(key==='ArrowLeft')c--;if(key==='ArrowRight')c++;if(key==='ArrowUp')r--;if(key==='ArrowDown')r++;
        if(key==='Home'){c=0;if(d.modifiers&4)r=0;}if(key==='End'){c=this.values.columns.length-1;if(d.modifiers&4)r=this.view.length-1;}
        if(key==='PageUp')r-=Math.max(1,Math.floor(this.height/this.values.rowHeight)-1);if(key==='PageDown')r+=Math.max(1,Math.floor(this.height/this.values.rowHeight)-1);
        r=Math.max(0,Math.min(this.view.length-1,r));c=Math.max(0,Math.min(this.values.columns.length-1,c));this.cursor={row:r,column:c};
        if(!(d.modifiers&1)||!this.anchor)this.anchor={row:r,column:c};const a=this.anchor;
        this.top=Math.max(0,Math.min(this.maxTop(),r*this.values.rowHeight<this.top?r*this.values.rowHeight:(r+1)*this.values.rowHeight>this.top+this.height-this.values.rowHeight?(r+2)*this.values.rowHeight-this.height:this.top));
        const before=this.values.columns.slice(0,c).reduce((sum,col)=>sum+col.width,0),end=before+this.values.columns[c].width,available=this.width-(this.values.rowHeaders?40:0);
        this.left=Math.max(0,Math.min(this.maxLeft(),before<this.left?before:end>this.left+available?end-available:this.left));
        const mode=this.values.selectionMode;this.selectRange({firstRow:mode==='column'?0:Math.min(a.row,r),lastRow:mode==='column'?this.view.length-1:Math.max(a.row,r),firstColumn:mode==='row'?0:Math.min(a.column,c),lastColumn:mode==='row'?this.values.columns.length-1:Math.max(a.column,c)},mode);this.cursor={row:r,column:c};event.preventDefault();
    }
    inspect(){return {rowCount:this.view.length,columns:this.values.columns.map(({format,...c})=>c),sort:this.sortState,selection:this.getSelection(),selectedRowIds:this.getSelectedRows().map(r=>this.rowKey?r[this.rowKey]:null),filters:this.dashboard?.filters??{},visibleRows:this.grid.values.frame.rows.map(t=>t.index-1).filter(i=>i>=0)};}
    destroy(){this.stopDashboard?.();return super.destroy();}
}
export {Table as DataTable};
