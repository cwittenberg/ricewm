import { SHEET_COMMANDS, checkedSheetCommand, checkedSheetClipboard, sheetCommandEffect } from './command-contract.mjs';
import { SHEET_QUERY_ACTIONS, checkedQueryRequest, sheetQueryEffect, sheetQueryAvailable } from './query-contract.mjs';
import {copyRiceBusPayload} from '../bus.mjs';
import {sheetSpecialCellKinds,SHEET_SPECIAL_VALUE_TYPES} from './special-cells-contract.mjs';
import {SHEET_PASTE_MODES,SHEET_PASTE_OPERATIONS} from './paste-contract.mjs';
import {sheetNavigationCommand} from './navigation-contract.mjs';
import {isCommittedWorkbookChange} from './wasm-engine.mjs';
import {checkedSheetFindUiState,checkedSheetFindHistoryRequest,SHEET_FIND_RESULT_COLUMNS} from './find-contract.mjs';

const fail = message => { throw Object.assign(new TypeError(message), {code:'INVALID_SHEET_ACTION'}); };
const plain = value => value !== null && typeof value === 'object' && !Array.isArray(value)
    && Object.getPrototypeOf(value) === Object.prototype;
const label = name => name.split('-').map(word => word[0].toUpperCase()+word.slice(1)).join(' ');
const queryName = action => `query.${action}`;
const committedOutcome = Object.freeze({outcome:'committed-change',applied:true,completed:false,retryable:false,
    description:'The native change committed but the action did not finish presenting it. Never repeat it. Inspect refreshed context, save or undo. refreshRequired reports whether a fresh view is still needed.'});
export function sheetCommittedActionResponse(cause, previousRevision, projection=null) {
    if(!isCommittedWorkbookChange(cause,previousRevision))fail('Expected a confirmed committed workbook change');
    const current=projection?.revision===cause.revision;
    return copyRiceBusPayload({applied:true,completed:false,outcome:'committed-change',revision:cause.revision,retryable:false,
        refreshRequired:!current,message:String(cause.message||'The change committed; inspect before continuing.').slice(0,480),projection:current?projection:null});
}
const clipboardParameters=type=>{
    const generation='Optional positive generation from view.context.clipboard; rejects a replaced or consumed clipboard.';
    if(type==='cut-range')return {fields:{sheetId:'worksheet ID from context',range:'Zero-based logical rectangle, at most 10000 cells.'},clipboard:'Starts a pending move. Paste performs the move; Cancel Clipboard abandons it. Cut itself does not edit workbook data.'};
    if(type==='cancel-clipboard')return {fields:{clipboardGeneration:generation},clipboard:'Clears the pending internal clipboard without changing workbook data.'};
    if(type==='paste-range')return {fields:{sheetId:'destination worksheet ID',row:'zero-based destination row',column:'zero-based destination column',mode:['all','values','formulas'],clipboardGeneration:generation},clipboard:'Copy pastes a snapshot. Cut moves the source once; the current Cut transaction requires mode all.'};
    return {};
};

/** Named automation uses the workbook's existing command/query contracts. No
 * parallel worksheet model, permissive JSON channel or private engine access. */
export function sheetActionCatalog({capabilities = [], query = false, documents = [], clipboard = [], viewReady = true} = {}) {
    const available = new Set(viewReady ? capabilities : []);
    query = query && viewReady;
    clipboard = viewReady ? clipboard : [];
    const actions = Object.fromEntries(SHEET_COMMANDS.filter(type => available.has(type)).map(type => [type, {
        title:label(type), description:type==='replace-text'?'Replace native cell inputs/formula text as one history transaction. target:null means Replace All; a target replaces only that cell. Read query.preview-replace for revision-bound counts.':type==='find-next'?'Find the next or previous native match with wrap, explicit scope/order/formula-or-value options and Excel wildcard syntax. This selects a match without editing cells.':`Run ${type} through the workbook command contract. Coordinates are zero-based; sheetId is a real projection.sheets ID.`,
        effect:sheetCommandEffect(type), outcomes:{committedChange:committedOutcome},
        parameters:{contract:'WorkbookContracts.checkedSheetCommand',discriminator:'type',value:type,omitDiscriminator:true,...clipboardParameters(type),...(type==='create-table'?{fields:{sheetId:'Worksheet ID from context',range:'Zero-based inclusive logical table rectangle, at most 10000 cells',name:'null for the first unused TableN, or a unique table name',hasHeaders:'true uses the first row as headings; false is currently unavailable'},description:'Create a table using the first row as headings. Blank and repeated headings become unique text labels; Undo restores the original cells and table together. Inserted headings and table design controls are not yet available.'}:{}),...(type==='format-ranges'?{fields:{sheetId:'Worksheet ID from context',ranges:'1..512 disjoint logical areas, at most 10000 selected cells',changes:'Same property/value edits as format-cells',borders:'Same border operations as format-cells'},description:'Apply native base-style edits as one history group while preserving selection. Outline/inside borders use each area; shared boundary edges are reconciled on neighbours without copying other formatting to gaps.'}:{}),...(type==='clear-ranges'?{fields:{sheetId:'Worksheet ID from context',ranges:'1..512 disjoint logical rectangles; at most 10000 selected cells total'},description:'Clear contents and hyperlinks across exact native areas as one undoable transaction. Preserve formats, rules, notes and selection. Entire array/spill footprints must be included; no cell matching or per-area mutation runs in the browser.'}:{}),...(type==='select-ranges'?{fields:{sheetId:'Worksheet ID from context',ranges:'1..512 ordered disjoint logical rectangles',active:'{row,column} inside one selected area'},description:'Set a native multi-area selection without changing workbook content. Clear Contents, formatting, explicit single-cell entry and ordered traversal have separate commands. Ctrl+Enter uses fill-selection; multi-area clipboard and saved view persistence remain pending.'}:{}),...(type==='replace-text'?{fields:{options:'WorkbookContracts.checkedSheetFindOptions, with lookIn: formulas',replacement:'Uninterpreted replacement input within 16 KiB',target:'null for Replace All, or {sheetId,row,column} for one cell'},preview:'query.preview-replace at the same expectedRevision'}:{}),...(type==='find-next'?{fields:{options:{sheetId:'real worksheet ID',within:['sheet','workbook','selection'],range:'null, or a zero-based logical rectangle for selection scope',text:'Find pattern; * and ? wildcards, ~ escapes',matchCase:'boolean',wholeCell:'boolean',lookIn:['formulas','values'],order:['rows','columns']},after:'null to start at the first/last match, or {sheetId,row,column}',direction:['forward','backward']}}:{}),...(type==='paste-special'?{fields:{sheetId:'worksheet ID from context',target:{firstRow:'zero-based integer',lastRow:'zero-based inclusive integer',firstColumn:'zero-based integer',lastColumn:'zero-based inclusive integer'},options:{mode:SHEET_PASTE_MODES,operation:SHEET_PASTE_OPERATIONS,transpose:'boolean',skipBlanks:'boolean',link:'boolean'},clipboardGeneration:'Optional positive generation from view.context.clipboard'},clipboard:'Copy may repeat across an exact multiple of the source size. Cut moves once with All, None operation, no transpose, skip blanks or link.'}:{})},
    }]));
    if(actions['rename-table']){
        actions['rename-table'].description='Rename a native table and/or stable column IDs as one Undo step, updating headers and supported formula, rule and retained-file references. Read query.tables and query.table-columns at the same revision before applying.';
        actions['rename-table'].parameters.fields={table:'Original table key from query.tables',name:'null to keep the table identity, or a new table name',columns:'List of {id,name} using stable IDs from query.table-columns; untouched columns are omitted',allowValidationWarning:'Optional boolean, false by default; warning confirmation remains tied to expectedRevision'};
    }
    if(actions['select-special-cells']){
        actions['select-special-cells'].description='Select native Special predicates across explicit logical scopes in one view command. Preserve workbook content and Undo history; no matches reject without changing selection. Protection and merged-cell matching are checked natively.';
        actions['select-special-cells'].parameters.fields={sheetId:'Active worksheet ID from context',ranges:'1..512 explicit logical rectangles; Current geometry and precedent/dependent kinds require the whole worksheet as output domain; reference origins are the native current selection, Direct or All Levels is encoded in kind. Differences use the active worksheet and compare each row at the native active column, or each column at the native active row',kind:sheetSpecialCellKinds(capabilities),types:SHEET_SPECIAL_VALUE_TYPES};
    }
    if(actions['modify-selection']){
        actions['modify-selection'].description='Add or remove an exact native selection rectangle without editing data. Toggle decides from the initial anchor unit: a selected cell/row/column/sheet removes; otherwise it adds. Merged cells stay whole and gaps stay unselected. The last logical cell remains selected.';
        actions['modify-selection'].parameters.fields={sheetId:'Active worksheet ID',range:'Logical gesture rectangle',anchor:'{row,column} inside the gesture',unit:['cell','row','column','sheet'],operation:['add','remove','toggle']};
    }
    if(actions['fill-selection']){
        actions['fill-selection'].description='Fill exact selected areas from one native input, translating relative formulas from origin, as one Undo group without changing selection. Query validation-fill for revision-bound cell alerts. Partial arrays/spills reject. Warning overrides never bypass Stop validation.';
        actions['fill-selection'].parameters.fields={sheetId:'Worksheet ID from context',ranges:'1..512 disjoint areas, at most 10000 selected cells',origin:'{row,column} selected native formula origin',input:'Locale-aware text or formula',allowValidationWarning:'Optional explicit boolean for Warning/Information only'};
    }
    if(actions['traverse-selection']){
        actions['traverse-selection'].description='Move the active cell through the native ordered selection without editing workbook content. Rows matches Tab; columns matches Enter. Wrap across areas, skipping hidden/filtered tracks and merged covered cells. One selected cell stays selected.';
        actions['traverse-selection'].parameters.fields={sheetId:'Active worksheet ID from context',order:['rows','columns'],direction:['forward','backward']};
    }
    if (SHEET_COMMANDS.some(type => available.has(type))) actions.command = {
        title:'Workbook command', outcomes:{committedChange:committedOutcome}, description:'Compatibility entrypoint. Pass {command: checkedSheetCommand}; only advertised workbook capabilities are accepted.',
        parameters:{command:'A complete checked workbook command, including type.'},
    };
    if(available.has('select-range')||available.has('navigate-range')||available.has('navigate-reference'))actions['navigate.reference']={title:'Go To Reference',effect:'query',description:available.has('navigate-reference')?'Resolve a cell/range, whole rows/columns, worksheet-scoped or workbook name, or table reference in the native engine. Multi-area and Go To Special remain pending.':'Select a literal cell/range, whole rows/columns, or a quoted worksheet reference through native navigation.',parameters:{reference:available.has('navigate-reference')?"A1:C20, 'Quarter 1'!Sales, or SalesTable[Amount]":"A1:C20 or 'Quarter 1'!A1"}};
    if (query) for (const action of SHEET_QUERY_ACTIONS.filter(action=>sheetQueryAvailable(action,capabilities))) actions[queryName(action)] = {
        title:label(action),description:`Run ${action} through the revisioned workbook query contract.`,effect:sheetQueryEffect(action),
        parameters:{contract:'WorkbookContracts.checkedQueryRequest',discriminator:'action',value:action,omitDiscriminator:true,...(action==='special-cells'?{fields:{sheetId:'Worksheet ID from context',ranges:'1..512 explicit logical rectangles; Current geometry and precedent/dependent kinds require the whole worksheet as output domain; reference origins are the native current selection, Direct or All Levels is encoded in kind. Differences use the active worksheet and compare each row at the native active column, or each column at the native active row',kind:sheetSpecialCellKinds(capabilities),types:SHEET_SPECIAL_VALUE_TYPES},selectionApplied:false,description:'Read a native disjoint selection plan without applying it. Use select-special-cells with the same explicit scope and revision to apply the native predicate.'}:{}),...(action==='find-results'?{fields:{options:'WorkbookContracts.checkedSheetFindOptions',sort:{column:SHEET_FIND_RESULT_COLUMNS,direction:['ascending','descending']},after:'null, or last native {sheetId,row,column}; reset on sort change',limit:'1..16'},ordering:'Full native sort keys across the complete match set; returned labels are previews, not client-side sort keys.'}:{})},
    };
    if(actions['query.validation-table-rename']){actions['query.validation-table-rename'].description='Check all proposed literal headings in one native candidate overlay without changing cells, metadata or history. Custom rules see the prospective heading set together; Stop and unsupported rules cannot be overridden.';actions['query.validation-table-rename'].parameters.fields={table:'Native table key/alias from query.tables',name:'null to retain the table name, or its proposed new name',columns:'Exact proposed {id,name} list used by rename-table; no warning override in a query'};}
    if(actions['query.tables'])actions['query.tables'].parameters.fields={sheetId:'null for all sheets, or a worksheet ID',offset:'0..256',limit:'1..64',literal:'Table name search text, at most 512 UTF-8 bytes'};
    if(actions['query.table-columns'])actions['query.table-columns'].parameters.fields={table:'Original table key from query.tables',offset:'0..16384',limit:'1..64'};
    for (const operation of documents) {
        if (!['new','open','save','save-as','save-as-xlsx','save-as-ods','save-as-csv','save-as-tsv'].includes(operation)) fail('Unsupported document action');
        actions[`document.${operation}`] = {title:label(operation),effect:'external',
            description:'Use the native workbook file workflow. File choices, overwrite/conversion and unsaved-change dialogs retain their normal confirmation.',parameters:{}};
    }
    for(const operation of clipboard){
        if(!['copy','cut','paste','copy-text','paste-text','paste-values'].includes(operation))fail('Unsupported clipboard action');
        if(!available.has('paste-text')||['copy','copy-text'].includes(operation)&&!available.has('copy-range')||operation==='cut'&&!available.has('cut-range')||operation==='paste'&&!available.has('paste-special'))continue;
        actions[`clipboard.${operation}`]={title:label(operation),effect:'external',outcomes:{committedChange:committedOutcome},
            description:['copy','cut'].includes(operation)?'Use the normal system Copy/Cut workflow. Publish a native snapshot identity and displayed TSV companion; Cut does not delete cells until a generation-checked native paste.':operation==='paste'?'Use the normal system Paste workflow. A matching document/token/generation selects native Paste Special; external text uses one native TSV transaction. HTML/rich cross-workbook import remains pending.':operation==='copy-text'?'Copy displayed values as plain TSV text through the granted system clipboard service. Internal Copy remains available if the host write fails.':'Read plain TSV text through the granted system clipboard service and paste it as one native workbook transaction. No HTML import. Values mode treats formula inputs as literal text.',
            parameters:{sheetId:'worksheet ID from context',[['copy','cut','copy-text'].includes(operation)?'range':'target']:'Zero-based logical rectangle, at most 10000 cells.',...(operation==='paste'?{mode:['all','values','formulas']}:{})}};
    }
    if(available.has('find-next')){
        const ui={effect:'query',scope:'Find UI session state; no workbook content change'};
        actions['find.state']={...ui,title:'Read Find settings',parameters:{}};
        actions['find.configure']={...ui,title:'Configure Find settings',parameters:{contract:'WorkbookContracts.checkedSheetFindUiState'},description:'Set the next dialog preferences, replacement and Options visibility. Does not execute a search or alter its last submitted scope.'};
        actions['find.history']={...ui,title:'Read recent Find inputs',parameters:{kind:['find','replace'],offset:'0..10',limit:'1..10'},description:'Page typed search/replacement history in this window. Does not read cell results or execute a search.'};
        actions['find.repeat']={...ui,title:'Repeat Find',parameters:{direction:['forward','backward']},description:'Repeat the last submitted native search. Sheet/workbook searches use the active sheet; selection searches keep their captured logical scope. No prior search opens Find without inventing a pattern.'};
    }
    actions['view.refresh']={title:'Refresh worksheet view',effect:'query',description:'Read a fresh native viewport without replaying any edit; also resumes view-dependent actions after a committed-change presentation failure.',parameters:{}};
    if(viewReady)actions['view.scroll']={title:'Scroll worksheet view',effect:'query',description:'Read and display a native viewport at logical row/column offsets, clamped to the current viewport bounds. Preserve selection, workbook content and Undo history. Native geometry resolves hidden, filtered and frozen tracks.',parameters:{topRow:'0..1048575, zero-based logical offset',leftColumn:'0..16383, zero-based logical offset'}};
    if(viewReady)actions['view.context']={title:'Read visible worksheet context',effect:'query',description:'Page rendered labels, worksheet identities and ordered selection areas. This reads presentation only; use workbook queries for authoritative data.',parameters:{cellOffset:'0..4096',cellLimit:'1..64',sheetOffset:'0..65535',sheetLimit:'1..64',rangeOffset:'Optional 0..512; provide with rangeLimit',rangeLimit:'Optional 1..64; defaults to 16 selection areas'}};
    return Object.freeze(Object.fromEntries(Object.entries(actions).map(([name,descriptor]) => [name,Object.freeze(descriptor)])));
}

/** Validate without executing an engine operation or opening any UI. */
export function checkedSheetAction(operation, data, {revision, expectedRevision, capabilities = [], query = false, documents = [], clipboard = [],sheets=[],activeSheetId,viewReady=true} = {}) {
    if (!Number.isSafeInteger(revision) || revision < 0 || expectedRevision !== revision) fail('Workbook changed; inspect its current context');
    if (typeof operation !== 'string' || !plain(data)) fail('Sheet action requires an operation and plain JSON arguments');
    const catalog = sheetActionCatalog({capabilities,query,documents,clipboard,viewReady});
    if (!Object.hasOwn(catalog,operation)) fail('Sheet action is unavailable for this workbook');
    const special=operation==='select-special-cells'?data:operation==='query.special-cells'?data:operation==='command'&&data.command?.type==='select-special-cells'?data.command:null;
    if(special&&!sheetSpecialCellKinds(capabilities).includes(special.kind))fail('Native workbook does not advertise this Special predicate');
    if(operation.startsWith('find.')){
        if(operation==='find.state'){
            if(Object.keys(data).length)fail('Find state does not accept arguments');
            return Object.freeze({kind:'find-ui',operation:'state'});
        }
        if(operation==='find.configure')return Object.freeze({kind:'find-ui',operation:'configure',data:checkedSheetFindUiState(data)});
        if(operation==='find.history')return Object.freeze({kind:'find-ui',operation:'history',data:checkedSheetFindHistoryRequest(data)});
        if(operation==='find.repeat'){
            if(Object.keys(data).length!==1||!['forward','backward'].includes(data.direction))fail('Invalid Find repeat direction');
            return Object.freeze({kind:'find-ui',operation:'repeat',direction:data.direction});
        }
    }
    if(operation==='view.refresh'){
        if(Object.keys(data).length)fail('View refresh does not accept command data');
        return Object.freeze({kind:'refresh'});
    }
    if(operation==='view.scroll'){
        if(Object.keys(data).length!==2||Object.keys(data).some(key=>!['topRow','leftColumn'].includes(key))||!Number.isSafeInteger(data.topRow)||data.topRow<0||data.topRow>=1048576
            ||!Number.isSafeInteger(data.leftColumn)||data.leftColumn<0||data.leftColumn>=16384)fail('Scroll requires bounded topRow and leftColumn offsets');
        return Object.freeze({kind:'scroll',topRow:data.topRow,leftColumn:data.leftColumn});
    }
    if(operation==='view.context')return Object.freeze({kind:'view',query:checkedSheetViewRequest(data)});
    if(operation==='navigate.reference'){
        if(Object.keys(data).length!==1||!Object.hasOwn(data,'reference'))fail('Go To requires exactly one reference');
        return Object.freeze({kind:'command',command:sheetNavigationCommand(data.reference,{sheets,activeSheetId,capabilities})});
    }
    if(operation.startsWith('clipboard.')){
        const name=operation.slice(10),copy=['copy','cut','copy-text'].includes(name),normalPaste=name==='paste';
        if(Object.keys(data).length!==(normalPaste?3:2)||!Object.hasOwn(data,'sheetId')||!Object.hasOwn(data,copy?'range':'target')||normalPaste&&!['all','values','formulas'].includes(data.mode))fail('Invalid clipboard action fields');
        const checked=checkedSheetCommand(copy?{type:name==='cut'?'cut-range':'copy-range',...data}:normalPaste?{type:'paste-special',sheetId:data.sheetId,target:data.target,options:{mode:data.mode,operation:'none',transpose:false,skipBlanks:false,link:false}}:{type:'paste-text',...data,text:'',mode:name==='paste-values'?'values':'all'});
        return Object.freeze({kind:'clipboard',operation:name,data:Object.freeze({sheetId:checked.sheetId,[copy?'range':'target']:checked[copy?'range':'target'],...(normalPaste?{mode:data.mode}:{})})});
    }
    if (operation.startsWith('document.')) {
        if (Object.keys(data).length) fail('Document workflows do not accept caller-supplied file handles or paths');
        return Object.freeze({kind:'document',operation:operation.slice(9)});
    }
    if (operation.startsWith('query.')) {
        if (Object.hasOwn(data,'action')) fail('Named query arguments must omit action');
        return Object.freeze({kind:'query',query:checkedQueryRequest({action:operation.slice(6),...data})});
    }
    let command;
    if (operation === 'command') {
        if (Object.keys(data).length !== 1 || !Object.hasOwn(data,'command')) fail('Command action requires exactly one command');
        command = checkedSheetCommand(data.command);
    } else {
        if (Object.hasOwn(data,'type')) fail('Named command arguments must omit type');
        command = checkedSheetCommand({type:operation,...data});
    }
    if (!capabilities.includes(command.type)) fail('Workbook does not advertise this command');
    return Object.freeze({kind:'command',command});
}

/** Context is read-only. Persistent query mutations must use an action so the
 * host's revision/confirmation path is identical to other workbook edits. */
export function checkedSheetContextQuery(value) {
    if(value?.action==='view-context'){
        const {action,...page}=value;
        return Object.freeze({action,...checkedSheetViewRequest(page)});
    }
    const query = checkedQueryRequest(value);
    if (sheetQueryEffect(query.action) !== 'query') fail('Use the named query action to change workbook filters');
    return query;
}

export function checkedSheetViewRequest(value){
    const fields=['cellOffset','cellLimit','sheetOffset','sheetLimit'];
    const extra=Object.hasOwn(value??{},'rangeOffset')||Object.hasOwn(value??{},'rangeLimit')?['rangeOffset','rangeLimit']:[];
    if(!plain(value)||Object.keys(value).length!==4+extra.length||[...fields,...extra].some(key=>!Object.hasOwn(value,key)||!Number.isSafeInteger(value[key])))fail('Invalid view context page');
    if(value.cellOffset<0||value.cellOffset>4096||value.sheetOffset<0||value.sheetOffset>65535||value.cellLimit<1||value.cellLimit>64||value.sheetLimit<1||value.sheetLimit>64)fail('Invalid view context page bounds');
    if(extra.length&&(value.rangeOffset<0||value.rangeOffset>512||value.rangeLimit<1||value.rangeLimit>64))fail('Invalid selection context page bounds');
    return Object.freeze({...value});
}

/** A bounded presentation page, never a second workbook or calculation model. */
export function sheetAutomationProjection(projection,page={cellOffset:0,cellLimit:16,sheetOffset:0,sheetLimit:16}){
    checkedSheetViewRequest(page);if(!projection)return null;
    const frame=projection.frame, cells=frame?.cells||[],sheets=projection.sheets||[];
    let selection=projection.selection;
    if(selection?.ranges){
        const rangeOffset=page.rangeOffset??0,rangeLimit=page.rangeLimit??16;
        selection={...selection,ranges:selection.ranges.slice(rangeOffset,rangeOffset+rangeLimit),rangeOffset,rangeCount:selection.ranges.length,nextRangeOffset:rangeOffset+rangeLimit<selection.ranges.length?rangeOffset+rangeLimit:null};
    }
    const visibleCells=cells.slice(page.cellOffset,page.cellOffset+page.cellLimit).map(cell=>({
        row:frame.rows[cell[0]]?.index,column:frame.columns[cell[1]]?.index,display:frame.strings[cell[2]],
    }));
    return {documentTitle:projection.documentTitle,revision:projection.revision,activeSheetId:projection.activeSheetId,activeSheet:sheets.find(sheet=>sheet.id===projection.activeSheetId)||null,
        selection,capabilities:projection.capabilities,clipboard:checkedSheetClipboard(projection.clipboard),
        ...(projection.scroll?{scroll:projection.scroll}:{}),
        sheets:sheets.slice(page.sheetOffset,page.sheetOffset+page.sheetLimit),sheetOffset:page.sheetOffset,sheetCount:sheets.length,
        nextSheetOffset:page.sheetOffset+page.sheetLimit<sheets.length?page.sheetOffset+page.sheetLimit:null,
        visibleCells,cellOffset:page.cellOffset,visibleCellCount:cells.length,
        nextCellOffset:page.cellOffset+page.cellLimit<cells.length?page.cellOffset+page.cellLimit:null,
        note:'Visible display labels only. Page with view.context or context query {action:"view-context",cellOffset,cellLimit,sheetOffset,sheetLimit}. Workbook queries return authoritative data.'};
}

/** Preserve an unusually complex engine query as JSON text within the existing
 * bus byte bound. No model data is dropped and transport limits stay intact. */
export function sheetAutomationResponse(value){
    try{return copyRiceBusPayload(value);}catch(error){
        if(!(error instanceof RangeError)||!Object.hasOwn(value,'result'))throw error;
        const {result,...rest}=value;
        try{return copyRiceBusPayload({...rest,resultEncoding:'json',resultJson:JSON.stringify(result)});}
        catch{throw Error('The query result exceeds the automation response limit. Request a smaller page.');}
    }
}
