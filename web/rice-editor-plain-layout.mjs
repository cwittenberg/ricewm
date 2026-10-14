import { plainEditorFont } from './rice-editor-content-fonts.mjs';

const segmenter = typeof Intl.Segmenter === 'function' ? new Intl.Segmenter(undefined, {granularity:'grapheme'}) : null;
export function editorGraphemes(text) {
    if (segmenter) return Array.from(segmenter.segment(text), item => item.segment);
    const output = [];
    for (const scalar of text) {
        const previous = output.at(-1);
        if (previous && (/\p{Mark}|\p{Emoji_Modifier}|[\u200d\ufe00-\ufe0f]/u.test(scalar)
            || previous.endsWith('\u200d')
            || /^\p{Regional_Indicator}$/u.test(previous) && /^\p{Regional_Indicator}$/u.test(scalar))) output[output.length - 1] += scalar;
        else output.push(scalar);
    }
    return output;
}

export function wrapEditorLine(context, line, available, mode, breakLongWords = false) {
    if (mode === 'none' || context.measureText(line).width <= available)
        return [{ text: line, start: 0, end: Array.from(line).length }];
    const words = mode === 'word' ? line.split(/(\s+)/).filter(Boolean) :
        breakLongWords ? editorGraphemes(line) : Array.from(line);
    const pieces = breakLongWords ? words.flatMap(word => context.measureText(word).width > available
        ? editorGraphemes(word) : [word]) : words;
    const lines = [];
    let current = '', start = 0;
    for (const piece of pieces) {
        const candidate = current + piece;
        if (current && context.measureText(candidate).width > available) {
            const end = start + Array.from(current).length;
            lines.push({ text: current, start, end }); start = end; current = piece;
        } else current = candidate;
    }
    lines.push({ text: current, start, end: start + Array.from(current).length });
    return lines;
}

export function plainEditorLayout(context, presentation, rect, size) {
    context.font = plainEditorFont(size);
    const logical = [];
    let start = 0;
    for (const match of presentation.document.matchAll(/([^\r\n]*)(\r\n|\r|\n|$)/g)) {
        logical.push({ text: match[1], start });
        start += Array.from(match[1]).length + Array.from(match[2]).length;
        if (!match[2]) break;
    }
    const digits = presentation.viewport?.digits ?? String(Math.max(1, presentation.lineNumberStart + logical.length - 1)).length;
    const gutter = presentation.lineNumbers ? Math.ceil(context.measureText('0'.repeat(digits + 1)).width) + 12 : 0;
    const lines = logical.flatMap((line, index) => wrapEditorLine(context, line.text,
        Math.max(1, rect.width - 8 - gutter), presentation.wrap, true).map((segment, segmentIndex) => ({
        line: segment.text, start: line.start + segment.start, end: line.start + segment.end,
        logicalLine: presentation.lineNumberStart + index, showNumber: segmentIndex === 0 && !(index === 0 && presentation.viewport?.continuation),
        marker: '', markerWidth: 0, alignment: null, justify: false,
    })));
    const lineHeight = Math.max(size + 2, Math.ceil(size * 1.35));
    const maximumLines = Math.max(1, Math.floor((rect.height - 8) / lineHeight));
    const caret = Math.min(start, Math.max(0, presentation.caret || 0));
    const caretLine = Math.max(0, lines.findIndex((line, index) => caret >= line.start &&
        (caret < line.end || caret === line.end && (presentation.viewport?.caretUpstream || index === lines.length - 1 || lines[index + 1].start > caret))));
    const firstLine = presentation.viewport ? Math.floor(presentation.viewport.offset / lineHeight) : Math.max(0, caretLine - maximumLines + 1);
    const pixelOffset = presentation.viewport ? presentation.viewport.offset % lineHeight : 0;
    return { lines, firstLine, maximumLines: presentation.viewport ? Math.ceil((rect.height - 8 + pixelOffset) / lineHeight) + 1 : maximumLines, lineHeight, gutter, caretLine, pixelOffset };
}

/** Unicode scalar offsets, measured with exactly the face/wrapping used to paint. */
export function plainEditorHit(context, presentation, command, x, y) {
    const size = Math.max(8, Number(command.sizePx) || 12);
    context.save();
    try {
        const layout = plainEditorLayout(context, presentation, command.rect, size);
        const row = Math.max(0, Math.floor((y - command.rect.y - 4 + layout.pixelOffset) / layout.lineHeight));
        const line = layout.lines[Math.min(layout.lines.length - 1, layout.firstLine + row)];
        const scalars = Array.from(line.line);
        const boundaries = [0];
        for (const cluster of editorGraphemes(line.line)) boundaries.push(boundaries.at(-1) + Array.from(cluster).length);
        const advance = Math.max(0, x - command.rect.x - 4 - layout.gutter);
        // Binary search limits text measurement work even on unwrapped long lines.
        // Equal positions for combining marks resolve to the end of that cluster.
        let low = 0, high = boundaries.length - 1;
        const width = index => context.measureText(scalars.slice(0, boundaries[index]).join('')).width;
        while (low < high) {
            const mid = Math.ceil((low + high) / 2);
            if (width(mid) <= advance) low = mid; else high = mid - 1;
        }
        if (low < boundaries.length - 1 && advance - width(low) >= width(low + 1) - advance) low++;
        while (low < boundaries.length - 1 && width(low + 1) === width(low)) low++;
        return line.start + boundaries[low];
    } finally { context.restore(); }
}
