import {exposeAppCopilot} from '@ricewm/app';
import { createSensitivityDialog, sensitivityCaption } from '@ricewm/app';
import {createNamedRangesDialog,nameFormulaToken} from './named-ranges.mjs';
import {checkedCellValidation,createValidationDialog,createValidationList} from './validation-ui.mjs';
import { SHEET_FONT_FAMILIES, adjacentFontSize, changeDecimals } from './home-format-actions.mjs';
import { headerAxis, headerRange, headerPreview } from './header-selection.mjs';
import { formatPainterCommand } from './format-painter.mjs';
import { createSheetCharts } from './chart-controller.mjs';
import { createChartOptionsDialog, createChartTypeDialog, createChartLegendDialog } from './chart-editor.mjs';
import { createChartDataDialog } from './chart-data-dialog.mjs';
import {createSheetRibbon} from './ribbon.mjs';
import {createFontColorPicker} from './font-color-picker.mjs';
import {createSheetContextMenu} from './context-menu.mjs';
import {createFormatDialog} from './format-dialog.mjs';
import {createStyleGallery} from './style-gallery.mjs';
import {formulaReferences} from './formula-references.mjs';
import {
    Button, Column, Container, Label, Menu, MenuBar, MenuItem, Message, Row, State,
    ZoomControl, Tabs, TextField, ToolBar, VirtualGrid, Stack, ScrollBar,
} from "@ricewm/app";
import { createAboutDialog } from "./about.mjs";
import { checkedEnginePort, checkedProjection } from "./engine-port.mjs";
import { SheetViewportCache } from "./viewport-cache.mjs";
import { checkedSheetCommand } from "./command-contract.mjs";
import { createFilterValuesDialog, createFindDialog } from "./data-dialogs.mjs";
import { openFilterDataProvider, openFindDataProvider } from "./data-provider.mjs";
import { workbookGrantFromOpen } from "./file-open.mjs";
import { createStructureDialog, structureCommand } from './structure-actions.mjs';
import { createLayoutDialog, headerResizeHit, resizeDragValue, resizeCommand } from './layout-dialog.mjs';
import { createSortDialog } from './sort-dialog.mjs';
import { createDataToolsDialog } from './data-tools-dialog.mjs';
import { characterOffset, completeFormula, referenceInsertion } from './formula-entry.mjs';
import { autoSumCommand, betweenCells, createFunctionDialog, fillSelection, rangeAddress, selectionRange } from './editor-actions.mjs';

const STARTUP_STATUS = "Opening workbook…";


export function startSheetShell({ app, createWindow, log, services, bus, instanceId }, enginePort = null) {
    const engine = checkedEnginePort(enginePort);
    let closed = false;
    let busy = false;
    let closePending = false;
    let functionOpen = false;
    let starting = Boolean(engine), pendingOpen = null;
    const isBusy = () => busy || closePending || functionOpen;
    let projection = null, charts = null;
    let viewport = { top: 0, left: 0, rows: 40, columns: 20 };
    const zoomListeners = new Set();
    const zoomSupported = () => Boolean(BigInt(app.hostCapabilities || 0) & (1n << 20n));
    const zoomView = () => ({ zoom: viewport.zoomPercent || 100, percent: viewport.zoomPercent || 100, ready: zoomSupported() && Boolean(projection?.frame) && !closed && !isBusy() });
    const zoomViewport = {
        get view() { return zoomView(); },
        onViewChanged(listener) { zoomListeners.add(listener); listener(zoomView()); return () => zoomListeners.delete(listener); },
        setZoom(percent) {
            if (!zoomView().ready || !Number.isInteger(percent) || percent < 25 || percent > 400 || percent === (viewport.zoomPercent || 100)) return;
            queueViewport({ zoomPercent: percent, followSelection: draft === null });
            updateZoom();
        },
    };
    function updateZoom() { const view = zoomView(); for (const listener of zoomListeners) listener(view); }
    let pendingViewport = false, viewportRead = null, inlineEditor = null, inlineLayer = null, inlinePosition = null, entryMode = false, pointing = null;
    let inputTransaction = false, drainingInput = false, commandInFlight = false;
    const queuedInput = [];
    let deferredSelection = null;
    const viewportCache = new SheetViewportCache();
    let dialog = null, contextMenu = null, contextColumn = null;
    let dismissValidation = null;
    let dataSession = null;
    let grid = null;
    let draft = null;
    let draftTarget = null;
    let formatPainter = null;
    const commandItems = new Map();
    const documentItems = new Map();
    const dataItems = new Map();
    const editorItems = [];
    const fontItems = [];
    const fontSize = new TextField({ name: 'font-size', value: '', enabled: false, accessibleName: 'Font size in points' });
    const fontFamily=new Menu({name:'font-family',text:'Font',enabled:false,accessibleName:'Font family',state:State.START_ALIGNED|State.TRAILING_ICON});
    for(const [index,family]of SHEET_FONT_FAMILIES.entries()){const item=new MenuItem({name:`font-family-${index}`,text:family});item.on(Message.ACTIVATE,()=>performEditorAction('font-family:'+family));fontFamily.append(item);}
    const structureItems = [];
    let pointerDrag = null, rangeAnchor = null, rangeExtent = null;
    let headerAnchor = null;
    let formulaBarRequestedVisible = true, offscreenDraft = false;
    const status = new Label({ name: "status-text", text: engine ? STARTUP_STATUS : "Spreadsheet engine unavailable" });
    const address = new TextField({ name: "name-box", value: "", readOnly: true, enabled: false });
    const formula = new TextField({ name: "formula-input", value: "", readOnly: true, enabled: false });
    const validationListButton=new Button({name:'validation-list-open',text:'',icon:74,visible:false,enabled:false,tooltip:'Choose an allowed value (Alt+Down)',accessibleName:'Choose an allowed value'});
    validationListButton.on(Message.ACTIVATE,()=>showValidationList());
    const formulaBar = new Row({ name: "formula-bar", text: "panel:fixed:flush" }, [
        new Container({text:"panel:fixed:width=112:flush"}, [address]),
        new Label({ text: "fx" }), formula,validationListButton,
    ]);
    const sheetTabs = new Tabs({ name: "sheet-tabs", text: "panel:tabs:integrated:flush:fixed:tight-stack" }, [new Label({ text: "No workbook loaded" })]);
    const zoom = new ZoomControl({ name: "sheet-zoom", viewport: zoomViewport, compact: true, min: 25, max: 400, showFit: false });
    zoom.slider.setProperty("accessibleName", "Sheet zoom percent");
    status.on(Message.CHANGE, event => { if (event.detail?.property === "text") {
        if (/^Ready(?:$|[ .·])/i.test(status.text)) { status.text = ""; return; }
        status.setProperty("tooltip", status.text);
    } });
    const root = new Column({ name: "sheet-root" });
    const closeAbout = () => {
        if (!dialog) return;
        dialog.parent?.remove(dialog, { destroy: true });
        dialog = null;
        functionOpen = false; updateEnabled(); focusGrid();
    };
    const showAbout = () => {
        if (closed || closePending || dialog || dataSession) return;
        dialog = createAboutDialog(closeAbout, { workbook: Boolean(engine), files: Boolean(engine?.documents),
            formats: engine?.documents?.formats || ['xlsx'], structure: supports('create-sheet'), sorting: supports('sort-range') });
        root.append(dialog);
    };
    const sensitivityBadge=new Button({name:'sensitivity-badge',icon:119,state:State.FLAT,visible:false});
    const sensitivityItem=new MenuItem({name:'sensitivity',text:'Sensitivity…',icon:119,visible:false});
    sensitivityItem.on(Message.ACTIVATE,()=>{
        if(closed||isBusy()||dialog||!engine?.documents?.sensitivity?.length)return;
        const initial=projection;
        functionOpen=true;updateEnabled();
        dialog=createSensitivityDialog({labels:engine.documents.sensitivity,onClose:closeAbout,onApply:async change=>{
            if(closed||projection!==initial)throw Error('Workbook changed. Reopen Sensitivity.');
            await engine.documents.changeSensitivity(change);
            await readProjection();
        }});
        root.append(dialog);
    });
    sensitivityBadge.on(Message.ACTIVATE,()=>sensitivityItem.sendMessage(Message.ACTIVATE));
    const action = (name, text, callback) => {
        const item = new MenuItem({ name, text, enabled: Boolean(callback) });
        if (callback) item.on(Message.ACTIVATE, callback);
        return item;
    };
    const engineAction = (name, text) => {
        const item = action(name, text, () => dispatchCommand({ type: name }));
        item.setProperty("enabled", false);
        commandItems.set(name, item);
        return item;
    };
    const supports = type => projection?.capabilities.includes(type) === true;
    const commandAvailable = type => supports(type) && (projection?.history === undefined ||
        (type !== 'undo' || projection.history.canUndo) && (type !== 'redo' || projection.history.canRedo));
    function updateEnabled() {
        const labelText=sensitivityCaption(engine?.documents?.sensitivity);
        sensitivityBadge.text=sensitivityCaption(engine?.documents?.sensitivity,{compact:true});
        sensitivityBadge.setProperty('accessibleName',labelText);sensitivityBadge.setProperty('tooltip',labelText);sensitivityBadge.setProperty('visible',Boolean(labelText));
        sensitivityItem.setProperty('visible',Boolean(labelText));sensitivityItem.setProperty('enabled',!closed&&!isBusy());
        sensitivityBadge.setProperty('enabled',sensitivityItem.getProperty('enabled'));
        ribbon.refreshIcons();
        validationListButton.setProperty('visible',supports('select-validation-value')&&Number.isInteger(projection?.selection.validation?.listRule));
        validationListButton.setProperty('enabled',!closed&&!isBusy()&&draft===null&&!projection?.selection.readOnly);
        // Commands are serialized by the input/engine guards. A short query or
        // cell commit must not flash every toolbar control disabled and back.
        if (busy && !closed && !closePending && !functionOpen) {
            formula.setProperty('readOnly', true);
            // History may already have advanced while bounded chart results are
            // loading. Keep those buttons accurate and queue further clicks.
            for (const {item,capability} of editorItems) if(capability==='undo'||capability==='redo')item.setProperty('enabled',commandAvailable(capability));
            return;
        }
        updateZoom();
        const editable = !closed && !isBusy() && supports("edit-cell") && Boolean(projection?.frame?.active) && !projection.selection.readOnly;
        formula.setProperty("enabled", editable);
        formula.setProperty("readOnly", !editable);
        for (const [type, item] of commandItems) item.setProperty("enabled", !closed && !isBusy() && commandAvailable(type));
        for (const [name, items] of documentItems) for (const item of items) {
            item.setProperty("enabled", !closed && !isBusy() && typeof engine?.documents?.[name] === 'function');
        }
        for (const item of dataItems.values()) item.setProperty('enabled', !closed && !isBusy() &&
            typeof engine?.query === 'function' && Boolean(projection?.frame?.active));
        const hasAppliedFilter=projection?.filterState ? projection.filterState.appliedColumns>0 : Boolean(projection?.frame?.filter_columns?.some(column=>column.active));
        dataItems.get('clear-filters')?.setProperty('enabled',!closed&&!isBusy()&&typeof engine?.query==='function'&&hasAppliedFilter);
        for (const { item, capability, available } of editorItems) item.setProperty('enabled', !closed && !isBusy() &&
            commandAvailable(capability) && Boolean(activeCell()) && (!available || available()));
        for (const { item } of editorItems) item.refreshSelection?.(projection);
        for(const {item,capability} of editorItems)if(capability==='paint-format')item.setProperty('checked',Boolean(formatPainter));
        for(const {item,capability} of editorItems)if(capability==='toggle-autofilter'&&item.name?.endsWith('toggle-filter'))item.setProperty('checked',Boolean(projection?.frame && (projection.frame.filter_header_row!==undefined || projection.frame.filter_columns?.length)));
        fontSize.setProperty('enabled', !closed && !isBusy() && supports('format-range') && Boolean(projection?.selection.font));
        fontFamily.setProperty('enabled',!closed&&!isBusy()&&supports('format-cells')&&Boolean(projection?.selection.font));
        for(const {item}of editorItems){if(item.name?.endsWith('wrap-text'))item.setProperty('checked',projection?.selection.format?.wrapText===true);if(item.name?.includes('vertical-'))item.setProperty('checked',item.name.endsWith('vertical-'+(projection?.selection.format?.vertical||'bottom')));}
        for (const { item, property } of fontItems) item.setProperty('checked', property === 'weight'
            ? projection?.selection.font?.weight >= 600 : projection?.selection.font?.[property] === true);
        for (const { item, capability, available } of structureItems) item.setProperty('enabled', !closed && !isBusy() &&
            supports(capability) && Boolean(projection) && (!available || available()));
        for (const tab of sheetTabs.children) if (tab instanceof Button) tab.setProperty("enabled", !closed && !isBusy() && supports("select-sheet"));
    }
    function cancelFormatPainter() {
        if (!formatPainter) return;
        formatPainter = null;
        for (const {item,capability} of editorItems) if (capability === 'paint-format') item.setProperty('checked', false);
        status.text = '';
    }
    function activeCell() {
        const active = projection?.frame?.active;
        return active ? { sheetId: projection.activeSheetId, row: active[0], column: active[1] } : null;
    }
    function clearInlineEditor() {
        if (inlineLayer) gridStack.remove(inlineLayer,{destroy:true});
        inlineLayer = inlineEditor = inlinePosition = null;
    }
    function focusGrid() { formula.setProperty('focused',false);grid?.setProperty('focused',true); }
    function showDraftFormulaBar(required) {
        offscreenDraft=required;
        formulaBar.setProperty('visible',formulaBarRequestedVisible||required);
    }
    function syncFormulaReferences() {
        if(draft===null)showDraftFormulaBar(false);
        if (!grid || !projection?.frame) return;
        grid.setProperty('state',grid.getProperty('state')&~State.PRESSED);
        grid.setFormulaReferences(formulaReferences(draft, {
            sheetName: projection.sheets.find(sheet => sheet.id === projection.activeSheetId)?.name || '',
            rows: projection.frame.row_count, columns: projection.frame.column_count,
        }));
    }
    function fillTarget(drag) {
        const vertical=Math.abs(drag.last.row-drag.first.row)>=Math.abs(drag.last.column-drag.first.column);
        return {...drag.source,...(vertical?{firstRow:Math.min(drag.source.firstRow,drag.last.row),lastRow:Math.max(drag.source.lastRow,drag.last.row)}:
            {firstColumn:Math.min(drag.source.firstColumn,drag.last.column),lastColumn:Math.max(drag.source.lastColumn,drag.last.column)})};
    }
    function showFillTarget(drag) {
        const range=fillTarget(drag);
        grid?.setProperty('state',grid.getProperty('state')|State.PRESSED);
        grid?.setFormulaReferences([{first_row:range.firstRow,last_row:range.lastRow,first_column:range.firstColumn,last_column:range.lastColumn,color:0}]);
    }
    function cancelDraft() {
        draft = null; draftTarget = null; pointing = null; entryMode = false;syncFormulaReferences();
        clearInlineEditor();formula.setProperty('state',formula.getProperty('state')&~State.KEY_NAVIGATION);formula.setProperty('value',projection?.selection.formula || '');
    }
    function syncDraft(value, source = null) {
        draft = value;formula.setProperty('value',value);
        if(inlineEditor && source !== inlineEditor)inlineEditor.setProperty('value',value);
        syncFormulaReferences();
    }
    function visibleMerge(target) {return projection?.frame.merges?.find(m=>target&&target.row>=m.first_row&&target.row<=m.last_row&&target.column>=m.first_column&&target.column<=m.last_column);}
    function editorPosition() {
        const frame=projection?.frame;
        const merged=visibleMerge(draftTarget);
        const row=merged?frame.rows[merged.row_slot]:frame?.rows.find(t=>t.index===draftTarget?.row),column=merged?frame.columns[merged.column_slot]:frame?.columns.find(t=>t.index===draftTarget?.column);
        if(!viewport.pixelWidth || !row || !column)return null;
        const lastRow=merged?frame.rows.findLast(t=>t.index<=merged.last_row&&t.index>=merged.first_row):row;
        const lastColumn=merged?frame.columns.findLast(t=>t.index<=merged.last_column&&t.index>=merged.first_column):column;
        const w=viewport.pixelWidth,h=viewport.pixelHeight,x=frame.row_header+column.clip_start,y=frame.column_header+row.clip_start;
        const px=Math.min(999,Math.round(x/w*1000)),py=Math.min(999,Math.round(y/h*1000));
        const pw=Math.max(1,Math.min(1000-px,Math.round(Math.max(1,lastColumn.clip_end-column.clip_start)/w*1000)));
        const ph=Math.max(1,Math.min(1000-py,Math.round(Math.max(1,lastRow.clip_end-row.clip_start)/h*1000)));
        return `positioned-item:${px}:${py}:${pw}:${ph}`;
    }
    function inlineDocumentStyle() {
        // Use the engine's visible style (including conditional formatting).
        // Empty/offscreen cells fall back to bounded selected-cell metadata.
        const frame=projection?.frame;
        const merged=visibleMerge(draftTarget);
        const row=merged?.row_slot??frame?.rows.findIndex(track=>track.index===draftTarget?.row);
        const column=merged?.column_slot??frame?.columns.findIndex(track=>track.index===draftTarget?.column);
        const cell=frame?.cells.find(cell=>cell[0]===row&&cell[1]===column);
        const style=cell?frame.styles[cell[3]]:null;
        const format=projection?.selection.format;
        const foreground=style?style.paint?.foreground:format?.fontColor;
        const background=(style?style.paint?.background:format?.fillColor)||'#FFFFFF';
        const vertical=(style?style.paint?.vertical:format?.vertical)||'bottom';
        return {
            documentZoom:frame?.zoom_percent||100,
            documentFont:style?.font||projection.selection.font||{family:'Arial',size_pt:12,weight:400,italic:false,underline:false},
            documentPaint:{background,...(foreground?{foreground}:{}),vertical},
        };
    }
    function beginEditing(text = null, mode = text !== null) {
        if(closed || isBusy() || !supports('edit-cell') || !activeCell() || projection.selection.readOnly)return;
        cancelFormatPainter();
        clearInlineEditor();draftTarget=activeCell();draft=text===null?projection.selection.formula:text;entryMode=mode;pointing=null;syncFormulaReferences();
        formula.setProperty('value',draft);formula.setProperty('state',entryMode?formula.getProperty('state')|State.KEY_NAVIGATION:formula.getProperty('state')&~State.KEY_NAVIGATION);grid?.setProperty('focused',false);
        const position=editorPosition();
        if(position) {
            showDraftFormulaBar(false);
            inlineEditor=new TextField({name:'cell-editor',...inlineDocumentStyle(),value:draft,focused:true,state:entryMode?State.KEY_NAVIGATION:0,accessibleName:'Cell input'});
            bindEditor(inlineEditor);formula.setProperty('focused',false);
            inlinePosition=new Stack({text:position},[inlineEditor]);
            inlineLayer=new Stack({name:'cell-editor-layer',text:'panel:overlay:flush'},[inlinePosition]);gridStack.append(inlineLayer);
        }else {showDraftFormulaBar(true);formula.setProperty('focused',true);formula.setSelection?.(Array.from(draft).length);}
        status.text=entryMode?'Enter commits and moves; arrows point to cells after =; F2 edits text':'Edit cell text; Enter commits, Escape cancels';
    }
    const keyName = detail => (["Escape", "Tab", "Enter", "ArrowUp", "ArrowLeft", "ArrowRight", "ArrowDown", "PageUp", "PageDown", "Home", "End"].includes(detail?.key) ? detail.key : ({ 9: "Escape", 23: "Tab", 36: "Enter", 104: "Enter", 112:'PageUp',117:'PageDown',110:'Home',115:'End',
        111: "ArrowUp", 113: "ArrowLeft", 114: "ArrowRight", 116: "ArrowDown" })[detail?.keyCode]);
    function neighboringCell(active,key,modifiers=0) {
        const vertical=['ArrowUp','ArrowDown','Enter','PageUp','PageDown'].includes(key);
        const step=['ArrowUp','ArrowLeft','PageUp'].includes(key)||(['Tab','Enter'].includes(key)&&(modifiers&1))?-1:1;
        const field=vertical?'row':'column',tracks=vertical?projection.frame.rows:projection.frame.columns,maximum=vertical?projection.frame.row_count:projection.frame.column_count;
        const slot=tracks.findIndex(t=>t.index===active[field]);
        const count=key.startsWith('Page')?Math.max(1,tracks.length-1):1;
        let next=slot>=0&&tracks[slot+step*count]?tracks[slot+step*count].index:active[field]+step*count;
        const visible=visibleMerge(active),selected=projection.selection.merge;
        const merged=visible||(selected&&active.row>=selected.first_row&&active.row<=selected.last_row&&active.column>=selected.first_column&&active.column<=selected.last_column?selected:null);
        if(merged&&!key.startsWith('Page')) {
            const edge=vertical?(step>0?merged.last_row:merged.first_row):(step>0?merged.last_column:merged.first_column);
            // A manually scrolled viewport can be arbitrarily far from the
            // selected cell. Do not jump to its first track when leaving it.
            next=visible?(step>0?(tracks.find(t=>t.index>edge)?.index??edge+1):(tracks.findLast(t=>t.index<edge)?.index??edge-1)):edge+step;
        }
        if(key==='Home')next=0;
        if(key==='End')next=maximum-1;
        return {...active,[field]:Math.max(0,Math.min(maximum-1,next))};
    }
    function pointReference(first,last=first,extend=false) {
        if(draft===null || !entryMode || !draft.startsWith('='))return false;
        if(!pointing) {
            const tail=referenceInsertion(draft,(inlineEditor||formula).selection,{rows:projection.frame.row_count,columns:projection.frame.column_count});
            pointing={start:tail?.start??draft.length,end:tail?.end??draft.length,
                first:(tail?.first&&(extend||tail.colon))?tail.first:first,last,colon:tail?.colon===true};
        }
        if(extend||pointing.colon)first=pointing.first;
        const reference=rangeAddress(betweenCells(first,last));
        const value=draft.slice(0,pointing.start)+reference+draft.slice(pointing.end);
        pointing={...pointing,end:pointing.start+reference.length,first,last,caret:characterOffset(value,pointing.start+reference.length)};
        if(inlineEditor && value!==draft && !inlineEditor.selection) {
            // Keep the overlay and its anchor in place. Replacing only this
            // tiny text field resets its native caret to the inserted
            // reference end without resizing or repainting the whole sheet.
            const previous=inlineEditor;syncDraft(value,previous);
            inlineEditor=new TextField({name:'cell-editor',...inlineDocumentStyle(),value,focused:true,state:State.KEY_NAVIGATION,accessibleName:'Cell input'});
            bindEditor(inlineEditor);inlinePosition.remove(previous,{destroy:true});inlinePosition.append(inlineEditor);
        } else syncDraft(value);
        (inlineEditor||formula).setSelection?.(pointing.caret);
        (inlineEditor||formula).setProperty('focused',true);grid?.setProperty('focused',false);
        status.text=`Reference ${reference} · Enter commits; F2 edits text`;
        revealReference(last);return true;
    }
    function queueInput(action) {
        if(queuedInput.length>=64){status.text='Input queue is full. Wait for the current edit to finish.';return;}
        queuedInput.push(action);
    }
    async function drainInput() {
        if(drainingInput||inputTransaction||isBusy()||closed)return;
        drainingInput=true;
        try {
            while(queuedInput.length&&!closed&&!isBusy()&&!inputTransaction)await queuedInput.shift()();
        } finally {drainingInput=false;}
    }
    async function commitDraft(key='Enter',modifiers=0,explicitTarget=null) {
        if(draft===null || !draftTarget || isBusy() || inputTransaction)return;
        const input=completeFormula(draft),target=draftTarget,next=explicitTarget||neighboringCell(target,key,modifiers);
        inputTransaction=true;draft=null;draftTarget=null;pointing=null;syncFormulaReferences();clearInlineEditor();focusGrid();
        let changed=false,validationCancelled=false;
        try {
            changed=await dispatchCommand({type:'edit-cell',...target,input},{onValidationCancel:()=>{validationCancelled=true;}});
            if(!changed){queuedInput.length=0;if(closed)return;if(validationCancelled){cancelDraft();focusGrid();return;}draftTarget=target;draft=input;showDraftFormulaBar(true);formula.setProperty('value',input);formula.setSelection?.(Array.from(input).length);formula.setProperty('focused',true);syncFormulaReferences();return;}
            if(!closed&&next&&(next.row!==target.row||next.column!==target.column))await dispatchCommand({type:'select-cell',...next});
            if(!closed)focusGrid();
        } finally {inputTransaction=false;if(changed&&!closed)void drainInput();}
    }
    function bindEditor(control) {
        control.setSelection?.(Array.from(control.getProperty('value')).length);
        control.on(Message.TEXT_SELECTION,()=>{
            const selection=control.selection;
            if(pointing&&selection&&(selection.anchor!==pointing.caret||selection.caret!==pointing.caret))pointing=null;
        });
        control.on(Message.VALUE_CHANGED,event=>{
            if(closed||isBusy()||!supports('edit-cell')||!activeCell()||projection.selection.readOnly){cancelDraft();return;}
            if(draft===null){draftTarget=activeCell();entryMode=true;control.setProperty('state',control.getProperty('state')|State.KEY_NAVIGATION);}
            pointing=null;syncDraft(String(event.detail?.value??control.getProperty('value')),control);
        });
        control.on(Message.KEY,handleEditorKey);
    }
    function handleEditorKey(event) {
            if(!event.detail?.pressed||isBusy()||closed)return;
            const key=keyName(event.detail),modifiers=event.detail.modifiers||0;
            if(event.detail.key==='F2'||event.detail.keyCode===68){entryMode=!entryMode;pointing=null;for(const editor of [formula,inlineEditor].filter(Boolean))editor.setProperty('state',entryMode?editor.getProperty('state')|State.KEY_NAVIGATION:editor.getProperty('state')&~State.KEY_NAVIGATION);status.text=entryMode?'Point mode: arrows select formula references; F2 edits text':'Text mode: arrows move the caret; F2 selects formula references';event.preventDefault();return;}
            if(key==='Escape'){cancelDraft();focusGrid();event.preventDefault();return;}
            if(draft===null||!draftTarget)return;
            if(['Enter','Tab'].includes(key)){event.preventDefault();void commitDraft(key,modifiers);return;}
            if(entryMode&&['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(key)) {
                event.preventDefault();
                if(draft.startsWith('=')) {
                    const tail=referenceInsertion(draft,(inlineEditor||formula).selection,{rows:projection.frame.row_count,columns:projection.frame.column_count});
                    const extend=Boolean(modifiers&1)||pointing?.colon||tail?.colon;
                    const base=pointing?.last||tail?.last||draftTarget;
                    const next=neighboringCell(base,key,modifiers);pointReference(next,next,extend);
                }
                else void commitDraft(key,modifiers);
            }
    }
    bindEditor(formula);
    function revealReference(cell) {
        const frame=projection?.frame;if(!frame)return;
        const changes={};
        for(const [axis,coordinate,start]of[['rows','row','top'],['columns','column','left']]) {
            const tracks=frame[axis];
            if(!tracks.some(track=>track.index===cell[coordinate]))changes[start]=Math.max(0,cell[coordinate]-(cell[coordinate]>(tracks.at(-1)?.index??0)?Math.max(1,tracks.length)-1:0));
        }
        if(Object.keys(changes).length)queueViewport({...changes,followSelection:false});
    }
    function relocateDraftEditor(selection) {
        if(draft===null)return;
        const position=editorPosition(),old=inlineEditor;
        if(position) {
            showDraftFormulaBar(false);
            if(inlinePosition)inlinePosition.setProperty('text',position);
            else {
                inlineEditor=new TextField({name:'cell-editor',...inlineDocumentStyle(),value:draft,focused:true,state:entryMode?State.KEY_NAVIGATION:0,accessibleName:'Cell input'});
                bindEditor(inlineEditor);inlinePosition=new Stack({text:position},[inlineEditor]);
                inlineLayer=new Stack({name:'cell-editor-layer',text:'panel:overlay:flush'},[inlinePosition]);gridStack.append(inlineLayer);
            }
            for(const [key,value] of Object.entries(inlineDocumentStyle()))inlineEditor.setProperty(key,value);
            formula.setProperty('focused',false);inlineEditor.setProperty('focused',true);
        } else { clearInlineEditor();showDraftFormulaBar(true);formula.setProperty('focused',true); }
        const current=inlineEditor||formula;
        current.setProperty('value',draft);
        current.setProperty('state',entryMode?current.getProperty('state')|State.KEY_NAVIGATION:current.getProperty('state')&~State.KEY_NAVIGATION);
        if(current!==old&&selection)current.setSelection?.(selection.anchor,selection.caret);
        grid?.setProperty('focused',false);syncFormulaReferences();
    }
    function queueViewport(changes) {
        viewport={...viewport,...changes};pendingViewport=true;void flushViewport();
    }
    async function flushViewport() {
        if(!pendingViewport||closed||isBusy()||!engine||viewportRead)return;
        pendingViewport=false;
        const requested={...viewport};
        viewportRead=readProjection({preserveDraft:true,region:requested});
        try{await viewportRead;}catch(error){if(!closed){ if(!pendingViewport && requested.zoomPercent !== (projection?.frame?.zoom_percent || 100)) { viewport={...viewport,zoomPercent:projection?.frame?.zoom_percent||100}; updateZoom(); } status.text=`Cannot change view: ${String(error.message||error).slice(0,180)}`; }}
        finally{viewportRead=null;if(pendingViewport)void flushViewport();}
    }
    function scrollBy(axis,steps) {
        const key=axis==='rows'?'top':'left',maximum=axis==='rows'?1048576:16384,count=axis==='rows'?viewport.rows:viewport.columns;
        const current=viewport[key];queueViewport({[key]:Math.max(0,Math.min(maximum-(viewport.pixelWidth?1:count),current+steps)),followSelection:false});
    }
    function bindGridEvents(control) {
        if(Message.SIZE_CHANGED)control.on(Message.SIZE_CHANGED,event=>{
            const {width,height}=event.detail||{};
            if(!Number.isInteger(width)||!Number.isInteger(height)||width<49||height<25)return;
            const pixelWidth=Math.min(65536,width),pixelHeight=Math.min(65536,height);
            if(viewport.pixelWidth===pixelWidth&&viewport.pixelHeight===pixelHeight)return;
            queueViewport({pixelWidth,pixelHeight,...(BigInt(app.hostCapabilities||0)&(1n<<14n)?{frameBytes:262144}:{}),rows:256,columns:128,top:viewport.top,left:viewport.left,followSelection:draft===null});
            if(inlinePosition){const position=editorPosition();if(position)inlinePosition.setProperty('text',position);}
        });
        control.on(Message.BLUR, () => { if (pointerDrag?.mode === 'resize') status.text = projection?.status || 'Resize cancelled'; if(pointerDrag?.mode!=='reference'){if(pointerDrag?.mode==='fill')syncFormulaReferences();if(pointerDrag?.mode==='header')control.presentFrame(projection.frame,{fullPaint:false,changedCells:[]});pointerDrag = null;} });
        control.on(Message.POINTER, event => {
            if(event.detail?.kind==='down'&&event.detail.button===1)charts?.select(null);
            if(event.detail?.kind==='wheel'&&!closed&&supports('select-cell')) {
                const horizontal=(event.detail.modifiers&1)||[6,7].includes(event.detail.button);
                const direction=[4,6].includes(event.detail.button)?-1:1;
                scrollBy(horizontal?'columns':'rows',direction*3);event.preventDefault();return;
            }
            if(deferredSelection && ['move','up','cancel'].includes(event.detail?.kind)) {
                const pending=deferredSelection,target=control.hitTest(event.detail.x,event.detail.y);
                if(event.detail.kind==='cancel'){pending.cancelled=true;deferredSelection=null;return;}
                if(pending.mode==='header') {
                    const last=pending.axis==='rows'?target?.row:target?.column;
                    if(Number.isInteger(last))pending.last=last;
                } else if(target&&Number.isInteger(target.row)&&Number.isInteger(target.column)&&!target.kind)pending.last={row:target.row,column:target.column};
                if(event.detail.kind==='up')pending.ended=true;
                event.preventDefault();return;
            }
            if((inputTransaction||commandInFlight)&&!closed&&!closePending&&!functionOpen&&event.detail?.kind==='down'&&event.detail.button===1) {
                const target=control.hitTest(event.detail.x,event.detail.y);
                const axis=target&&supports('select-range')?headerAxis(target):null;
                if(axis) {
                    if(queuedInput.length>=64){status.text='Input queue is full. Wait for the current edit to finish.';event.preventDefault();return;}
                    const sheetId=projection.activeSheetId,last=axis==='rows'?target.row:target.column;
                    const prior=deferredSelection?.mode==='header'&&!deferredSelection.cancelled&&deferredSelection.sheetId===sheetId&&deferredSelection.axis===axis?deferredSelection:headerAnchor;
                    const anchor=prior?.sheetId===sheetId&&prior.axis===axis?prior.first:(axis==='rows'?activeCell()?.row:activeCell()?.column);
                    const pending={mode:'header',axis,sheetId,first:event.detail.modifiers&1?(anchor??last):last,last,ended:false,cancelled:false};
                    deferredSelection=pending;
                    queueInput(async()=>{
                        if(pending.cancelled||closed||projection?.activeSheetId!==sheetId){if(deferredSelection===pending)deferredSelection=null;return;}
                        cancelDraft();rangeAnchor=rangeExtent=null;focusGrid();
                        if(!pending.ended) {
                            pointerDrag={mode:'header',axis,first:pending.first,last:pending.last,sheetId,revision:projection.revision};
                            control.presentFrame(headerPreview(projection.frame,headerRange(projection.frame,axis,pending.first,pending.last)),{fullPaint:false,changedCells:[]});
                        } else {
                            headerAnchor={axis,first:pending.first,last:pending.last,sheetId};
                            await dispatchCommand({type:'select-range',sheetId,...headerRange(projection.frame,axis,pending.first,pending.last)});
                        }
                        if(deferredSelection===pending)deferredSelection=null;
                    });
                    event.preventDefault();return;
                }
                if(target&&Number.isInteger(target.row)&&Number.isInteger(target.column)&&!target.kind){
                    const pending={sheetId:projection.activeSheetId,first:{row:target.row,column:target.column},last:{row:target.row,column:target.column},ended:false,cancelled:false};
                    deferredSelection=pending;
                    queueInput(async()=>{
                        if(pending.cancelled||projection?.activeSheetId!==pending.sheetId){if(deferredSelection===pending)deferredSelection=null;return;}
                        cancelDraft();rangeAnchor=rangeExtent=null;
                        const selection=()=>supports('select-range')&&(pending.first.row!==pending.last.row||pending.first.column!==pending.last.column)
                            ? {type:'select-range',sheetId:pending.sheetId,...betweenCells(pending.first,pending.last)}
                            : {type:'select-cell',sheetId:pending.sheetId,...pending.last};
                        const first=selection();await dispatchCommand(first);
                        if(!pending.cancelled&&projection?.activeSheetId===pending.sheetId){
                            const final=selection();if(pending.ended&&JSON.stringify(first)!==JSON.stringify(final))await dispatchCommand(final);
                            else if(!pending.ended)pointerDrag={mode:'select',first:pending.first,last:pending.last,revision:projection.revision};
                            focusGrid();
                        }
                        if(deferredSelection===pending)deferredSelection=null;
                    });
                    event.preventDefault();
                }
                return;
            }
            if (closed || isBusy() || !supports("select-cell")) { if(pointerDrag?.mode==='fill')syncFormulaReferences();pointerDrag = null; return; }
            const hit = control.hitTest(event.detail.x, event.detail.y);
            const kind = event.detail?.kind;
            if(kind==='down'&&event.detail.button===3&&hit){void openContextMenu(hit,event.detail.x,event.detail.y);event.preventDefault();return;}
            if (kind === 'cancel') { if(pointerDrag?.mode==='header')control.presentFrame(projection.frame,{fullPaint:false,changedCells:[]}); if(pointerDrag?.mode==='fill')syncFormulaReferences();pointerDrag = null; status.text = projection?.status || 'Resize cancelled'; return; }
            if(pointerDrag?.mode==='reference'&&['move','up'].includes(kind)) {
                if(hit&&hit.row!==null&&hit.column!==null&&hit.row!==undefined)pointReference(pointerDrag.first,hit);
                if(kind==='up')pointerDrag=null;event.preventDefault();return;
            }
            if(kind==='down'&&event.detail.button===1&&draft!==null&&hit&&hit.row!==null&&hit.column!==null&&hit.row!==undefined) {
                if(pointReference(hit,hit,Boolean(event.detail.modifiers&1))){pointerDrag={mode:'reference',first:pointing.first};event.preventDefault();return;}
                const target={sheetId:projection.activeSheetId,row:hit.row,column:hit.column};void commitDraft('Enter',0,target);event.preventDefault();return;
            }
            if (pointerDrag?.mode === 'resize' && ['move', 'up'].includes(kind)) {
                const drag = pointerDrag, coordinate = drag.hit.axis === 'rows' ? event.detail.y : event.detail.x;
                if (projection.revision !== drag.revision || projection.activeSheetId !== drag.sheetId) { pointerDrag = null; status.text = 'Workbook changed. Start the resize again.'; return; }
                const value = resizeDragValue(drag.hit, coordinate);
                if (value === null) { pointerDrag = null; return; }
                if (kind === 'move') { status.text = `${drag.hit.axis === 'rows' ? 'Row' : 'Column'} ${drag.hit.index + 1}: ${value} ${drag.hit.axis === 'rows' ? 'pt' : 'px'} — release to apply, Escape to cancel`; return; }
                pointerDrag = null;
                if (coordinate === drag.hit.origin) { status.text = projection.status; return; }
                void dispatchCommand(resizeCommand(drag.sheetId, drag.hit.axis, drag.hit.index, drag.hit.index, value), { expectedRevision: drag.revision });
                return;
            }
            if (pointerDrag?.mode === 'header' && ['move', 'up'].includes(kind)) {
                const drag = pointerDrag;
                if (drag.revision !== projection.revision || drag.sheetId !== projection.activeSheetId) {
                    pointerDrag = null; control.presentFrame(projection.frame); return;
                }
                const coordinate = drag.axis === 'rows' ? hit?.row : hit?.column;
                if (Number.isInteger(coordinate) && coordinate !== drag.last) {
                    drag.last = coordinate;
                    control.presentFrame(headerPreview(projection.frame, headerRange(projection.frame, drag.axis, drag.first, drag.last)), {fullPaint:false,changedCells:[]});
                }
                event.preventDefault();
                if (kind === 'move') return;
                pointerDrag = null;
                headerAnchor = {axis:drag.axis, first:drag.first, last:drag.last, sheetId:drag.sheetId};
                void dispatchCommand({type:'select-range',sheetId:drag.sheetId,...headerRange(projection.frame,drag.axis,drag.first,drag.last)});
                return;
            }
            if (pointerDrag && ['move', 'up'].includes(kind)) {
                if (hit && hit.row !== null && hit.column !== null && hit.row !== undefined && hit.revision === pointerDrag.revision) pointerDrag.last = hit;
                if (kind === 'move') { if(pointerDrag.mode==='fill')showFillTarget(pointerDrag);status.text = `${pointerDrag.mode === 'fill' ? 'Fill' : 'Select'} ${rangeAddress(betweenCells(pointerDrag.first, pointerDrag.last))}`; return; }
                const drag = pointerDrag; pointerDrag = null;if(drag.mode==='fill')syncFormulaReferences();
                if (projection.revision !== drag.revision) return;
                if (drag.mode === 'paint' && formatPainter) {
                    try { const command=formatPainterCommand(formatPainter,projection.activeSheetId,betweenCells(drag.first,drag.last));cancelFormatPainter();void dispatchCommand(command); }
                    catch(error) { cancelFormatPainter();status.text=error.message; }
                    return;
                }
                if (drag.mode === 'fill') {
                    const target=fillTarget(drag);
                    if (JSON.stringify(target) !== JSON.stringify(drag.source)) void dispatchCommand({ type: 'fill-range', sheetId: projection.activeSheetId, source: drag.source, target });
                } else {
                    rangeAnchor = drag.first; rangeExtent = drag.last;
                    void dispatchCommand({ type: 'select-range', sheetId: projection.activeSheetId, ...betweenCells(drag.first, drag.last) });
                }
                return;
            }
            if(kind==='down'&&event.detail.button===6&&hit?.kind==='fill-handle'&&supports('fill-down-adjacent')) {
                pointerDrag=null;syncFormulaReferences();void dispatchCommand({type:'fill-down-adjacent',sheetId:projection.activeSheetId,range:selectionRange(projection)});event.preventDefault();return;
            }
            if (kind !== 'down' || event.detail.button !== 1) return;
            const edge = headerResizeHit(projection?.frame, event.detail.x, event.detail.y);
            if (edge && supports(`resize-${edge.axis}`) && edge.revision === projection.revision) {
                cancelDraft(); formula.setProperty('focused', false); control.setProperty('focused', true);
                pointerDrag = { mode: 'resize', hit: edge, sheetId: projection.activeSheetId, revision: projection.revision };
                event.preventDefault(); return;
            }
            if (hit?.kind === 'filter') {
                if (hit.revision === projection?.revision) void showData('filter', hit.column);
                return;
            }
            if(hit&&hit.revision===projection?.revision&&supports('select-range')&&(hit.row===null||hit.column===null)) {
                const axis = headerAxis(hit);
                if (!axis) return;
                const last = axis === 'rows' ? hit.row : hit.column;
                const previous = headerAnchor?.sheetId === projection.activeSheetId && headerAnchor.axis === axis ? headerAnchor.first : (axis === 'rows' ? activeCell()?.row : activeCell()?.column);
                const first = event.detail.modifiers & 1 ? (previous ?? last) : last;
                cancelDraft();focusGrid();rangeAnchor=rangeExtent=null;
                pointerDrag={mode:'header',axis,first,last,revision:projection.revision,sheetId:projection.activeSheetId};
                control.presentFrame(headerPreview(projection.frame,headerRange(projection.frame,axis,first,last)),{fullPaint:false,changedCells:[]});
                event.preventDefault();return;
            }
            if (!hit || hit.row === null || hit.column === null || hit.revision !== projection?.revision) return;
            headerAnchor = null;
            cancelDraft();
            formula.setProperty("focused", false);
            control.setProperty("focused", true);
            if (formatPainter && !hit.kind) {
                pointerDrag={mode:'paint',first:hit,last:hit,revision:projection.revision};event.preventDefault();return;
            }
            if (hit.kind === 'fill-handle' && supports('fill-range')) {
                pointerDrag = { mode: 'fill', source: selectionRange(projection), first: hit, last: hit, revision: projection.revision };showFillTarget(pointerDrag); return;
            }
            if (supports('select-range')) {
                const first = event.detail.modifiers & 1 ? (rangeAnchor || activeCell()) : hit;
                pointerDrag = { mode: 'select', first, last: hit, revision: projection.revision }; return;
            }
            rangeAnchor = rangeExtent = null;
            void dispatchCommand({ type: "select-cell", sheetId: projection.activeSheetId, row: hit.row, column: hit.column });
        });
        const handleGridKey = event => {
            if(event.detail?.pressed&&(inputTransaction||commandInFlight)&&!closed&&!closePending&&!functionOpen) {
                const key=keyName(event.detail);
                if(key==='Escape'&&deferredSelection){deferredSelection.cancelled=true;deferredSelection=null;event.preventDefault();return;}
                if(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Tab','Enter','PageUp','PageDown','Home','End'].includes(key)||(event.detail.keyCode===65&&[1,4,5].includes(event.detail.modifiers))||event.detail.keyCode===68||event.detail.key==='F2'||event.detail.text) {
                    const detail={...event.detail};queueInput(()=>handleGridKey({detail,preventDefault(){}}));event.preventDefault();
                }
                return;
            }
            if (!event.detail?.pressed || closed || isBusy() || !supports("select-cell")) return;
            const modifiers = event.detail.modifiers || 0;
            if(draft===null&&modifiers===8&&(event.detail.key==='ArrowDown'||event.detail.keyCode===116)){
                event.preventDefault();void showValidationList();return;
            }
            if (draft === null && ((modifiers === 1 && (event.detail.key === 'F10' || event.detail.keyCode === 76)) || event.detail.key === 'ContextMenu' || event.detail.keyCode === 135)) {
                const active = activeCell(), frame = projection.frame;
                if (active) {
                    const row = frame.rows.find(track => track.index === active.row), column = frame.columns.find(track => track.index === active.column);
                    void openContextMenu({ ...active, revision: projection.revision }, frame.row_header + (column?.clip_start || 0), frame.column_header + (row?.clip_end || 0));
                }
                event.preventDefault(); return;
            }
            // Input may already be in flight to the grid while the host applies
            // the editor focus patch. Preserve its logical editing destination.
            if(draft!==null && !(modifiers&(4|8|64))) {
                if(event.detail.text&&!/[\u0000-\u001f]/u.test(event.detail.text)){
                    const editor=inlineEditor||formula,characters=Array.from(draft),selection=editor.selection;
                    const start=selection?Math.min(selection.anchor,selection.caret):characters.length,end=selection?Math.max(selection.anchor,selection.caret):characters.length;
                    pointing=null;syncDraft(characters.slice(0,start).join('')+event.detail.text+characters.slice(end).join(''));
                    editor.setSelection?.(start+Array.from(event.detail.text).length);
                    editor.setProperty('focused',true);grid?.setProperty('focused',false);
                    event.preventDefault();return;
                }
                return handleEditorKey(event);
            }
            if (draft === null && supports('select-range') && event.detail.keyCode === 65 && (modifiers === 1 || modifiers === 4 || modifiers === 5)) {
                const axis = modifiers === 5 ? 'all' : modifiers === 4 ? 'columns' : 'rows';
                const selected = selectionRange(projection);
                const first = axis === 'rows' ? selected.firstRow : selected.firstColumn;
                const last = axis === 'rows' ? selected.lastRow : selected.lastColumn;
                headerAnchor={axis,first,last,sheetId:projection.activeSheetId};rangeAnchor=rangeExtent=null;event.preventDefault();
                return dispatchCommand({type:'select-range',sheetId:projection.activeSheetId,...headerRange(projection.frame,axis,first,last)});
            }
            if (modifiers & 4) {
                const code=event.detail.keyCode;
                if(code===10){event.preventDefault();showFormat();return;}
                if(code===110){event.preventDefault();cancelDraft();void dispatchCommand({type:'select-cell',sheetId:projection.activeSheetId,row:0,column:0});return;}
                if(code===115){event.preventDefault();void selectUsedEnd();return;}
                if(code===41){event.preventDefault();void showData('find');return;}
                if(code===46&&(modifiers&1)){event.preventDefault();performEditorAction('toggle-filter');return;}
                const document=({57:'new',32:'open',39:'save'})[code];
                if(document){event.preventDefault();void dispatchDocument(document);return;}
                const operation = ({ 54: 'copy', 55: modifiers & 1 ? 'paste-values' : 'paste', 40: 'fill-down', 27: 'fill-right', 52: 'undo', 29: 'redo',
                    56: 'font-bold', 31: 'font-italic', 30: 'font-underline' })[event.detail.keyCode];
                if (operation) { event.preventDefault(); performEditorAction(operation); }
                return;
            }
            if (modifiers & (8 | 64)) return;
            if (event.detail.key === 'F2' || event.detail.keyCode === 68) { event.preventDefault(); beginEditing(null, false); return; }
            if (event.detail.key === 'Delete' || event.detail.keyCode === 119) { event.preventDefault(); performEditorAction('clear'); return; }
            if (typeof event.detail.text === 'string' && event.detail.text && !/[\u0000-\u001f]/u.test(event.detail.text)) {
                event.preventDefault(); beginEditing(event.detail.text); return;
            }
            const key = keyName(event.detail);
            if (key === 'Escape') { if(deferredSelection)deferredSelection.cancelled=true;deferredSelection=null;cancelFormatPainter();if(pointerDrag?.mode==='header')control.presentFrame(projection.frame,{fullPaint:false,changedCells:[]});pointerDrag = null; cancelDraft(); status.text = projection.status; return; }
            if(!['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Enter','Tab','PageUp','PageDown','Home','End'].includes(key))return;
            const extending = (modifiers & 1) && !['Tab','Enter'].includes(key);
            if (extending && !supports('select-range')) return;
            if (extending && headerAnchor?.sheetId === projection.activeSheetId && headerAnchor.axis !== 'all') {
                const axis=headerAnchor.axis;
                const step=axis==='columns'?({ArrowLeft:-1,ArrowRight:1})[key]:({ArrowUp:-1,ArrowDown:1})[key];
                if (step !== undefined) {
                    headerAnchor.last=Math.max(0,Math.min((axis==='columns'?projection.frame.column_count:projection.frame.row_count)-1,headerAnchor.last+step));
                    event.preventDefault();
                    return dispatchCommand({type:'select-range',sheetId:projection.activeSheetId,...headerRange(projection.frame,axis,headerAnchor.first,headerAnchor.last)});
                }
                event.preventDefault();return;
            }
            headerAnchor=null;
            const active = extending && rangeExtent ? rangeExtent : activeCell();
            if (!active) return;
            const target=neighboringCell(active,key,modifiers);
            if (target.row === active.row && target.column === active.column) return;
            cancelDraft(); event.preventDefault();
            if (extending) {
                rangeAnchor ||= activeCell(); rangeExtent = target;
                return dispatchCommand({ type: 'select-range', sheetId: projection.activeSheetId, ...betweenCells(rangeAnchor, target) });
            }
            rangeAnchor = rangeExtent = null;
            return dispatchCommand({ type: "select-cell", ...target });
        };
        control.on(Message.KEY,handleGridKey);
    }
    const toggle = (name, text, control) => {
        const item = action(name, text, () => {
            const visible = control===formulaBar?!formulaBarRequestedVisible:!control.getProperty("visible");
            if(control===formulaBar){formulaBarRequestedVisible=visible;showDraftFormulaBar(offscreenDraft);}
            else control.setProperty("visible", visible);
            item.setProperty("checked", visible);
        });
        item.setProperty("checked", true);
        return item;
    };
    const documentAction = (name, text, toolbar = false) => {
        const item = toolbar ? new Button({name:`toolbar-${name}`,text,enabled:false}) : action(name,text);
        item.on(Message.ACTIVATE, () => { void dispatchDocument(name); });
        const items = documentItems.get(name) || [];
        items.push(item); documentItems.set(name, items);
        return item;
    };
    const dataAction = (name, text, callback, toolbar = false) => {
        const item = toolbar ? new Button({name:`toolbar-${name}`,text,enabled:false}) : action(name, text, callback);
        if(toolbar)item.on(Message.ACTIVATE,callback);
        item.setProperty('enabled', false); dataItems.set(name, item); return item;
    };
    const editorAction = (name, text, capability, toolbar = false, available = null, control = null) => {
        const palette = name === 'font-color' && toolbar;
        const item = control || (palette ? createFontColorPicker({onChoose:applyFontColor}) : toolbar ? new Button({ name: `toolbar-${name}`, text, enabled: false }) : action(name, text));
        if (!palette) item.on(Message.ACTIVATE, () => performEditorAction(name));
        editorItems.push({ item, capability, available }); return item;
    };
    const fontAction = (name, text, property, toolbar = false) => {
        const item = editorAction(name, text, 'format-range', toolbar, () => Boolean(projection?.selection.font));
        fontItems.push({ item, property });
        return item;
    };
    fontSize.on(Message.KEY, event => {
        if (event.detail?.pressed && (event.detail.key === 'Enter' || event.detail.keyCode === 36)) {
            event.preventDefault(); performEditorAction('font-size');
        }
    });
    const structureAction = (name, text, capability = name, toolbar = false, available = null) => {
        const item = toolbar ? new Button({ name: `toolbar-${name}`, text, enabled: false }) : action(name, text);
        item.on(Message.ACTIVATE, () => performStructureAction(name, capability));
        structureItems.push({ item, capability, available }); return item;
    };
    function performStructureAction(name, capability) {
        if (closed || isBusy() || !projection || dialog || dataSession || !supports(capability)) return;
        if (name === 'delete-sheet' && projection.sheets.length < 2) return;
        cancelDraft(); pointerDrag = null; rangeAnchor = rangeExtent = null;
        if (['rename-sheet', 'move-sheet', 'delete-sheet', 'delete-rows', 'delete-columns'].includes(name)) {
            try {
                functionOpen = true; updateEnabled();
                dialog = createStructureDialog({ projection, action: name, onClose: closeAbout,
                    apply: async command => {
                        const applied = await dispatchCommand(command, { fromDialog: true });
                        if (!applied && !closed) throw Error(status.text);
                        return applied;
                    } });
                root.append(dialog);
            } catch (error) { functionOpen = false; updateEnabled(); status.text = String(error.message || error).slice(0, 240); }
            return;
        }
        try {
            const command = structureCommand(projection, name);
            if (command) void dispatchCommand(command);
        } catch (error) { status.text = String(error.message || error).slice(0, 240); }
    }
    async function showStyles() {
        if(closed||isBusy()||dialog||dataSession||!projection||!supports('apply-cell-style')||typeof engine?.query!=='function')return;
        const initial=projection;cancelDraft();busy=true;updateEnabled();
        try{const response=await engine.query({action:'cell-styles',offset:0,limit:18});if(closed)return;
            functionOpen=true;dialog=createStyleGallery({canDelete:supports('delete-cell-style'),projection:initial,initialPage:response.result,query:async request=>(await engine.query({action:'cell-styles',...request})).result,isCurrent:()=>!closed&&projection===initial,onClose:closeAbout,
                apply:(command,expectedRevision)=>dispatchCommand(command,{fromDialog:true,expectedRevision})});root.append(dialog);
        }catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);functionOpen=false;}
        finally{busy=false;updateEnabled();}
    }
    async function showFormat(initialTab='number') {
        if(closed||isBusy()||dialog||dataSession||!projection||!supports('format-cells'))return;
        const initial=projection;cancelDraft();pointerDrag=null;grid?.setProperty("focused",false);busy=true;updateEnabled();
        try{
            let themeColors=[];
            if(supports('apply-cell-style')&&typeof engine?.query==='function'){
                const catalog=await engine.query({action:'cell-styles',offset:0,limit:1});
                if(closed)return;
                themeColors=Array.isArray(catalog.result?.theme)?catalog.result.theme.slice(0,6):[];
            }
            functionOpen=true;
            dialog=createFormatDialog({projection:initial,initialTab,themeColors,isCurrent:()=>!closed&&projection===initial,onClose:closeAbout,
                apply:(command,expectedRevision)=>dispatchCommand(command,{fromDialog:true,expectedRevision})});root.append(dialog);
        }catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);functionOpen=false;}
        finally{busy=false;updateEnabled();}
    }
    function showLayout(axis) {
        if (closed || isBusy() || !projection?.selection.layout || !supports(`resize-${axis}`) || dialog || dataSession) return;
        const initial = projection; cancelDraft(); pointerDrag = null; functionOpen = true; updateEnabled();
        dialog = createLayoutDialog({ projection: initial, axis, onClose: closeAbout,
            isCurrent: () => !closed && projection === initial,
            apply: (command, expectedRevision) => dispatchCommand(command, { fromDialog: true, expectedRevision }) });
        root.append(dialog);
    }
    function showFunction(initial = 'SUM',category = null) {
        if (closed || isBusy() || dialog || dataSession || !supports('edit-cell') || !activeCell()) return;
        cancelDraft(); functionOpen = true; updateEnabled();
        dialog = createFunctionDialog({ projection, initial, category, onClose: closeAbout,
            apply: command => dispatchCommand(command, { fromDialog: true }) });
        root.append(dialog);
    }
    function showNames(mode='manage') {
        if(closed||isBusy()||dialog||dataSession||!projection||typeof engine.query!=='function'||!supports('create-defined-name'))return;
        const initial=projection,saved=mode==='use'&&draft!==null?{text:draft,selection:(inlineEditor||formula).selection}:null;
        let chosen=null;
        cancelDraft();pointerDrag=null;grid?.setProperty('focused',false);formula.setProperty('focused',false);functionOpen=true;updateEnabled();
        dialog=createNamedRangesDialog({projection:initial,mode,query:request=>engine.query(request),currentRevision:()=>projection?.revision,
            apply:async(command,expectedRevision)=>{const result=await dispatchCommand(command,{fromDialog:true,expectedRevision});if(!result&&!closed)throw Error(status.text);return result;},
            onUse:item=>{chosen=nameFormulaToken(item,initial.activeSheetId);},
            onClose:()=>{closeAbout();if(closed||mode!=='use'||(!chosen&&!saved))return;
                const text=saved?.text??'=',chars=Array.from(text),selection=saved?.selection,anchor=selection?.anchor??chars.length,caret=selection?.caret??anchor,first=Math.min(anchor,caret),last=Math.max(anchor,caret);
                const next=chosen?chars.slice(0,first).join('')+chosen+chars.slice(last).join(''):text;
                beginEditing(next,false);const editor=inlineEditor||formula;if(chosen){const end=first+Array.from(chosen).length;editor.setSelection(end,end);}else if(selection)editor.setSelection(anchor,caret);
            }});root.append(dialog);
    }
    async function showDataTools(kind) {
        if(closed||isBusy()||dialog||dataSession||!supports(kind)||!activeCell()||typeof engine.query!=='function')return;
        const initial=projection;cancelDraft();pointerDrag=null;rangeAnchor=rangeExtent=null;
        try {
            functionOpen=true;busy=true;updateEnabled();let range=selectionRange(initial);
            const single=range.firstRow===range.lastRow&&range.firstColumn===range.lastColumn;
            const wholeRows=range.firstRow===0&&range.lastRow===1048575,wholeColumns=range.firstColumn===0&&range.lastColumn===16383;
            if(wholeRows||wholeColumns||kind==='remove-duplicates'&&single){
                const response=await engine.query({action:'used-range',sheetId:initial.activeSheetId});
                if(closed)return;if(projection!==initial||response.revision!==initial.revision)throw Error('Worksheet changed. Reopen this tool.');
                const used=response.result;if(!used||used.firstRow===null)throw Error('Select worksheet data first.');
                if(single)range={firstRow:used.firstRow,lastRow:used.lastRow,firstColumn:used.firstColumn,lastColumn:used.lastColumn};
                else range={firstRow:Math.max(range.firstRow,used.firstRow),lastRow:Math.min(range.lastRow,used.lastRow),firstColumn:Math.max(range.firstColumn,used.firstColumn),lastColumn:Math.min(range.lastColumn,used.lastColumn)};
                if(range.firstRow>range.lastRow||range.firstColumn>range.lastColumn)throw Error('The selected rows or columns contain no data.');
            }
            if((range.lastRow-range.firstRow+1)*(range.lastColumn-range.firstColumn+1)>10000)throw Error('Select at most 10000 cells for this operation.');
            dialog=createDataToolsDialog({kind,projection:{...initial,selection:{...initial.selection,range}},query:request=>engine.query(request),
                isCurrent:()=>!closed&&projection===initial,onClose:closeAbout,
                apply:async(command,expectedRevision)=>{
                    if(projection!==initial)throw Error('Worksheet changed. Reopen this tool.');
                    const applied=await dispatchCommand(command,{fromDialog:true,expectedRevision});
                    if(!applied&&!closed)throw Error(status.text);return applied;
                }});root.append(dialog);
        }catch(error){functionOpen=false;status.text=String(error.message||error).slice(0,240);}
        finally{busy=false;if(!dialog)functionOpen=false;updateEnabled();}
    }
    async function showSort(direction = 'ascending', targetColumn = null, useUsedRange = false) {
        if (closed || isBusy() || dialog || dataSession || !supports('sort-range') || !activeCell()) return;
        const initial = projection, active = activeCell();
        let range = selectionRange(initial);
        cancelDraft(); pointerDrag = null; rangeAnchor = rangeExtent = null;
        functionOpen = true; busy = true; updateEnabled();
        try {
            const single = range.firstRow === range.lastRow && range.firstColumn === range.lastColumn;
            if ((single || useUsedRange) && typeof engine.query === 'function') {
                const response = await engine.query({ action: 'used-range', sheetId: initial.activeSheetId });
                if (closed) return;
                if (response.revision !== initial.revision || projection !== initial) {
                    await readProjection();
                    throw Error('Workbook changed. Reopen Sort to use the refreshed range.');
                }
                const used = response.result;
                if (!used || used.sheetId !== initial.activeSheetId || used.firstRow === null) throw Error('Add worksheet data or select a range before sorting.');
                range = { firstRow: used.firstRow, lastRow: used.lastRow, firstColumn: used.firstColumn, lastColumn: used.lastColumn };
            }
            const column = targetColumn ?? (active.column >= range.firstColumn && active.column <= range.lastColumn ? active.column : range.firstColumn);
            dialog = createSortDialog({ sheetId: initial.activeSheetId, range, column, direction, header: useUsedRange,
                isCurrent: () => !closed && projection === initial && projection.revision === initial.revision,
                onClose: closeAbout, apply: async command => {
                    if (projection !== initial) throw Error('Workbook changed. Close and reopen Sort.');
                    const applied = await dispatchCommand(command, { fromDialog: true });
                    if (!applied && !closed) throw Error(status.text);
                    return applied;
                } });
            root.append(dialog);
        } catch (error) { if (!closed) status.text = `Sort unavailable: ${String(error.message || error).slice(0, 240)}`; functionOpen = false; }
        finally { busy = false; if (!dialog) functionOpen = false; updateEnabled(); }
    }
    async function insertChart(kind) {
        if(closed||isBusy()||!projection||!supports('create-chart'))return;
        const initial=projection;let draftChart;
        busy=true;updateEnabled();cancelDraft();
        try{
            const response=await engine.query({action:'chart-draft',sheetId:initial.activeSheetId,range:selectionRange(initial),kind,seriesByRows:false,headers:true,...(viewport.pixelWidth?{viewport:{...viewport,followSelection:false}}:{})});
            if(closed||projection!==initial||response.revision!==initial.revision)return;
            draftChart=response.result.chart;
        }catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{busy=false;updateEnabled();}
        if(draftChart&&await dispatchCommand({type:'create-chart',chart:draftChart},{expectedRevision:initial.revision})){
            const latest=Math.max(...charts.ids);if(Number.isFinite(latest))charts.select(latest);
        }
    }
    async function showChartTypes(id=null) {
        if(closed||isBusy()||dialog||dataSession||!projection)return;
        const initial=projection;functionOpen=true;busy=true;updateEnabled();cancelDraft();
        try {
            const catalog=await engine.query({action:'chart-types'});
            const existing=id?(await charts.get(id)).result.chart:null;
            if(closed||projection!==initial||catalog.revision!==initial.revision)throw Error('Workbook changed. Reopen Chart Type.');
            dialog=createChartTypeDialog({catalog:catalog.result,currentKind:existing?.kind,onClose:closeAbout,onSelect:async kind=>{
                if(closed||projection!==initial)throw Error('Workbook changed. Reopen Chart Type.');
                let command;
                if(existing) {
                    const converted=await engine.query({action:'chart-change-type',id:existing.id,kind});
                    if(converted.revision!==initial.revision)throw Error('Chart changed. Reopen Chart Type.');
                    command={type:'update-chart',chart:converted.result.chart};
                }
                else {
                    const source=await engine.query({action:'chart-draft',sheetId:initial.activeSheetId,range:selectionRange(initial),kind,seriesByRows:false,headers:true,...(viewport.pixelWidth?{viewport:{...viewport,followSelection:false}}:{})});
                    if(source.revision!==initial.revision)throw Error('Workbook changed. Reopen Chart Type.');
                    command={type:'create-chart',chart:source.result.chart};
                }
                const applied=await dispatchCommand(command,{fromDialog:true,expectedRevision:initial.revision});
                if(!applied)throw Error(status.text||'Chart update failed');
                if(!existing){const latest=Math.max(...charts.ids);if(Number.isFinite(latest))charts.select(latest);}
                return true;
            }});
            root.append(dialog);
        } catch(error) {if(!closed)status.text=String(error.message||error).slice(0,240);functionOpen=false;}
        finally {busy=false;updateEnabled();}
    }
    async function showChart(id=charts?.selected,action='edit-chart',part=null) {
        if(action==='chart-type'){void showChartTypes(id);return;}
        if(!id||closed||isBusy()||dialog||dataSession)return;
        const initial=projection;functionOpen=true;busy=true;updateEnabled();cancelDraft();
        try{
            if(action==='legend') {
                const plot=charts.projection(id);
                if(!plot)throw Error('Chart is no longer visible.');
                dialog=createChartLegendDialog({projection:plot,onSelect:selection=>charts.select(id,selection),onClose:closeAbout});
                root.append(dialog);return;
            }
            const response=await charts.get(id);
            if(closed||projection!==initial||response.revision!==initial.revision)throw Error('Chart changed. Repeat the operation.');
            const chart=response.result.chart;
            const apply=async command=>{
                const applied=await dispatchCommand(command,{fromDialog:true,expectedRevision:initial.revision});
                if(!applied)throw Error(status.text||'Chart update failed');return true;
            };
            const common={chart,isCurrent:()=>!closed&&projection===initial,onClose:closeAbout};
            if(action==='select-data')dialog=createChartDataDialog({...common,sheets:initial.sheets,seriesNames:Object.fromEntries((charts.projection(id)?.series||[]).map(series=>[series.id,series.name])),onApply:async request=>{
                if(request.type==='update-chart')return apply(request);
                const source=await engine.query(request);
                if(source.revision!==initial.revision)throw Error('Chart changed. Reopen Select Data.');
                const next=source.result.chart;
                return apply({type:'update-chart',chart:{...chart,categories:next.categories,series:next.series.map((series,i)=>({...series,color:chart.series[i]?.color??series.color,marker:chart.series[i]?.marker??series.marker}))}});
            }});
            else dialog=createChartOptionsDialog({...common,onApply:apply,seriesColors:charts.seriesColors(id),legendItems:charts.projection(id)?.geometry?.legendItems||[],initialPart:part,initialTab:part?.sourceIndex!==undefined&&['pie','doughnut'].includes(chart.kind)?'slices':part?.seriesId?'series':'chart'});
            root.append(dialog);
        }catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);functionOpen=false;}
        finally{busy=false;updateEnabled();}
    }
    async function applyFontColor(value) {
        if (!closed && !closePending && !functionOpen && commandInFlight) {
            queueInput(() => applyFontColor(value)); return;
        }
        if (closed || isBusy() || !projection || !supports('format-cells')) return;
        const range = selectionRange(projection), initial = projection;
        if (!range || !activeCell()) return;
        cancelFormatPainter(); cancelDraft();
        await dispatchCommand({type:'format-cells',sheetId:initial.activeSheetId,range,
            changes:[{path:'font.color',value:value ?? 'none'}]}, {expectedRevision:initial.revision});
        if (!closed) focusGrid();
    }
    function performEditorAction(name) {
        if(!closed&&!closePending&&!functionOpen&&commandInFlight) {
            queueInput(()=>performEditorAction(name));return;
        }
        if (closed || isBusy() || !projection) return;
        if (name === 'format-painter') {
            if (formatPainter) { cancelFormatPainter(); return; }
            const range = selectionRange(projection);
            if (!range || !supports('paint-format')) return;
            try { formatPainterCommand({sheetId:projection.activeSheetId,range},projection.activeSheetId,range); }
            catch(error) { status.text=error.message;return; }
            cancelDraft();formatPainter={sheetId:projection.activeSheetId,range:{...range}};updateEnabled();focusGrid();
            status.text='Format Painter: select a destination. Escape cancels.';return;
        }
        cancelFormatPainter();
        if(name==='insert-chart'){void showChartTypes();return;}
        if(name==='chart-type'){void showChartTypes(charts?.selected);return;}
        if(name==='insert-chart-column'||name==='insert-chart-line'){void insertChart(name.endsWith('line')?'line':'clustered-column');return;}
        if(name==='format-chart'||name==='chart-data'){void showChart(charts?.selected,name==='chart-data'?'select-data':'edit-chart');return;}
        if(name==='delete-chart'){if(charts?.selected)void dispatchCommand({type:'delete-chart',id:charts.selected});return;}
        const range = selectionRange(projection), active = activeCell();
        if (!range || !active) return;
        if(name==='cell-styles'){void showStyles();return;}
        if(['name-manager','define-name','use-name'].includes(name)){showNames(name==='define-name'?'define':name==='use-name'?'use':'manage');return;}
        if (name==='format-cells'||name==='font-color'||name==='fill-color'||name==='borders'){showFormat(({ 'font-color':'font','fill-color':'fill',borders:'border'})[name]||'number');return;}
        if (name === 'row-height' || name === 'column-width') { showLayout(name === 'row-height' ? 'rows' : 'columns'); return; }
        if(['remove-duplicates','text-to-columns'].includes(name)){showDataTools(name);return;}
        if (['sort', 'sort-ascending', 'sort-descending'].includes(name)) { void showSort(name === 'sort-descending' ? 'descending' : 'ascending'); return; }
        if (name === 'toggle-filter') { void dispatchCommand({type:'toggle-autofilter',sheetId:projection.activeSheetId,range});return; }
        if(name==='home-clear-filters'){void clearFilters();return;}
        if(name.startsWith('functions-')){const categories={financial:'Financial',logical:'Logical',text:'Text',date:'Date and time',lookup:'Lookup and reference',math:'Math and trigonometry',statistical:'Statistical',engineering:'Engineering',information:'Information',database:'Database',legacy:'Legacy'};showFunction('SUM',categories[name.slice(10)]);return;}
        if (name === 'functions' || name === 'vlookup') { showFunction(name === 'vlookup' ? 'VLOOKUP' : 'SUM'); return; }
        let command;
        if (name === 'undo' || name === 'redo') command = { type: name };
        else if (name === 'merge-cells' || name === 'unmerge-cells') command = { type: name, sheetId: projection.activeSheetId, range };
        else if (name === 'copy' || name === 'cut' || name === 'clear') command = { type: name === 'copy' ? 'copy-range' : name==='cut'?'cut-range':'clear-range', sheetId: projection.activeSheetId, range };
        else if (['paste', 'paste-values', 'paste-formulas'].includes(name)) command = { type: 'paste-range', ...active,
            mode: name === 'paste' ? 'all' : name === 'paste-values' ? 'values' : 'formulas' };
        else if (name === 'fill-down' || name === 'fill-right') command = fillSelection(projection, name === 'fill-down' ? 'down' : 'right');
        else if (name === 'autosum') { command = autoSumCommand(projection); if (!command) { showFunction('SUM'); return; } }
        else if(name.startsWith('font-family:'))command={type:'format-cells',sheetId:projection.activeSheetId,range,changes:[{path:'font.name',value:name.slice(12)}]};
        else if(name==='wrap-text'||name.startsWith('vertical-'))command={type:'format-cells',sheetId:projection.activeSheetId,range,changes:[{path:name==='wrap-text'?'alignment.wrap_text':'alignment.vertical',value:name==='wrap-text'?String(!projection.selection.format?.wrapText):name.slice(9)}]};
        else if(name==='decimal-increase'||name==='decimal-decrease'){const value=changeDecimals(projection.selection.numberFormat||'General',name==='decimal-increase'?1:-1);if(value===null)return;command={type:'format-range',sheetId:projection.activeSheetId,range,path:'num_fmt',value};}
        else if (name.startsWith('font-')) {
            const font = projection.selection.font;
            if (!font || !supports('format-range')) return;
            const properties = { 'font-bold': ['font.b', font.weight >= 600], 'font-italic': ['font.i', font.italic], 'font-underline': ['font.u', font.underline] };
            const setting = properties[name];
            const resized=name==='font-grow'?adjacentFontSize(font.size_pt,1):name==='font-shrink'?adjacentFontSize(font.size_pt,-1):null;
            if((name==='font-grow'||name==='font-shrink')&&resized===null)return;
            command = { type: 'format-range', sheetId: projection.activeSheetId, range,
                path: setting ? setting[0] : 'font.size', value: setting ? String(!setting[1]) : resized!==null?String(resized):fontSize.getProperty('value').trim() };
        }
        else if (name.startsWith('align-')) command = { type: 'format-range', sheetId: projection.activeSheetId, range,
            path: 'alignment.horizontal', value: name.slice(6) };
        else if (name.startsWith('format-')) command = { type: 'format-range', sheetId: projection.activeSheetId, range,
            path: 'num_fmt', value: ({ 'format-general': 'General', 'format-number': '#,##0.00', 'format-percent': '0.00%',
                'format-currency': '$#,##0.00', 'format-date': 'yyyy-mm-dd' })[name] };
        if (command) return dispatchCommand(command);
        else status.text = 'Select the source and destination cells, then choose Fill Down or Fill Right.';
    }
    const ribbon=createSheetRibbon({sensitivityItem,sensitivityBadge,canUseSheetDataIcons:()=>Boolean(app.hostCapabilities&(1n<<26n)),canSplit:()=>{const r=selectionRange(projection);return Boolean(r&&r.firstColumn===r.lastColumn);},canMerge:()=>projection?.selection.canMerge===true,canUnmerge:()=>projection?.selection.canUnmerge===true,editorAction,fontAction,documentAction,structureAction,dataAction,action,toggle,fontSize,fontFamily,formulaBar,showAbout,
        canDecimal:direction=>changeDecimals(projection?.selection.numberFormat||'General',direction)!==null,canGrowFont:()=>adjacentFontSize(projection?.selection.font?.size_pt,1)!==null,canShrinkFont:()=>adjacentFontSize(projection?.selection.font?.size_pt,-1)!==null,canClearFilters:()=>Boolean(projection?.filterState?.appliedColumns),
        showFind:()=>{void showData('find');},showFilter:()=>{void showData('filter');},clearFilters:()=>{void clearFilters();},
        exit:()=>{if(!closed&&!closePending)window.requestClose();},
        canFill:direction=>Boolean(projection&&fillSelection(projection,direction)),
        canLayout:()=>Boolean(projection?.selection.layout),
        canMoveSheetLeft:()=>projection&&projection.sheets[0].id!==projection.activeSheetId,
        canMoveSheetRight:()=>projection&&projection.sheets.at(-1).id!==projection.activeSheetId,
        canDeleteSheet:()=>projection?.sheets.length>1,canEditChart:()=>Boolean(charts?.selected)});
    root.append(ribbon.control);
    root.append(formulaBar);
    const unavailable = new Column({ name: "viewport-unavailable", text: "panel:distribute" }, [
        new Label({ text: engine ? "Native virtual grid unavailable" : "Spreadsheet engine unavailable", state: State.EMPHASIZED }),
        new Label({ text: engine
            ? "An engine port was supplied for integration testing. Native grid rendering, workbook editing and file operations remain unavailable."
            : "The Wasm engine and native virtual grid are not connected.\nWorkbook editing and XLS/XLSX/ODS opening and saving are unavailable." }),
    ]);
    const gridStack = new Stack({name:'grid-stack',text:'panel:flush'},[unavailable]);
    charts=createSheetCharts({parent:gridStack,query:request=>engine.query(request),services,
        onCommand:(command,revision)=>dispatchCommand(command,{expectedRevision:revision}),
        onSelection:id=>{ribbon.setChartSelected(Boolean(id));updateEnabled();},
        onEdit:(id,action,part)=>{void showChart(id,action,part);},
        onError:error=>{if(!closed)status.text=`Chart: ${String(error.message||error).slice(0,220)}`;},
    });
    const verticalScroll = new ScrollBar({name:'vertical-scroll',orientation:'vertical'});
    const horizontalScroll = new ScrollBar({name:'horizontal-scroll',orientation:'horizontal'});
    const viewportHost = new Column({ name: "viewport-host", text: "panel:distribute" }, [
        new Row({text:'panel:distribute'},[gridStack,new Stack({text:'panel:flush:equal-fixed:accessory-width=16'},[verticalScroll])]),
        new Stack({text:'panel:flush:equal-fixed:accessory-height=16'},[horizontalScroll]),
    ]);
    root.append(viewportHost);
    verticalScroll.on(Message.VALUE_CHANGED,event=>{const value=Number(event.detail?.value);if(Number.isFinite(value)&&value>=0&&value<=1)queueViewport({top:Math.round(value*(1048576-(viewport.pixelWidth?1:viewport.rows))),followSelection:false});});
    horizontalScroll.on(Message.VALUE_CHANGED,event=>{const value=Number(event.detail?.value);if(Number.isFinite(value)&&value>=0&&value<=1)queueViewport({left:Math.round(value*(16384-(viewport.pixelWidth?1:viewport.columns))),followSelection:false});});

    root.append(new Row({ name: 'sheet-tabs-bar', text: 'panel:status-bar:accessory-height=30:flush:gap=4' }, [
        structureAction('create-sheet', '+ New sheet', 'create-sheet', true, () => projection.sheets.length < 256), sheetTabs,
        new Container({ name: 'sheet-notice', text: 'panel:flush', state: State.EXPANSIVE | State.FLAT }, [status]),
        zoom,
    ]));
    const window = createWindow({ title: "Rice Sheet", root });

    function renderProjection(next,{preserveDraft=false}={}) {
        if (projection && next.revision < projection.revision) throw new TypeError("Stale engine projection");
        const previous = projection;
        const keepDraft=preserveDraft&&draft!==null&&previous?.revision===next.revision&&previous?.activeSheetId===next.activeSheetId;
        const editorSelection=keepDraft?(inlineEditor||formula).selection:null;
        projection = next;
        if (headerAnchor) {
            const expected=next.frame&&headerRange(next.frame,headerAnchor.axis,headerAnchor.first,headerAnchor.last),actual=selectionRange(next);
            if(headerAnchor.sheetId!==next.activeSheetId||!expected||!actual||Object.keys(expected).some(key=>expected[key]!==actual[key]))headerAnchor=null;
        }
        contextColumn=next.frame?.active?.[1]??null;
        if(next.scroll) {
            if(!pendingViewport)viewport={...viewport,top:next.scroll.topRow,left:next.scroll.leftColumn};
            verticalScroll.setProperty('value',next.scroll.topRow/Math.max(1,next.scroll.totalRows-1));
            horizontalScroll.setProperty('value',next.scroll.leftColumn/Math.max(1,next.scroll.totalColumns-1));
        }
        if (next.frame) {
            unavailable.setProperty("visible", false);
            if (!grid) {
                grid = new VirtualGrid({ name: "sheet-grid", canvas: 'document', frame: next.frame });
                bindGridEvents(grid);
                gridStack.append(grid);
            } else grid.presentFrame(next.frame, next.damage);
        } else {
            if (grid) gridStack.remove(grid, { destroy: true });
            grid = null;
            unavailable.setProperty("visible", true);
        }
        window.title = `${next.documentTitle || "Workbook"} — Rice Sheet`;
        address.setProperty("value", next.selection.address);
        if(keepDraft)relocateDraftEditor(editorSelection);
        else {draft = null; draftTarget = null; pointing = null;syncFormulaReferences();clearInlineEditor();formula.setProperty("value", next.selection.formula);}
        fontSize.setProperty('value', next.selection.font ? String(next.selection.font.size_pt) : '');
        fontFamily.text=next.selection.font?.family||'Font';for(const item of fontFamily.children)item.setProperty('checked',item.text===next.selection.font?.family);
        ribbon.setNumberFormat(next.selection.numberFormat);
        status.text = !next.frame ? "Grid rendering and editing unavailable" : !supports("edit-cell") ? "Read-only engine viewport" : /^Ready(?:$|[ .·])/i.test(next.status) ? "" : next.status;
        const prompt=next.selection.validation?.prompt;
        if(!status.text&&prompt)status.text=[prompt.title,prompt.message].filter(Boolean).join(': ');
        if (!previous || previous.activeSheetId !== next.activeSheetId || JSON.stringify(previous.sheets) !== JSON.stringify(next.sheets)) {
            for (const child of [...sheetTabs.children]) sheetTabs.remove(child, { destroy: true });
            for (const sheet of next.sheets) {
                const tab = new Button({ text: sheet.name, checked: sheet.id === next.activeSheetId, enabled: false });
                tab.on(Message.ACTIVATE, () => selectSheet(sheet.id));
                sheetTabs.append(tab);
            }
        }
        updateEnabled();
        installContextMenu();
    }

    async function presentResult(raw,options={}) {
        if (closed) return;
        if (projection && raw?.revision < projection.revision) throw new TypeError("Stale engine projection");
        let next;
        try { next = checkedProjection(raw, viewportCache); }
        catch (error) {
            if (error.code !== "RESYNC_REQUIRED") throw error;
            // A read may safely resynchronize after a missed delta. Never replay
            // the command which produced this result, and never retry in a loop.
            const snapshot = await engine.readViewport(viewport, null);
            if (closed) return;
            if (projection && snapshot?.revision < projection.revision) throw new TypeError("Stale engine projection");
            next = checkedProjection(snapshot, viewportCache);
        }
        if (!closed) {
            renderProjection(next,options);
            try{await charts.refresh(next,viewport);}catch(error){if(!closed)status.text=`Chart: ${String(error.message||error).slice(0,220)}`;}
        }
    }

    async function readProjection({preserveDraft=false,region=viewport}={}) {
        const known = viewportCache.current;
        const cursor = known ? { view: known.view, generation: known.generation, sequence: known.sequence, revision: known.frame.revision } : null;
        await presentResult(await engine.readViewport(region, cursor),{preserveDraft});
    }

    function showValidationList(){
        const active=activeCell(),ruleIndex=projection?.selection.validation?.listRule;
        if(closed||isBusy()||draft!==null||dialog||dataSession||!active||!Number.isInteger(ruleIndex)||!supports('select-validation-value')||projection.selection.readOnly||typeof engine?.query!=='function')return;
        const initial=projection,cell={...active,ruleIndex},frame=projection.frame;
        const row=frame.rows.find(track=>track.index===active.row),column=frame.columns.find(track=>track.index===active.column);
        const anchor=row&&column?{x:Math.round(frame.row_header+column.clip_start),y:Math.round(frame.column_header+row.clip_end)}:null;
        functionOpen=true;updateEnabled();
        dialog=createValidationList({cell,revision:initial.revision,anchor,
            query:async(data,options)=>{busy=true;updateEnabled();try{return await engine.query(data,options);}finally{busy=false;if(!closed)updateEnabled();}},
            apply:(command,revision)=>dispatchCommand(command,{fromDialog:true,expectedRevision:revision}),
            isCurrent:()=>!closed&&projection===initial,onClose:closeAbout});
        (anchor?gridStack:root).append(dialog);
    }

    async function dispatchCommand(command, { fromDialog = false, expectedRevision, onValidationCancel } = {}) {
        while(viewportRead){try{await viewportRead;}catch{}}
        if (closed || busy || closePending || (functionOpen && !fromDialog) || !engine || !commandAvailable(command.type)) return false;
        if (expectedRevision !== undefined && projection?.revision !== expectedRevision) { status.text = 'Workbook changed. Repeat the operation on the refreshed view.'; return false; }
        let checked;
        try { checked = checkedSheetCommand(command); }
        catch (error) { status.text = `Input rejected: ${String(error.message || error).slice(0, 200)}`; cancelDraft(); return false; }
        busy = true; commandInFlight = true; updateEnabled();
        const previousFunctionOpen=functionOpen;
        let validating=false;
        try {
            const selected=activeCell();
            const noInputCheck=selected&&selected.sheetId===checked.sheetId&&selected.row===checked.row&&selected.column===checked.column&&projection.selection.validation?.checksInput===false;
            if(checked.type==='edit-cell'&&supports('data-validation')&&!noInputCheck){
                // The candidate, query and eventual mutation share one revision.
                // A decision dialog never grants permission to a later workbook.
                expectedRevision??=projection.revision;
                validating=true;functionOpen=true;updateEnabled();
                if(typeof engine.query!=='function')throw Error('Cell validation could not be checked.');
                const {sheetId,row,column,input}=checked;
                const response=await engine.query({action:'validation-cell',sheetId,row,column,input},{expectedRevision});
                if(closed||closePending)return false;
                const feedback=checkedCellValidation(response,checked,expectedRevision);
                if(projection.revision!==expectedRevision)throw Error('Workbook changed. Try entering the value again.');
                // Never trust an override supplied by a caller: only this
                // decision can enable a warning override, never a Stop rule.
                checked=checkedSheetCommand({type:'edit-cell',sheetId,row,column,input});
                if(feedback.alerts.length){
                    const decision=await new Promise(resolve=>{
                        let settled=false,feedbackDialog;
                        const finish=value=>{
                            if(settled)return;settled=true;dismissValidation=null;
                            if(feedbackDialog.parent)root.remove(feedbackDialog,{destroy:true});
                            resolve(value);
                        };
                        feedbackDialog=createValidationDialog({alerts:feedback.alerts,onDecision:finish});
                        dismissValidation=()=>finish('cancel');root.append(feedbackDialog);
                    });
                    if(decision==='cancel')onValidationCancel?.();
                    if(decision!=='accept'||feedback.alerts.some(alert=>alert.style==='stop'||alert.style==='unsupported'))return false;
                    checked=checkedSheetCommand({...checked,allowValidationWarning:true});
                }
                if(closed||closePending)return false;
                if(projection.revision!==expectedRevision)throw Error('Workbook changed. Try entering the value again.');
            }
            const options={...(expectedRevision===undefined?{}:{expectedRevision}),...(viewport.pixelWidth?{viewport:{...viewport,followSelection:!command.type.endsWith('-chart')}}:{})};
            const result = await engine.command(checked,Object.keys(options).length?options:undefined);
            if (!closed) {
                if (['select-cell','select-sheet'].includes(checked.type)) { headerAnchor=null;rangeAnchor=rangeExtent=null; }
                if (result) await presentResult(result);
                else await readProjection();
            }
            return !closed;
        } catch (error) {
            if (!closed) status.text = `Engine request failed: ${String(error.message || error).slice(0, 240)}`;
            return false;
        } finally { if(validating){dismissValidation?.();functionOpen=previousFunctionOpen;}busy = false; commandInFlight = false; updateEnabled();if(pendingViewport)void flushViewport();if(!inputTransaction)void drainInput(); }
    }
    async function selectUsedEnd() {
        if(closed||isBusy()||!projection||typeof engine?.query!=='function')return;
        const initial=projection;let target=null;busy=true;updateEnabled();cancelDraft();
        try {
            const response=await engine.query({action:'used-range',sheetId:initial.activeSheetId});
            if(closed)return;
            if(projection!==initial||response.revision!==initial.revision)throw Error('Workbook changed. Repeat Ctrl+End.');
            const range=response.result;
            if(range?.sheetId!==initial.activeSheetId)throw Error('Invalid used range');
            target={type:'select-cell',sheetId:initial.activeSheetId,row:range.lastRow??0,column:range.lastColumn??0};
        }catch(error){if(!closed)status.text=String(error.message||error);}
        finally{busy=false;updateEnabled();}
        if(target&&!closed){await dispatchCommand(target,{expectedRevision:initial.revision});focusGrid();}
    }
    async function selectSheet(sheetId) {
        if (projection?.activeSheetId === sheetId) return;
        cancelDraft();
        rangeAnchor = rangeExtent = null; pointerDrag = null;
        await dispatchCommand({ type: "select-sheet", sheetId });
    }

    function closeContextMenu() {
        if(contextMenu){contextMenu.detach();contextMenu.destroy();contextMenu=null;}
    }
    async function openContextMenu(hit,x,y) {
        if(closed||isBusy()||dialog||dataSession||!projection?.frame||hit.revision!==projection.revision)return;
        closeContextMenu();cancelDraft();pointerDrag=null;contextColumn=hit.column;
        const range=selectionRange(projection),row=hit.row,column=hit.column;
        if(row===null||column===null) {
            if(!supports('select-range'))return;
            if(!await dispatchCommand({type:'select-range',sheetId:projection.activeSheetId,firstRow:row??0,lastRow:row??projection.frame.row_count-1,firstColumn:column??0,lastColumn:column??projection.frame.column_count-1}))return;
        }else if(Number.isInteger(row)&&Number.isInteger(column)) {
            const inside=range&&row>=range.firstRow&&row<=range.lastRow&&column>=range.firstColumn&&column<=range.lastColumn;
            if(!inside&&!await dispatchCommand({type:'select-cell',sheetId:projection.activeSheetId,row,column}))return;
        }else return;
        if(closed)return;
        if(!contextMenu)installContextMenu();
    }
    function installContextMenu() {
        if(closed||!grid||!projection?.frame)return;
        const capabilities={cut:'cut-range',copy:'copy-range',paste:'paste-range','paste-values':'paste-range','paste-formulas':'paste-range',clear:'clear-range','toggle-filter':'toggle-autofilter','format-cells':'format-cells',sort:'sort-range','sort-ascending':'sort-range','sort-descending':'sort-range'};
        const can=name=>name==='merge-cells'?supports(name)&&projection?.selection.canMerge===true:name==='unmerge-cells'?supports(name)&&projection?.selection.canUnmerge===true:['filter-values','clear-column-filter'].includes(name)?typeof engine?.query==='function':supports(capabilities[name]||name);
        const filtered=projection.frame.filter_header_row!==undefined||Boolean(projection.frame.filter_columns?.length);
        if(contextMenu){contextMenu.refresh({can,filtered});return;}
        contextMenu=createSheetContextMenu({can,filtered,onAction:name=>{
            const columnTarget=contextColumn??activeCell()?.column;
            if(['insert-rows','insert-columns','delete-rows','delete-columns'].includes(name))performStructureAction(name,name);
            else if(name==='filter-values')void showData('filter',columnTarget);
            else if(name==='clear-column-filter')void clearColumnFilter(columnTarget);
            else performEditorAction(name);
        }}).attachTo(gridStack);
    }
    async function clearColumnFilter(column) {
        if(closed||isBusy()||typeof engine?.query!=='function')return;
        busy=true;updateEnabled();let provider;
        try{provider=await openFilterDataProvider({query:request=>engine.query(request),sheetId:projection.activeSheetId,column});await provider.apply(null);if(!closed)await readProjection();}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{await provider?.close();busy=false;updateEnabled();}
    }
    function closeData() {
        const previous = dataSession;
        if (!previous) return null;
        dataSession = null;
        if (previous.controller) previous.controller.dialog.parent?.remove(previous.controller.dialog, { destroy: true });
        return Promise.resolve(previous.provider?.close()).catch(() => log?.warn('Sheet query cleanup failed')).finally(() => {
            if (!closed && !dataSession) { busy = false; updateEnabled(); }
        });
    }

    async function showData(kind, targetColumn = null) {
        if (closed || isBusy() || dialog || dataSession || typeof engine?.query !== 'function' || !activeCell()) return;
        const active = activeCell(), holder = { provider: null, controller: null };
        dataSession = holder; busy = true; updateEnabled(); cancelDraft();
        try {
            const options = { query: request => engine.query(request), sheetId: active.sheetId };
            holder.provider = kind === 'filter'
                ? await openFilterDataProvider({ ...options, column: targetColumn ?? active.column })
                : await openFindDataProvider(options);
            if (closed || dataSession !== holder) { await holder.provider.close(); return; }
            const onClose = () => { void closeData(); };
            let columnName = '', columnNumber = (targetColumn ?? active.column) + 1;
            while (columnNumber) { columnName = String.fromCharCode(65 + (columnNumber - 1) % 26) + columnName; columnNumber = Math.floor((columnNumber - 1) / 26); }
            const headerColumn=projection.frame?.columns.find(column=>column.index===(targetColumn??active.column));
            const headerRow=projection.frame?.rows.find(row=>row.index===projection.frame.filter_header_row);
            const anchor=kind==='filter'&&headerColumn ? {x:Math.round(projection.frame.row_header+headerColumn.clip_start),y:Math.round(projection.frame.column_header+(headerRow?.clip_end||0))}:null;
            holder.controller = kind === 'filter'
                ? createFilterValuesDialog({ query: request => holder.provider.query(request), selected: holder.provider.selected, excluded: holder.provider.excluded, criterion: holder.provider.criterion,
                    supportsExclusion:holder.provider.supportsExclusion,typedFilters:holder.provider.typedFilters,displayValues: holder.provider.displayValues,anchor,
                    title: `Filter column ${columnName}${holder.provider.headerConvention ? ' · first used row is header' : ''}`,
                    sort: supports('sort-range') ? async direction => {
                        await closeData();
                        if (!closed) await showSort(direction, targetColumn ?? active.column, true);
                    } : null,
                    onClose, apply: async predicate => {
                        await holder.provider.apply(predicate);
                        if (!closed && dataSession === holder) await readProjection();
                    } })
                : createFindDialog({ query: request => holder.provider.query(request), title: holder.provider.filtered ? 'Find in filtered data' : 'Find in current sheet', onClose,
                    select: async target => {
                        const result = await engine.command({ type: 'select-cell', ...target });
                        if (!closed && dataSession === holder) { if (result) await presentResult(result); else await readProjection(); }
                    } });
            (anchor?gridStack:root).append(holder.controller.dialog);
        } catch (error) {
            if (!closed && dataSession === holder) status.text = `Data request failed: ${String(error.message || error).slice(0, 240)}`;
            if (dataSession === holder) await closeData();
        }
    }

    async function clearFilters() {
        if (closed || isBusy() || typeof engine?.query !== 'function' || !projection) return;
        if (!(projection.filterState ? projection.filterState.appliedColumns>0 : projection.frame?.filter_columns?.some(column=>column.active))) return;
        busy = true; updateEnabled(); cancelDraft(); grid?.setProperty('focused',false);
        try {
            await engine.query({ action: 'clear-filter', sheetId: projection.activeSheetId });
            if (!closed) { await readProjection(); focusGrid(); }
        } catch (error) { if (!closed) status.text = `Filter request failed: ${String(error.message || error).slice(0, 240)}`; }
        finally { busy = false; updateEnabled(); }
    }

    async function dispatchDocument(name, parameter) {
        while(viewportRead){try{await viewportRead;}catch{}}
        if (closed || isBusy() || typeof engine?.documents?.[name] !== 'function') return;
        cancelFormatPainter();busy = true; updateEnabled(); cancelDraft();
        try {
            const result = await engine.documents[name](parameter);
            if (closed || !result) return;
            if (result.replaced) {
                headerAnchor=null;rangeAnchor=rangeExtent=null;pointerDrag=null;
                projection = null; viewportCache.clear();charts.reset();
                await readProjection();
            } else if (result.saved) status.text = `Saved ${result.name || 'workbook'} as ${String(engine.documents.format || 'xlsx').toUpperCase()}`;
        } catch (error) {
            if (!closed) status.text = `Workbook operation failed: ${String(error.message || error).slice(0,240)}`;
        } finally { busy = false; updateEnabled(); }
    }

    const stopOpen = app.on(Message.APP_OPEN, event => {
        try {
            const grant = workbookGrantFromOpen(event);
            if (!grant) return;
            if (typeof engine?.documents?.['open-grant'] !== 'function') throw Error('Opening workbooks is unavailable');
            if (starting && !pendingOpen) { pendingOpen = grant; return; }
            if (isBusy() || pendingOpen) throw Error('Finish the current workbook operation before opening another file');
            void dispatchDocument('open-grant', grant);
        } catch (error) { status.text = `${String(error.message || error).slice(0, 240)}. No file was read or changed.`; }
    });
    const copilotActions={command:{command:'Checked Sheet command. Coordinates are zero-based; use real sheet IDs and projection.capabilities.',examples:[{type:'edit-cell',sheetId:'from projection.sheets',row:0,column:0,input:'text or =formula'},{type:'select-sheet',sheetId:'from projection.sheets'},{type:'create-sheet'},{type:'rename-sheet',sheetId:'from projection.sheets',name:'Report'},{type:'undo'},{type:'redo'}]},report:{name:'new report worksheet name',sourceSheetId:'existing worksheet ID',cells:[{row:0,column:0,input:"=SUM('Source'!B2:B20)"}]}};
    const stopCopilot=exposeAppCopilot({bus,instanceId,
        context:async query=>{
            if(query && !isBusy() && engine?.query && projection){
                const response=await engine.query(query,{expectedRevision:projection.revision});
                return {app:'org.ricewm.sheet',ready:!closed&&!isBusy(),revision:projection.revision,projection,result:response.result,actions:copilotActions};
            }
            return {app:'org.ricewm.sheet',ready:!closed&&!isBusy()&&Boolean(projection)&&draft===null,revision:projection?.revision??0,projection,draft,queries:{usedRange:{action:'used-range',sheetId:'from projection.sheets'},note:'Pass a checked Sheet query as data.query to copilot.context; projection.frame contains the visible cells and selection.formula contains the active cell input.'},actions:copilotActions};
        },
        act:async(operation,data,expectedRevision)=>{
            if(operation==='command'){
                const applied=await dispatchCommand(checkedSheetCommand(data.command),{expectedRevision});
                if(!applied)throw Error(status.text||'Sheet action was not applied');
            }else if(operation==='report'){
                if(typeof data.name!=='string'||!Array.isArray(data.cells)||data.cells.length<1||data.cells.length>256)throw Error('A report needs a sheet name and 1–256 cells');
                const source=projection.sheets.find(sheet=>sheet.id===data.sourceSheetId);
                if(!source)throw Error('Choose a source worksheet from the active workbook');
                checkedSheetCommand({type:'rename-sheet',sheetId:source.id,name:data.name});
                const cells=data.cells.map(cell=>checkedSheetCommand({type:'edit-cell',sheetId:source.id,row:cell.row,column:cell.column,input:cell.input}));
                const sourceName="'"+source.name.replaceAll("'","''")+"'!";
                if(!cells.some(cell=>cell.input.startsWith('=')&&(cell.input.includes(sourceName)||cell.input.includes(source.name+'!'))))throw Error('Report formulas must reference the source worksheet');
                const apply=async command=>{if(!await dispatchCommand(command,{expectedRevision}))throw Error(status.text||'Report stopped; inspect the workbook before retrying');expectedRevision=projection.revision;};
                await apply({type:'create-sheet'});
                const target=projection.activeSheetId;
                await apply({type:'rename-sheet',sheetId:target,name:data.name});
                for(const cell of cells)await apply({...cell,sheetId:target});
            }else throw Error('Unknown Sheet AI action');
            return {applied:true,revision:projection.revision,projection};
        }});
    const stopCloseRequest = app.on(Message.APP_CLOSE_REQUEST, () => {
        if (typeof engine?.requestClose !== 'function') return true;
        closePending = true; queuedInput.length=0; updateEnabled();
        return (async () => {
            let approved = false;
            try { await closeData(); approved = await engine.requestClose() === true; return approved; }
            catch { return false; }
            finally { if (!approved && !closed) { closePending = false; updateEnabled(); } }
        })();
    });
    const stopClose = app.on(Message.APP_CLOSE, () => {
        if (closed) return;
        closed = true;queuedInput.length=0;
        dismissValidation?.();
        viewportCache.clear();
        void charts.close().catch(()=>log?.warn("Chart cleanup failed"));
        stopOpen();
        stopClose();
        stopCloseRequest();
        stopCopilot();
        closeAbout();
        const dataCleanup = closeData();
        if (engine) {
            const release = () => {
                try { Promise.resolve(engine.close()).catch(() => log?.warn("Sheet engine cleanup failed")); }
                catch { log?.warn("Sheet engine cleanup failed"); }
            };
            if (dataCleanup) void dataCleanup.finally(release); else release();
        }
    });
    if (engine) {
        busy = true;
        readProjection().catch(() => {
            if (!closed) status.text = "Engine viewport unavailable. Workbook editing remains disabled.";
        }).finally(() => {
            starting = false; busy = false; updateEnabled();if(pendingViewport)void flushViewport();
            const grant = pendingOpen; pendingOpen = null;
            if (grant && !closed) void dispatchDocument('open-grant', grant);
        });
    }
    return undefined;
}
