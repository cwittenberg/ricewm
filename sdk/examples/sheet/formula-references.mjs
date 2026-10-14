// Presentation-only lexer for draft A1 references. The Wasm parser remains the
// authority for formulas; this never reads cells or evaluates workbook values.
const reference = /(?:(?:'((?:[^']|'')+)'|([\p{L}_][\p{L}\p{N}_.]*))!)?(\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6})(?::(\$?[A-Za-z]{1,3}\$?[1-9][0-9]{0,6}))?/uy;
const identifier = /[\p{L}\p{N}_.$]/u;
function cell(text) {
    const match = /^\$?([A-Za-z]+)\$?([0-9]+)$/u.exec(text);
    let column = 0;
    for (const ch of match[1].toUpperCase()) column = column * 26 + ch.charCodeAt(0) - 64;
    return { row: Number(match[2]) - 1, column: column - 1 };
}
export function formulaReferences(draft, { sheetName = '', rows = 1048576, columns = 16384 } = {}) {
    if (typeof draft !== 'string' || !draft.startsWith('=')) return [];
    const result = [], seen = new Set();
    for (let offset = 1; offset < draft.length && result.length < 16;) {
        // Excel quoted strings escape quotes by doubling them.
        if (draft[offset] === '"') {
            for (++offset; offset < draft.length; ++offset) if (draft[offset] === '"') {
                if (draft[offset + 1] === '"') ++offset; else { ++offset; break; }
            }
            continue;
        }
        // Structured/external references are not local A1 rectangles.
        if (draft[offset] === '[') {
            let depth = 1; ++offset;
            while (offset < draft.length && depth) { if (draft[offset] === '[') ++depth; if (draft[offset] === ']') --depth; ++offset; }
            while (offset < draft.length && /[^+*/^&=<>(),;\s]/u.test(draft[offset])) ++offset;
            continue;
        }
        reference.lastIndex = offset;
        const match = (!identifier.test(draft[offset - 1]) && draft[offset - 1] !== '!') ? reference.exec(draft) : null;
        if (!match) { ++offset; continue; }
        const end = reference.lastIndex;
        if ((draft[end] && identifier.test(draft[end])) || /^\s*\(/u.test(draft.slice(end)) || draft[end] === '!') { offset = end; continue; }
        const scope = (match[1]?.replaceAll("''", "'") ?? match[2] ?? sheetName).toLocaleLowerCase('en-US');
        const first = cell(match[3]), last = cell(match[4] ?? match[3]);
        if (scope === sheetName.toLocaleLowerCase('en-US') && first.row < rows && last.row < rows && first.column < columns && last.column < columns) {
            const rect = { first_row: Math.min(first.row, last.row), last_row: Math.max(first.row, last.row),
                first_column: Math.min(first.column, last.column), last_column: Math.max(first.column, last.column) };
            const key = JSON.stringify(rect);
            if (!seen.has(key)) { seen.add(key); result.push({ ...rect, color: result.length % 8 }); }
        }
        offset = end;
    }
    return result;
}
