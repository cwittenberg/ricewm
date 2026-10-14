import {checkedScroll} from './viewport-contract.mjs';
/**
 * UI-side adapter contract, not a calculation engine or a workbook model.
 *
 * The future host-owned Wasm adapter must implement:
 *   version: 1
 *   readViewport({top, left, rows, columns}) -> Promise<projection>
 *   command(typedCommand) -> Promise<projection | void>
 * UI-side version 1 is transport-independent. Backend service version 2 owns
 * atomic select/edit/undo/redo requests, using zero-based logical coordinates.
 *   close() -> Promise<void> | void
 *
 * A projection contains documentTitle, revision, activeSheetId, sheets
 * [{id, name, gridId?}], selection {address, formula}, status and optional
 * capabilities listing supported typed commands. Missing capabilities disables
 * engine actions. Optional history {canUndo, canRedo} describes the engine's
 * unified history; absence retains compatibility with earlier providers.
 * The formula bar forwards raw input without calculation. Optional
 * frame is a Uint8Array containing <=64KiB UTF-8 JSON for SDK GridFrame v1.
 * Every sheet must carry a unique u32 gridId when a frame is present; frame.sheet
 * must match the active sheet gridId, and frame.revision the metadata revision.
 * checkedProjection decodes that frame into immutable presentation data for
 * VirtualGrid. All values come from the engine; the shell never calculates.
 * Alternatively viewUpdate carries a bounded snapshot/delta envelope consumed
 * by a per-view SheetViewportCache. It cannot be combined with frame.
 * Instances are owned by a single shell and closed with it. The injected
 * port is a development seam, not permission to expose host objects to apps.
 */
import { copyGridFrame, GRID_FRAME_MAX_BYTES, validateGridStyle } from "@ricewm/app";
import { checkedCapabilities, checkedSheetRange, checkedSelectionValidation } from "./command-contract.mjs";

export function checkedEnginePort(port) {
    if (port === null) return null;
    if (port?.version !== 1 || ["readViewport", "command", "close"].some(key => typeof port[key] !== "function")) {
        throw new TypeError("Rice Sheet requires engine port version 1");
    }
    return port;
}

function text(value, limit, label) {
    if (typeof value !== "string" || value.length > limit) throw new TypeError(`Invalid engine ${label}`);
    return value;
}

function checkedFormat(value) {
    if (value === undefined) return undefined;
    const keys = ['fontColor','fillColor','borders','horizontal','vertical','wrapText','strike'];
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).length !== keys.length ||
        Object.keys(value).some(key => !keys.includes(key)) ||
        !['general','left','center','right','justify','fill','centerContinuous','distributed'].includes(value.horizontal) ||
        !['top','center','bottom','justify','distributed'].includes(value.vertical) ||
        typeof value.wrapText !== 'boolean' || typeof value.strike !== 'boolean') throw new TypeError('Invalid selected cell formatting');
    for (const key of ['fontColor','fillColor']) if (value[key] !== null && (typeof value[key] !== 'string' || !/^#[0-9a-f]{6}$/i.test(value[key]))) throw new TypeError('Invalid selected cell color');
    const paint = {borders:value.borders};
    if (value.fontColor !== null) paint.foreground=value.fontColor;
    if (value.fillColor !== null) paint.background=value.fillColor;
    // The SDK owns the single strict border/paint schema shared with the grid.
    validateGridStyle({alignment:0,role:0,paint});
    const borders=Object.freeze(Object.fromEntries(Object.entries(value.borders).map(([edge,item])=>[edge,Object.freeze({...item})])));
    return Object.freeze({...value,borders});
}

function checkedSelectedMerge(value) {
    if(value===undefined)return undefined;
    const keys=['first_row','last_row','first_column','last_column'];
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==4||keys.some(k=>!Number.isSafeInteger(value[k])||value[k]<0)
        ||value.last_row<value.first_row||value.last_row>=1048576||value.last_column<value.first_column||value.last_column>=16384
        ||(value.first_row===value.last_row&&value.first_column===value.last_column))throw new TypeError('Invalid selected merged cell');
    return Object.freeze({...value});
}

export function checkedProjection(value, viewportCache = null) {
    if (!value || !Number.isSafeInteger(value.revision) || value.revision < 0 ||
        !Array.isArray(value.sheets) || value.sheets.length > 256) {
        throw new TypeError("Invalid engine viewport metadata");
    }
    if (viewportCache?.current && value.revision < viewportCache.current.frame.revision) {
        throw new TypeError("Stale engine projection revision");
    }
    const seen = new Set();
    const gridIds = new Set();
    const sheets = value.sheets.map(sheet => {
        const id = text(sheet.id, 128, "sheet id");
        if (!id || seen.has(id)) throw new TypeError("Invalid engine sheet identity");
        seen.add(id);
        const gridId = sheet.gridId;
        if ((value.frame !== undefined || value.viewUpdate !== undefined || gridId !== undefined) &&
            (!Number.isSafeInteger(gridId) || gridId < 0 || gridId > 0xffffffff || gridIds.has(gridId))) {
            throw new TypeError("Invalid engine grid sheet identity");
        }
        if (gridId !== undefined) gridIds.add(gridId);
        return Object.freeze({ id, name: text(sheet.name, 256, "sheet name"), ...(gridId === undefined ? {} : { gridId }) });
    });
    if (!seen.has(value.activeSheetId)) throw new TypeError("Invalid active engine sheet");
    for (const key of ['readOnly', 'truncated', 'canMerge', 'canUnmerge']) {
        if (value.selection?.[key] !== undefined && typeof value.selection[key] !== 'boolean') throw new TypeError('Invalid cell editor state');
    }
    const layout = value.selection?.layout;
    if (layout !== undefined && (!layout || typeof layout !== 'object' || Array.isArray(layout) || Object.keys(layout).length !== 2 ||
        !Number.isFinite(layout.rowHeightPt) || layout.rowHeightPt < 0 || layout.rowHeightPt > 750000 ||
        !Number.isFinite(layout.columnWidthPx) || layout.columnWidthPx < 0 || layout.columnWidthPx > 1000000)) throw new TypeError('Invalid engine selection layout');
    const selectedMerge=checkedSelectedMerge(value.selection?.merge);
    const selectedFont = value.selection?.font;
    if (selectedFont !== undefined) validateGridStyle({ alignment: 0, role: 0, font: selectedFont });
    const selectedFormat = checkedFormat(value.selection?.format);
    const metadata = {
        revision: value.revision,
        capabilities: checkedCapabilities(value.capabilities),
        documentTitle: text(value.documentTitle, 512, "document title"),
        activeSheetId: value.activeSheetId,
        sheets: Object.freeze(sheets),
        selection: Object.freeze({
            address: text(value.selection?.address, 128, "selection address"),
            canMerge: value.selection?.canMerge === true,
            canUnmerge: value.selection?.canUnmerge === true,
            formula: text(value.selection?.formula, 32768, "selection formula"),
            ...(value.selection?.numberFormat === undefined ? {} : { numberFormat: text(value.selection.numberFormat, 2048, "number format") }),
            ...(value.selection?.range === undefined ? {} : { range: checkedSheetRange(value.selection.range) }),
            ...(layout === undefined ? {} : { layout: Object.freeze({ ...layout }) }),
            ...(selectedMerge === undefined ? {} : {merge:selectedMerge}),
            ...(selectedFont === undefined ? {} : { font: Object.freeze({ ...selectedFont }) }),
            ...(selectedFormat === undefined ? {} : {format:selectedFormat}),
            ...(value.selection?.validation === undefined ? {} : {validation:checkedSelectionValidation(value.selection.validation)}),
            ...(value.selection?.readOnly === undefined ? {} : {readOnly:value.selection.readOnly}),
            ...(value.selection?.truncated === undefined ? {} : {truncated:value.selection.truncated}),
        }),
        status: text(value.status, 1024, "status"),
    };
    if (value.scroll !== undefined) metadata.scroll = checkedScroll(value.scroll);
    if (value.filterState !== undefined) {
        const state=value.filterState;
        if (!state || typeof state!=='object' || Array.isArray(state) || Object.keys(state).length!==2 ||
            typeof state.enabled!=='boolean' || !Number.isInteger(state.appliedColumns) || state.appliedColumns<0 || state.appliedColumns>16384 ||
            (!state.enabled && state.appliedColumns!==0)) throw new TypeError('Invalid engine filter state');
        metadata.filterState=Object.freeze({...state});
    }
    if (value.history !== undefined) {
        const history = value.history;
        if (!history || typeof history !== 'object' || Array.isArray(history) ||
            Object.keys(history).length !== 2 || typeof history.canUndo !== 'boolean' || typeof history.canRedo !== 'boolean') {
            throw new TypeError('Invalid engine history availability');
        }
        metadata.history = Object.freeze({ canUndo: history.canUndo, canRedo: history.canRedo });
    }
    if (value.calculation !== undefined) {
        const stats = value.calculation;
        if (stats !== null && (!stats || typeof stats !== 'object' ||
            Object.keys(stats).length !== 3 || typeof stats.incremental !== 'boolean' ||
            !['affectedCells', 'evaluatedFormulas'].every(key => Number.isSafeInteger(stats[key]) && stats[key] >= 0))) {
            throw new TypeError('Invalid bounded calculation counters');
        }
        metadata.calculation = stats === null ? null : Object.freeze({...stats});
    }
    if (value.frame !== undefined && value.viewUpdate !== undefined) throw new TypeError("Conflicting engine viewport encodings");
    let frame;
    let damage;
    if (value.frame !== undefined) {
        if (!(value.frame instanceof Uint8Array) || value.frame.byteLength > GRID_FRAME_MAX_BYTES) {
            throw new TypeError("Invalid encoded engine grid frame");
        }
        frame = copyGridFrame(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(value.frame)));
        if (frame.revision !== value.revision || frame.sheet !== sheets.find(sheet => sheet.id === value.activeSheetId).gridId) {
            throw new TypeError("Engine grid frame does not match its revision and active sheet");
        }
        viewportCache?.clear();
    } else if (value.viewUpdate !== undefined) {
        if (!(value.viewUpdate instanceof Uint8Array) || value.viewUpdate.byteLength > GRID_FRAME_MAX_BYTES || !viewportCache) {
            throw new TypeError("Encoded engine delta requires a bounded per-view cache");
        }
        const update = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(value.viewUpdate));
        const presented = update.kind === "snapshot" ? update.frame : update;
        if (presented?.revision !== value.revision || presented?.sheet !== sheets.find(sheet => sheet.id === value.activeSheetId).gridId) {
            throw new TypeError("Engine viewport delta does not match its revision and active sheet");
        }
        damage = viewportCache.apply(update);
        frame = damage.frame;
    } else {
        viewportCache?.clear();
    }
    if (frame && metadata.selection.range) {
        // The engine's selection rectangle uses clipped visible slots. Keep
        // its highlight, but identify the true logical fill endpoint separately.
        // This decorates presentation only; the engine viewport cache is raw.
        frame = copyGridFrame({ ...frame, fill_endpoint: [metadata.selection.range.lastRow, metadata.selection.range.lastColumn] });
    }
    return Object.freeze({
        ...(frame === undefined ? {} : { frame }),
        ...(damage === undefined ? {} : { damage }),
        ...metadata,
    });
}
