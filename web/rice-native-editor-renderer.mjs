import { ensureOfficeFonts } from './rice-office-fonts.mjs';
import { paintOfficeEditor, paintAttributedRichEditor, parseOfficeLayout } from "./rice-office-editor-renderer.mjs";

import { ensureEditorContentFonts, plainEditorFont } from "./rice-editor-content-fonts.mjs";
import { plainEditorLayout, plainEditorHit, wrapEditorLine } from "./rice-editor-plain-layout.mjs";

import { rgbaColor } from "./rice-native-canvas-primitives.mjs";

const SYNTAX_KEYWORDS = new Set([
    "as", "async", "await", "break", "case", "class", "const", "continue",
    "def", "do", "else", "enum", "export", "false", "fn", "for", "from",
    "function", "if", "impl", "import", "in", "interface", "let", "loop",
    "match", "mod", "move", "mut", "new", "none", "null", "pub", "raise",
    "return", "self", "static", "struct", "super", "switch", "this", "throw",
    "trait", "true", "try", "type", "use", "var", "where", "while", "yield",
]);

export function parsePresentation(value) {
    const text = String(value || "");
    const match = /^editor:([^:\n]+):(none|word|char):(lines|nolines)(?::(\d+):(\d+)(?::(currentline|nocurrentline)(?::(\d+))?(?::(adaptive|vibrant|soft|mono)(?::(\d+))?)?)?)?\n/.exec(
        text,
    );
    if (!match) {
        return {
            language: "plain",
            wrap: "none",
            lineNumbers: false,
            highlightCurrentLine: false,
            caret: null,
            anchor: null,
            lineNumberStart: 1,
            forceDarkMode: false,
            document: text,
        };
    }
    const forceDarkMode = match[1].endsWith("~dark");
    const encodedLanguage = forceDarkMode ? match[1].slice(0, -5) : match[1];
    const languageSize = /^(.*)@(\d+)$/.exec(encodedLanguage) ||
        /^(rich(?:~.*)?)~z(\d+)$/.exec(encodedLanguage);
    const suffixSize = languageSize ? Number(languageSize[2]) : null;
    const trailingSize = match[9] === undefined ? null : Number(match[9]);
    const fontSize = trailingSize ?? suffixSize;
    const baseLanguage = languageSize?.[1] || encodedLanguage;
    const viewportToken = /~V(\d+),(\d+),(\d+),([01]),([01])(?:,([01]))?$/.exec(baseLanguage);
    const viewport = viewportToken ? {offset:Number(viewportToken[1]), extent:Number(viewportToken[2]), digits:Number(viewportToken[3]), continuation:viewportToken[4]==='1', caretVisible:viewportToken[5]==='1', caretUpstream:viewportToken[6]==='1'} : null;
    return {
        viewport,
        language: viewportToken ? baseLanguage.slice(0, viewportToken.index) : baseLanguage,
        syntaxStyle: match[8] || "adaptive",
        wrap: match[2],
        lineNumbers: match[3] === "lines",
        highlightCurrentLine: match[6] === "currentline",
        caret: match[4] === undefined ? null : Number(match[4]),
        anchor: match[5] === undefined ? null : Number(match[5]),
        lineNumberStart: Math.max(1, Number(match[7]) || 1),
        fontSize: Number.isFinite(fontSize) && fontSize >= 6 && fontSize <= 96
            ? fontSize
            : null,
        forceDarkMode,
        document: text.slice(match[0].length),
    };
}

function mixedColor(left, right, weight = 0.5) {
    return left.map((value, index) =>
        index === 3
            ? value
            : Math.round(value * (1 - weight) + right[index] * weight)
    );
}

function relativeLuminance(color) {
    const channels = color.slice(0, 3).map(value => {
        const channel = value / 255;
        return channel <= 0.04045
            ? channel / 12.92
            : ((channel + 0.055) / 1.055) ** 2.4;
    });
    return channels[0] * 0.2126 + channels[1] * 0.7152 +
        channels[2] * 0.0722;
}

function contrastRatio(left, right) {
    const bright = Math.max(relativeLuminance(left), relativeLuminance(right));
    const dark = Math.min(relativeLuminance(left), relativeLuminance(right));
    return (bright + 0.05) / (dark + 0.05);
}

function caretColor(command, chrome) {
    const background = chrome?.body || [255, 255, 255, 255];
    const preferred = chrome?.selection || chrome?.active || command.color;
    if (contrastRatio(preferred, background) >= 3) {
        return [...preferred.slice(0, 3), 255];
    }
    if (contrastRatio(command.color, background) >= 3) {
        return [...command.color.slice(0, 3), 255];
    }
    const black = [0, 0, 0, 255];
    const white = [255, 255, 255, 255];
    return contrastRatio(black, background) >= contrastRatio(white, background)
        ? black
        : white;
}

function syntaxTokens(line, language) {
    if (language === "plain") return [{ kind: "text", text: line }];
    const comment = ["python", "shell", "yaml", "toml"].includes(language)
        ? /#.*$/
        : /\/\/.*$/;
    const output = [];
    let index = 0;
    const matcher =
        /"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\/\*.*?\*\/|\b\d+(?:\.\d+)?\b|\b[A-Za-z_$][A-Za-z0-9_$]*\b/g;
    const commentMatch = comment.exec(line);
    const codeEnd = commentMatch?.index ?? line.length;
    for (const match of line.slice(0, codeEnd).matchAll(matcher)) {
        if (match.index > index) {
            output.push({
                kind: "text",
                text: line.slice(index, match.index),
            });
        }
        const value = match[0];
        output.push({
            kind: value.startsWith('"') || value.startsWith("'") || value.startsWith("`")
                ? "string"
                : value.startsWith("/*")
                    ? "comment"
                : /^\d/.test(value)
                    ? "number"
                    : SYNTAX_KEYWORDS.has(value.toLowerCase())
                        ? "keyword"
                        : "text",
            text: value,
        });
        index = match.index + value.length;
    }
    if (index < codeEnd) {
        output.push({ kind: "text", text: line.slice(index, codeEnd) });
    }
    if (commentMatch) {
        output.push({ kind: "comment", text: line.slice(codeEnd) });
    }
    return output;
}

function syntaxPalette(style, command, chrome) {
    if (command.forceDarkMode) {
        const vivid = {
            text: command.color,
            keyword: [130, 180, 255, 255],
            string: [145, 215, 165, 255],
            number: [245, 185, 115, 255],
            comment: [165, 175, 190, 255],
        };
        if (style === "mono") {
            return { ...vivid, keyword: command.color, string: command.color, number: command.color };
        }
        if (style === "soft") {
            return {
                text: command.color,
                keyword: mixedColor(command.color, vivid.keyword, 0.62),
                string: mixedColor(command.color, vivid.string, 0.58),
                number: mixedColor(command.color, vivid.number, 0.58),
                comment: mixedColor(command.color, vivid.comment, 0.7),
            };
        }
        return vivid;
    }
    const adaptive = {
        text: command.color,
        keyword: chrome?.active || mixedColor(command.color, [92, 138, 220, 255], 0.55),
        string: mixedColor(command.color, chrome?.active || [80, 175, 110, 255], 0.45),
        number: mixedColor(command.color, chrome?.close || [210, 130, 70, 255], 0.45),
        comment: mixedColor(command.color, chrome?.control || [128, 128, 128, 255], 0.55),
    };
    if (style === "vibrant") {
        return {
            text: command.color,
            keyword: mixedColor(command.color, chrome?.active || [75, 120, 255, 255], 0.82),
            string: mixedColor(command.color, [16, 158, 88, 255], 0.72),
            number: mixedColor(command.color, [220, 92, 40, 255], 0.74),
            comment: mixedColor(command.color, [115, 115, 125, 255], 0.72),
        };
    }
    if (style === "soft") {
        return {
            text: command.color,
            keyword: mixedColor(command.color, adaptive.keyword, 0.42),
            string: mixedColor(command.color, adaptive.string, 0.38),
            number: mixedColor(command.color, adaptive.number, 0.38),
            comment: mixedColor(command.color, adaptive.comment, 0.44),
        };
    }
    if (style === "mono") {
        return {
            text: command.color,
            keyword: command.color,
            string: command.color,
            number: command.color,
            comment: mixedColor(command.color, chrome?.control || [128, 128, 128, 255], 0.55),
        };
    }
    return adaptive;
}

function richFormatting(language) {
    if (language === "rich") return { inline: [], blocks: [] };
    if (!language.startsWith("rich~")) return null;
    const inline = [];
    const blocks = [];
    for (const encoded of language.split("~").slice(1, 257)) {
        const match = /^([biusQBNACRJ])(\d+)-(\d+)$/u.exec(encoded);
        if (!match) continue;
        const start = Number(match[2]);
        const end = Number(match[3]);
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end) continue;
        const range = { style: match[1], start, end };
        if (/^[bius]$/u.test(match[1])) inline.push(range);
        else blocks.push(range);
    }
    return { inline, blocks };
}

function richSegments(text, lineStart, ranges) {
    const length = Array.from(text).length;
    const boundaries = new Set([0, length]);
    for (const range of ranges) {
        const start = Math.max(0, Math.min(length, range.start - lineStart));
        const end = Math.max(0, Math.min(length, range.end - lineStart));
        if (start < end) {
            boundaries.add(start);
            boundaries.add(end);
        }
    }
    const points = [...boundaries].sort((left, right) => left - right);
    const characters = Array.from(text);
    return points.slice(0, -1).map((start, index) => {
        const end = points[index + 1];
        const styles = new Set(ranges
            .filter(range => range.start <= lineStart + start && range.end >= lineStart + end)
            .map(range => range.style));
        return { text: characters.slice(start, end).join(""), styles };
    });
}

function editorFont(size, rich, styles = new Set()) {
    if (!rich) return plainEditorFont(size);
    const slant = styles.has("i") ? "italic " : "";
    const weight = styles.has("b") ? 700 : 400;
    return `${slant}${weight} ${size}px "RiceWM Sans", "RiceWM CJK", "RiceWM Document CJK", "RiceWM Document Emoji", sans-serif`;
}

function richBlockForLine(blocks, lineStart, lineEnd, styles) {
    return blocks.findLast(range =>
        styles.has(range.style) &&
        range.start <= lineStart && range.end >= lineEnd
    )?.style || null;
}

function richTextWidth(context, text, start, ranges, size) {
    let width = 0;
    for (const segment of richSegments(text, start, ranges)) {
        context.font = editorFont(size, true, segment.styles);
        width += context.measureText(segment.text).width;
    }
    return width;
}

export class RiceNativeEditorRenderer {
    constructor(context, onDamage = () => {}, onFault = () => {}) {
        this.context = context;
        this.onDamage = onDamage; this.onFault = onFault; this.fontWork = null; this.officeFontWork = new Set();
    }

    render(command, chrome) {
        const context = this.context;
        const { rect } = command;
        const size = Math.max(8, Number(command.sizePx) || 12);
        const lineHeight = Math.max(size + 2, Math.ceil(size * 1.35));
        const inset = 4;
        context.save();
        const presentation = parsePresentation(command.text);
        const fontWork = ensureEditorContentFonts(presentation.document, context.canvas?.ownerDocument);
        if (fontWork && !this.fontWork) {
            this.fontWork = fontWork;
            void fontWork.then(() => { this.fontWork = null; this.onDamage(); }, error => { this.onFault(error); });
        } else if (fontWork) void fontWork.catch(() => {});
        const officeFontWork = ensureOfficeFonts(parseOfficeLayout(presentation.language), context.canvas?.ownerDocument);
        if (officeFontWork) {
            this.officeFontWork.add(officeFontWork);
            void officeFontWork.then(() => { this.officeFontWork.delete(officeFontWork); this.onDamage(); }, error => { this.officeFontWork.delete(officeFontWork); this.onFault(error); });
        }
        if (paintOfficeEditor(context, command, presentation) || paintAttributedRichEditor(context, command, presentation)) { context.restore(); return; }
        if (presentation.forceDarkMode) {
            command = {
                ...command,
                color: [235, 238, 244, 255],
                forceDarkMode: true,
            };
            chrome = {
                ...chrome,
                active: [105, 170, 255, 255],
                body: [18, 20, 24, 255],
                border: [65, 72, 84, 255],
                control: [160, 170, 185, 255],
                close: [245, 185, 115, 255],
                selection: [60, 125, 220, 255],
            };
        }
        const formatting = richFormatting(presentation.language);
        const rich = formatting !== null;
        context.font = editorFont(size, rich);
        const logicalLines = [];
        let logicalStart = 0;
        for (const match of presentation.document.matchAll(/([^\r\n]*)(\r\n|\r|\n|$)/g)) {
            const line = match[1];
            logicalLines.push({ text: line, start: logicalStart });
            logicalStart += Array.from(line).length + Array.from(match[2]).length;
            if (!match[2]) break;
        }
        const digits = presentation.viewport?.digits ?? String(Math.max(
            1,
            presentation.lineNumberStart + logicalLines.length - 1,
        )).length;
        const gutterWidth = presentation.lineNumbers
            ? Math.ceil(context.measureText("0".repeat(digits + 1)).width) + 12
            : 0;
        const available = Math.max(
            1,
            rect.width - inset * 2 - gutterWidth,
        );
        const lines = [];
        const plain = !rich ? plainEditorLayout(context, presentation, rect, size) : null;
        if (plain) lines.push(...plain.lines);
        let orderedItem = 0;
        for (const [index, logical] of (plain ? [] : logicalLines).entries()) {
            const logicalEnd = logical.start + Array.from(logical.text).length;
            const structure = rich
                ? richBlockForLine(
                    formatting.blocks,
                    logical.start,
                    logicalEnd,
                    new Set(["Q", "B", "N"]),
                )
                : null;
            const alignment = rich
                ? richBlockForLine(
                    formatting.blocks,
                    logical.start,
                    logicalEnd,
                    new Set(["A", "C", "R", "J"]),
                )
                : null;
            orderedItem = structure === "N" ? orderedItem + 1 : 0;
            const marker = structure === "B"
                ? "•"
                : structure === "N"
                    ? `${orderedItem}.`
                    : structure === "Q"
                        ? "❝"
                        : "";
            context.font = editorFont(size, rich);
            const markerWidth = marker
                ? Math.ceil(context.measureText(marker).width + 8)
                : 0;
            const segments = wrapEditorLine(context,
                logical.text,
                Math.max(1, available - markerWidth),
                presentation.wrap,
            );
            for (const [segmentIndex, segment] of segments.entries()) {
                lines.push({
                    line: segment.text,
                    start: logical.start + segment.start,
                    end: logical.start + segment.end,
                    logicalLine: presentation.lineNumberStart + index,
                    showNumber: segmentIndex === 0,
                    marker: segmentIndex === 0 ? marker : "",
                    markerWidth,
                    alignment,
                    justify: alignment === "J" && segmentIndex < segments.length - 1,
                });
            }
        }
        const maximumLines = Math.max(
            1,
            Math.floor((rect.height - inset * 2) / lineHeight),
        );
        context.beginPath();
        context.rect(rect.x, rect.y, rect.width, rect.height);
        context.clip();
        context.fillStyle = rgbaColor(command.color);
        context.textAlign = "left";
        context.textBaseline = "top";
        if (gutterWidth > 0) {
            context.fillStyle = rgbaColor(
                chrome?.body || mixedColor(command.color, [0, 0, 0, 255], 0.1),
            );
            context.fillRect(rect.x, rect.y, gutterWidth, rect.height);
            context.fillStyle = rgbaColor(chrome?.border || command.color);
            context.fillRect(
                rect.x + gutterWidth - 1,
                rect.y,
                1,
                rect.height,
            );
        }
        const palette = syntaxPalette(presentation.syntaxStyle, command, chrome);
        const caret = Math.max(0, Math.min(
            Array.from(presentation.document).length,
            presentation.caret ?? 0,
        ));
        const anchor = Math.max(0, Math.min(
            Array.from(presentation.document).length,
            presentation.anchor ?? caret,
        ));
        const caretLine = plain?.caretLine ?? Math.max(0, lines.findIndex((line, index) =>
            caret >= line.start && (
                caret < line.end ||
                (caret === line.end && (
                    index === lines.length - 1 ||
                    lines[index + 1].start > caret
                ))
            )
        ));
        const firstLine = plain?.firstLine ?? Math.max(0, caretLine - maximumLines + 1);
        const pixelOffset = plain?.pixelOffset || 0;
        const visibleLines = lines.slice(firstLine, firstLine + (plain?.maximumLines ?? maximumLines));
        const selectionStart = Math.min(caret, anchor);
        const selectionEnd = Math.max(caret, anchor);
        const focusColor = caretColor(command, chrome);
        const backgroundColor = chrome?.body || [255, 255, 255, 255];
        const themedSelection = chrome?.selection || chrome?.active || focusColor;
        const selectionColor = contrastRatio(themedSelection, backgroundColor) >= 1.5
            ? themedSelection
            : focusColor;
        const baseTextX = rect.x + inset + gutterWidth;
        const lineTextX = line => {
            context.wordSpacing = "0px";
            if (line.justify) {
                const spaces = (line.line.match(/ /g) || []).length;
                const measured = richTextWidth(context, line.line, line.start, formatting.inline, size);
                if (spaces) context.wordSpacing = `${Math.max(0, available - line.markerWidth - measured) / spaces}px`;
            }
            if (!rich) return baseTextX;
            const contentWidth = Math.max(1, available - line.markerWidth);
            const measured = richTextWidth(
                context,
                line.line,
                line.start,
                formatting.inline,
                size,
            );
            const offset = line.alignment === "R"
                ? Math.max(0, contentWidth - measured)
                : line.alignment === "C"
                    ? Math.max(0, (contentWidth - measured) / 2)
                    : 0;
            return baseTextX + line.markerWidth + offset;
        };
        for (let index = 0; index < visibleLines.length; index += 1) {
            const line = visibleLines[index];
            const y = rect.y + inset + index * lineHeight - pixelOffset;
            if (
                command.caret &&
                presentation.highlightCurrentLine &&
                firstLine + index === caretLine
            ) {
                context.fillStyle = rgbaColor(focusColor);
                context.globalAlpha = 0.14;
                context.fillRect(
                    rect.x + gutterWidth,
                    y,
                    Math.max(1, rect.width - gutterWidth),
                    lineHeight,
                );
                context.globalAlpha = 1;
            }
            if (gutterWidth > 0 && line.showNumber) {
                // Rich inline state must never bleed into editor chrome. The
                // preceding visual line may have left the canvas font italic
                // or bold, so restore the normal monospace gutter face first.
                context.font = editorFont(size, false);
                context.fillStyle = rgbaColor(palette.comment);
                context.textAlign = "right";
                context.fillText(
                    String(line.logicalLine),
                    rect.x + gutterWidth - 7,
                    y,
                );
                context.textAlign = "left";
            }
            const textX = lineTextX(line);
            const selectedStart = Math.max(line.start, selectionStart);
            const selectedEnd = Math.min(line.end, selectionEnd);
            context.font = editorFont(size, rich);
            if (selectedStart < selectedEnd) {
                const characters = Array.from(line.line);
                const before = characters.slice(0, selectedStart - line.start).join("");
                const selected = characters.slice(
                    selectedStart - line.start,
                    selectedEnd - line.start,
                ).join("");
                const beforeWidth = rich
                    ? richTextWidth(
                        context,
                        before,
                        line.start,
                        formatting.inline,
                        size,
                    )
                    : context.measureText(before).width;
                const selectedWidth = rich
                    ? richTextWidth(
                        context,
                        selected,
                        selectedStart,
                        formatting.inline,
                        size,
                    )
                    : context.measureText(selected).width;
                context.fillStyle = rgbaColor(selectionColor);
                context.globalAlpha = 0.35;
                context.fillRect(
                    textX + beforeWidth,
                    y,
                    Math.max(2, selectedWidth),
                    lineHeight,
                );
                context.globalAlpha = 1;
            }
            let x = textX;
            if (rich) {
                if (line.marker) {
                    context.font = editorFont(size, true);
                    context.fillStyle = rgbaColor(palette.text);
                    context.fillText(line.marker, baseTextX, y);
                }
                for (const segment of richSegments(
                    line.line,
                    line.start,
                    formatting.inline,
                )) {
                    context.font = editorFont(size, true, segment.styles);
                    context.fillStyle = rgbaColor(palette.text);
                    context.fillText(segment.text, x, y);
                    if (segment.styles.has("b")) {
                        // Some bundled/system canvas font combinations do not
                        // synthesize a visibly heavier face from `700`.
                        // Reinforce the ink by a sub-pixel second pass while
                        // retaining the same measured layout and caret model.
                        const embolden = Math.max(0.6, Math.min(1, size / 16));
                        context.fillText(segment.text, x + embolden, y);
                    }
                    const width = context.measureText(segment.text).width;
                    context.lineWidth = Math.max(1, Math.round(size / 14));
                    context.strokeStyle = rgbaColor(palette.text);
                    if (segment.styles.has("u")) {
                        context.beginPath();
                        context.moveTo(x, y + size + 2);
                        context.lineTo(x + width, y + size + 2);
                        context.stroke();
                    }
                    if (segment.styles.has("s")) {
                        context.beginPath();
                        context.moveTo(x, y + Math.round(size * 0.6));
                        context.lineTo(x + width, y + Math.round(size * 0.6));
                        context.stroke();
                    }
                    x += width;
                }
            } else {
                for (const token of syntaxTokens(line.line, presentation.language)) {
                    context.fillStyle = rgbaColor(palette[token.kind]);
                    context.fillText(token.text, x, y);
                    x += context.measureText(token.text).width;
                }
            }
        }
        if (command.caret && presentation.viewport?.caretVisible !== false && caretLine >= firstLine && caretLine < firstLine + visibleLines.length) {
            const line = visibleLines[caretLine - firstLine] || visibleLines.at(-1);
            const beforeCaret = Array.from(line.line)
                .slice(0, Math.max(0, Math.min(line.end, caret) - line.start))
                .join("");
            context.font = editorFont(size, rich);
            const beforeCaretWidth = rich
                ? richTextWidth(
                    context,
                    beforeCaret,
                    line.start,
                    formatting.inline,
                    size,
                )
                : context.measureText(beforeCaret).width;
            const caretX = Math.min(
                rect.x + rect.width - inset - 2,
                lineTextX(line) + beforeCaretWidth,
            );
            const caretY =
                rect.y + inset + (caretLine - firstLine) * lineHeight - pixelOffset;
            context.fillStyle = rgbaColor(focusColor);
            context.fillRect(Math.max(rect.x, caretX), caretY, 2, lineHeight);
        }
        context.restore();
    }

    hit(command, x, y) {
        const presentation = parsePresentation(command.text);
        if (presentation.language.startsWith('rich') || presentation.language.startsWith('office')) return null;
        return plainEditorHit(this.context, presentation, command, x, y);
    }
}
