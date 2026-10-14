// Presentation-only contract shared with rice-ui::grid_frame. Workbook state,
// formulas and geometry generation stay in the app's authoritative engine.
export const GRID_FRAME_LEGACY_MAX_BYTES = 64 * 1024;
export const GRID_FRAME_MAX_BYTES = 256 * 1024;
export const GRID_LARGE_FRAME_CAPABILITY = 1n << 14n;
export const GRID_FILTER_HEADER_CAPABILITY = 1n << 15n;
export const GRID_REFERENCES_CAPABILITY = 1n << 16n;
export const GRID_PAINT_CAPABILITY = 1n << 19n;
export const GRID_DOCUMENT_CANVAS_CAPABILITY = 1n << 21n;
export const GRID_FRACTIONAL_FONT_CAPABILITY = 1n << 22n;
export const GRID_MERGES_CAPABILITY = 1n << 24n;
export const DOCUMENT_ALIGNMENT_CAPABILITY = 1n << 25n;
export const GRID_ZOOM_CAPABILITY = 1n << 20n;
export const GRID_FRAME_MAX_STYLES = 4096;
export const GRID_FILL_ENDPOINT_CAPABILITY = 1n << 11n;
export const GRID_TEXT_STYLE_CAPABILITY = 1n << 12n;
const integer = (n, min, max) => Number.isSafeInteger(n) && n >= min && n <= max;
const fail = () => { throw new RangeError("invalid virtual grid frame"); };
const fields = (value, names) => value && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).length === names.length && names.every(name => Object.hasOwn(value, name));
const trackFields = ["index", "start", "extent", "clip_start", "clip_end", "frozen"];
const frameFields = ["version", "revision", "sheet", "row_count", "column_count", "width", "height",
    "row_header", "column_header", "rows", "columns", "strings", "styles", "cells", "selection", "active"];
const byteLength = value => new TextEncoder().encode(value).length;
function tracks(list, count, size) {
    if (!Array.isArray(list) || list.length > 512) fail();
    const identities = new Set();
    let end = 0, scrolling = false;
    for (const t of list) {
        if (!fields(t, trackFields) || !integer(t.index, 0, count - 1) || identities.has(t.index)
            || !integer(t.start, -1_000_000, 65536) || !integer(t.extent, 1, 1_000_000)
            || !integer(t.clip_start, end, size) || !integer(t.clip_end, t.clip_start + 1, size)
            || t.clip_start < t.start || t.clip_end > t.start + t.extent
            || typeof t.frozen !== "boolean" || (scrolling && t.frozen)) fail();
        identities.add(t.index); end = t.clip_end; scrolling ||= !t.frozen;
    }
}
export function validateGridStyle(style) {
    if (!fields(style, ["alignment", "role", ...["font","paint"].filter(k=>Object.hasOwn(style??{},k))])
        || !integer(style.alignment, 0, 2) || !integer(style.role, 0, 3)) fail();
    if (Object.hasOwn(style, "font")) {
        const font = style.font;
        if (!fields(font, ["family", "size_pt", "weight", "italic", "underline"])
            || typeof font.family !== "string" || !font.family.length || byteLength(font.family) > 128
            || /[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(font.family)
            || (!Number.isFinite(font.size_pt) || font.size_pt < 1 || font.size_pt > 409) || !integer(font.weight, 100, 900) || font.weight % 100 !== 0
            || typeof font.italic !== "boolean" || typeof font.underline !== "boolean") fail();
    }
    if (Object.hasOwn(style,"paint")) {
        const p=style.paint, allowed=["foreground","background","borders","vertical","wrap","strike"], color=v=>typeof v==="string"&&/^#[0-9a-f]{6}$/iu.test(v);
        if(!p||typeof p!=="object"||Array.isArray(p)||Object.keys(p).some(k=>!allowed.includes(k)))fail();
        for(const k of ["foreground","background"])if(Object.hasOwn(p,k)&&!color(p[k]))fail();
        if(Object.hasOwn(p,"vertical")&&!["top","center","bottom"].includes(p.vertical))fail();
        for(const k of ["wrap","strike"])if(Object.hasOwn(p,k)&&typeof p[k]!=="boolean")fail();
        if(Object.hasOwn(p,"borders")) {
            const b=p.borders,edges=["top","right","bottom","left"];
            if(!b||typeof b!=="object"||Array.isArray(b)||Object.keys(b).some(k=>!edges.includes(k)))fail();
            for(const edge of Object.values(b))if(!fields(edge,["style","color"])||!color(edge.color)||!["hair","thin","medium","thick","double","dotted","dashed","dashDot","dashDotDot","mediumDashed","mediumDashDot","mediumDashDotDot","slantDashDot"].includes(edge.style))fail();
        }
    }
    return style;
}
export function validateGridReferences(references, rows = 0xffffffff, columns = 0xffffffff) {
    if (!Array.isArray(references) || references.length > 16) fail();
    for (const r of references) {
        if (!fields(r, ["first_row", "last_row", "first_column", "last_column", "color"])
            || !integer(r.first_row, 0, rows - 1) || !integer(r.last_row, r.first_row, rows - 1)
            || !integer(r.first_column, 0, columns - 1) || !integer(r.last_column, r.first_column, columns - 1)
            || !integer(r.color, 0, 7)) fail();
    }
    return references;
}
export const gridMergeValues = m => [m.first_row,m.last_row,m.first_column,m.last_column,m.row_slot,m.column_slot,m.x,m.y,m.width,m.height];
export function gridMergeAt(frame,row,column) {
    return frame.merges?.find(m=>row>=m.first_row&&row<=m.last_row&&column>=m.first_column&&column<=m.last_column)??null;
}
function validateMerges(frame) {
    const list=frame.merges;
    if(!Array.isArray(list)||list.length>512)fail();
    for(let i=0;i<list.length;i++) {
        const m=list[i];
        if(!fields(m,["first_row","last_row","first_column","last_column","row_slot","column_slot","x","y","width","height"])
            ||!integer(m.first_row,0,frame.row_count-1)||!integer(m.last_row,m.first_row,frame.row_count-1)
            ||!integer(m.first_column,0,frame.column_count-1)||!integer(m.last_column,m.first_column,frame.column_count-1)
            ||(m.first_row===m.last_row&&m.first_column===m.last_column)
            ||!integer(m.row_slot,0,frame.rows.length-1)||!integer(m.column_slot,0,frame.columns.length-1)
            ||!integer(m.x,-1_000_000_000,65536)||!integer(m.y,-1_000_000_000,65536)
            ||!integer(m.width,1,1_000_000_000)||!integer(m.height,1,1_000_000_000))fail();
        const rs=frame.rows.filter(t=>t.index>=m.first_row&&t.index<=m.last_row),cs=frame.columns.filter(t=>t.index>=m.first_column&&t.index<=m.last_column);
        if(!rs.length||!cs.length||frame.rows[m.row_slot]!==rs[0]||frame.columns[m.column_slot]!==cs[0]
            ||m.x>cs[0].clip_start||m.y>rs[0].clip_start||m.x+m.width<cs.at(-1).clip_end||m.y+m.height<rs.at(-1).clip_end)fail();
        if(list.slice(0,i).some(n=>n.first_row<=m.last_row&&n.last_row>=m.first_row&&n.first_column<=m.last_column&&n.last_column>=m.first_column))fail();
    }
}
export function validateGridFrame(frame) {
    if (!fields(frame, [...frameFields, ...["filter_columns", "fill_endpoint", "filter_header_row", "references", "zoom_percent", "canvas", "merges"].filter(name => Object.hasOwn(frame ?? {}, name))]) || frame.version !== 1
        || !integer(frame.revision, 0, Number.MAX_SAFE_INTEGER) || !integer(frame.sheet, 0, 0xffffffff)
        || !integer(frame.row_count, 1, 0xffffffff) || !integer(frame.column_count, 1, 0xffffffff)
        || !integer(frame.width, 1, 65536) || !integer(frame.height, 1, 65536)
        || !integer(frame.row_header, 0, Math.min(256, frame.width))
        || !integer(frame.column_header, 0, Math.min(256, frame.height))) fail();
    if (Object.hasOwn(frame, "zoom_percent") && !integer(frame.zoom_percent, 25, 400)) fail();
    if (Object.hasOwn(frame, "canvas") && frame.canvas !== "document") fail();
    tracks(frame.rows, frame.row_count, frame.height - frame.column_header);
    if (Object.hasOwn(frame, "references")) validateGridReferences(frame.references, frame.row_count, frame.column_count);
    tracks(frame.columns, frame.column_count, frame.width - frame.row_header);
    if(Object.hasOwn(frame,"merges"))validateMerges(frame);
    if (frame.rows.length * frame.columns.length > 4096
        || !Array.isArray(frame.cells) || frame.cells.length > 4096
        || !Array.isArray(frame.strings) || frame.strings.length > 4096
        || !Array.isArray(frame.styles) || !frame.styles.length || frame.styles.length > GRID_FRAME_MAX_STYLES) fail();
    if (Object.hasOwn(frame, "filter_columns")) {
        if (!Array.isArray(frame.filter_columns) || frame.filter_columns.length > 512) fail();
        const columns = new Set();
        for (const entry of frame.filter_columns) {
            if (!fields(entry, ["column", "active"]) || !integer(entry.column, 0, frame.column_count - 1)
                || typeof entry.active !== "boolean" || columns.has(entry.column)) fail();
            columns.add(entry.column);
        }
    }
    if (Object.hasOwn(frame, "filter_header_row") && !integer(frame.filter_header_row, 0, frame.row_count - 1)) fail();
    let bytes = 0;
    const multiline = new Set();
    for (const [index,s] of frame.strings.entries()) {
        // Unicode mode matches only unpaired surrogate code units here. Valid
        // astral pairs remain legal and serialize to native UTF-8 Strings.
        if (typeof s !== "string" || /[\u0000-\u0009\u000b-\u001f\u007f-\u009f\ud800-\udfff]/u.test(s) || byteLength(s) > 4096) fail();
        if(s.includes("\n"))multiline.add(index);
        bytes += byteLength(s);
    }
    if (bytes > GRID_FRAME_MAX_BYTES) fail();
    for (const style of frame.styles) {
        validateGridStyle(style);
    }
    const referencedMultiline = new Set();
    let previous = -1;
    for (const cell of frame.cells) {
        if (!Array.isArray(cell) || cell.length !== 4 || !integer(cell[0], 0, frame.rows.length - 1)
            || !integer(cell[1], 0, frame.columns.length - 1) || !integer(cell[2], 0, frame.strings.length - 1)
            || !integer(cell[3], 0, frame.styles.length - 1)) fail();
        if(multiline.has(cell[2])){if(frame.styles[cell[3]].paint?.wrap!==true)fail();referencedMultiline.add(cell[2]);}
        const key = cell[0] * 512 + cell[1];
        if (key <= previous) fail();
        previous = key;
    }
    if(referencedMultiline.size!==multiline.size)fail();
    const s = frame.selection;
    if (s !== null && (!fields(s, ["first_row", "last_row", "first_column", "last_column"])
        || !integer(s.first_row, 0, frame.rows.length - 1) || !integer(s.last_row, s.first_row, frame.rows.length - 1)
        || !integer(s.first_column, 0, frame.columns.length - 1)
        || !integer(s.last_column, s.first_column, frame.columns.length - 1))) fail();
    if (Object.hasOwn(frame, "fill_endpoint") && (!Array.isArray(frame.fill_endpoint) || frame.fill_endpoint.length !== 2
        || !integer(frame.fill_endpoint[0], 0, frame.row_count - 1)
        || !integer(frame.fill_endpoint[1], 0, frame.column_count - 1))) fail();
    const active = frame.active;
    if (active !== null && (!Array.isArray(active) || active.length !== 2
        || !integer(active[0], 0, frame.row_count - 1) || !integer(active[1], 0, frame.column_count - 1))) fail();
    if (byteLength(JSON.stringify(frame)) > GRID_FRAME_MAX_BYTES) fail();
    return frame;
}
function freeze(value) {
    if (value && typeof value === "object") {
        for (const child of Object.values(value)) freeze(child);
        Object.freeze(value);
    }
    return value;
}
export function copyGridFrame(frame) {
    validateGridFrame(frame);
    return freeze(JSON.parse(JSON.stringify(frame)));
}
export function gridFrameHit(frame, x, y) {
    if (!integer(x, 0, frame.width - 1) || !integer(y, 0, frame.height - 1)) return null;
    const row = y < frame.column_header ? null
        : frame.rows.find(t => y - frame.column_header >= t.clip_start && y - frame.column_header < t.clip_end)?.index;
    const column = x < frame.row_header ? null
        : frame.columns.find(t => x - frame.row_header >= t.clip_start && x - frame.row_header < t.clip_end)?.index;
    if (row === undefined || column === undefined) return null;
    if (column !== null && frame.filter_columns?.some(entry => entry.column === column)) {
        const track = frame.columns.find(t => t.index === column);
        const header = frame.filter_header_row === undefined ? null : frame.rows.find(t => t.index === frame.filter_header_row);
        const headerVisible = frame.filter_header_row === undefined || Boolean(header);
        const span = header ? header.clip_end - header.clip_start : frame.column_header;
        const height = Math.min(18, span);
        const top = (header ? frame.column_header + header.clip_start : 0) + Math.floor((span - height) / 2);
        const left = frame.row_header + track.clip_end - 18;
        if (headerVisible && track.clip_end - track.clip_start >= 24 && span >= 12
            && x >= left && x < left + 16 && y >= top && y < top + height) {
            return Object.freeze({ kind: "filter", revision: frame.revision, sheet: frame.sheet, ...(frame.filter_header_row === undefined ? {} : {row}), column });
        }
    }
    const selection = frame.selection;
    if (selection && !frame.merges?.some(m=>m.first_row<=frame.rows[selection.last_row].index&&m.last_row>=frame.rows[selection.first_row].index&&m.first_column<=frame.columns[selection.last_column].index&&m.last_column>=frame.columns[selection.first_column].index)) {
        const r = frame.rows[selection.last_row], c = frame.columns[selection.last_column];
        const left = frame.row_header + c.clip_end - 7, top = frame.column_header + r.clip_end - 7;
        if ((!frame.fill_endpoint || (frame.fill_endpoint[0] === r.index && frame.fill_endpoint[1] === c.index))
            && c.clip_end - c.clip_start >= 8 && r.clip_end - r.clip_start >= 8
            && c.clip_end === c.start + c.extent && r.clip_end === r.start + r.extent
            && x >= left && x < left + 6 && y >= top && y < top + 6) {
            return Object.freeze({ kind: "fill-handle", revision: frame.revision, sheet: frame.sheet, row: r.index, column: c.index });
        }
    }
    const merged=row===null||column===null?null:gridMergeAt(frame,row,column);
    return Object.freeze({ revision: frame.revision, sheet: frame.sheet, row:merged?.first_row??row, column:merged?.first_column??column });
}
