import {Button, Dialog, Label, Message, Row, TextField} from '@ricewm/app';
import {checkedSheetCommand} from './command-contract.mjs';
import {selectionRange} from './editor-actions.mjs';

/** Geometry of the already validated native viewport, not worksheet geometry. */
export function headerResizeHit(frame, x, y) {
    if (!frame || !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0 || x >= frame.width || y >= frame.height) return null;
    const rows = x < frame.row_header && y >= frame.column_header;
    const columns = y < frame.column_header && x >= frame.row_header;
    if (!rows && !columns) return null;
    const coordinate = rows ? y : x, origin = rows ? frame.column_header : frame.row_header;
    let best = null;
    for (const track of rows ? frame.rows : frame.columns) {
        // A clipped viewport boundary is not a real track edge. Frozen and
        // filtered tracks retain their authoritative logical identities.
        if (track.clip_end !== track.start + track.extent) continue;
        const edge = origin + track.start + track.extent, distance = Math.abs(coordinate - edge);
        if (distance <= 2 && (!best || distance < best.distance)) best = {axis:rows?'rows':'columns',index:track.index,
            extent:track.extent,origin:coordinate,distance,revision:frame.revision,sheet:frame.sheet};
    }
    return best ? Object.freeze(best) : null;
}
export function resizeDragValue(hit, coordinate) {
    if (!Number.isFinite(coordinate)) return null;
    const pixels = hit.extent + coordinate - hit.origin;
    return hit.axis === 'rows' ? Math.max(1, Math.min(409, pixels * .75)) : Math.max(1, Math.min(4096, pixels));
}
export function resizeCommand(sheetId, axis, first, last, value) {
    return checkedSheetCommand(axis === 'rows'
        ? {type:'resize-rows',sheetId,firstRow:first,lastRow:last,heightPt:value}
        : {type:'resize-columns',sheetId,firstColumn:first,lastColumn:last,widthPx:value});
}
export function createLayoutDialog({projection,axis,isCurrent,apply,onClose}) {
    const rows=axis==='rows',range=selectionRange(projection),layout=projection.selection.layout;
    if (!range || !layout || !['rows','columns'].includes(axis)) throw Error('Select a cell with authoritative layout information.');
    const first=rows?range.firstRow:range.firstColumn,last=rows?range.lastRow:range.lastColumn;
    const maximum=rows?409:4096,unit=rows?'points':'pixels',value=rows?layout.rowHeightPt:layout.columnWidthPx;
    const input=new TextField({name:'layout-value',value:String(value),focused:true,accessibleName:rows?'Row height in points':'Column width in pixels'});
    const status=new Label({name:'layout-status',text:value<1||value>maximum?'The imported active size is outside the editable range. Choose a supported size or Cancel.':''}),accept=new Button({name:'layout-apply',text:'Apply'}),cancel=new Button({name:'layout-cancel',text:'Cancel'});
    const dialog=new Dialog({name:'layout-dialog',text:rows?'Row height':'Column width',modal:true},[
        new Label({text:`${rows?'Height':'Width'} in ${unit} (1–${maximum}; fractional values allowed)`}),input,
        new Label({text:`Applies to ${last-first+1} selected ${rows?'row(s)':'column(s)'}, including hidden tracks. Resizing does not unhide them. The initial value is the active cell’s size.`}),status,
        new Row({text:'panel:actions'},[cancel,accept]),
    ]);
    let closed=false,acting=false;
    const close=()=>{if(!closed&&!acting){closed=true;onClose();}};
    async function submit() {
        if(closed||acting)return;
        let command;
        try {
            if(!isCurrent())throw Error('Workbook changed. Close and reopen this dialog.');
            const text=input.getProperty('value').trim();
            if(!/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(text))throw Error(`Enter a number from 1 to ${maximum} ${unit}.`);
            command=resizeCommand(projection.activeSheetId,axis,first,last,Number(text));
        }catch(error){status.text=String(error.message||error).slice(0,240);return;}
        acting=true;for(const c of [input,cancel,accept])c.setProperty('enabled',false);
        try{const done=await apply(command,projection.revision);if(!closed){if(done){acting=false;close();}else status.text='The size was not changed. Close and reopen this dialog to refresh.';}}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{acting=false;if(!closed)for(const c of [input,cancel,accept])c.setProperty('enabled',true);}
    }
    accept.on(Message.ACTIVATE,submit);cancel.on(Message.ACTIVATE,close);
    for(const control of [dialog,input,accept,cancel])control.on(Message.KEY,event=>{
        if(!event.detail?.pressed)return;
        if(event.detail.key==='Escape'||event.detail.keyCode===9){event.preventDefault();close();}
        if(control===input&&(event.detail.key==='Enter'||event.detail.keyCode===36)){event.preventDefault();void submit();}
    });
    dialog.on(Message.DESTROY,()=>{closed=true;});return dialog;
}
