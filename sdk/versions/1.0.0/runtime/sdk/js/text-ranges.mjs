const encoder = new TextEncoder();
const segmenter = (() => {
    try {
        if(typeof globalThis.Intl?.Segmenter!=='function')return null;
        const value=new Intl.Segmenter(undefined,{granularity:'grapheme'});
        for(const _ of value.segment('')){}
        return value;
    }
    catch { return null; }
})();

/** Unicode scalar positions belong to native controls; document ranges use
 * UTF-8 bytes. Neither position may split a grapheme during a source edit. */
export function textGraphemeEdges(value) {
    if (typeof value !== 'string') throw TypeError('Expected text');
    const edges = [0]; let offset = 0;
    if (segmenter) {
        for (const part of segmenter.segment(value)) { offset += Array.from(part.segment).length; edges.push(offset); }
        return edges;
    }
    const chars = Array.from(value); let regional = 0;
    for (let i = 0; i < chars.length; i++) {
        const c = chars[i], previous = chars[i-1], ri = /\p{Regional_Indicator}/u.test(c);
        const joined = i > 0 && (/\p{Mark}|[\uFE00-\uFE0F\u{E0100}-\u{E01EF}\u{1F3FB}-\u{1F3FF}]/u.test(c) ||
            c === '\u200D' || previous === '\u200D' || c === '\n' && previous === '\r' || ri && regional % 2 === 1);
        if (i && !joined) edges.push(i);
        regional = ri ? regional + 1 : 0;
    }
    if (chars.length) edges.push(chars.length);
    return edges;
}

export function textRangeIndex(value) {
    if (typeof value !== 'string' || value.length > 262144) throw RangeError('Text range exceeds its bound');
    const scalarToByte = [0]; let size = 0;
    for (const character of value) {
        const point = character.codePointAt(0);
        if (point >= 0xd800 && point <= 0xdfff) throw TypeError('Text contains an unpaired surrogate');
        size += encoder.encode(character).length; scalarToByte.push(size);
    }
    const byteToScalar = new Map(scalarToByte.map((at, scalar) => [at, scalar]));
    const graphemes = new Set(textGraphemeEdges(value));
    const byte = scalar => {
        if (!Number.isSafeInteger(scalar) || !graphemes.has(scalar)) throw RangeError('Selection must use grapheme boundaries');
        return scalarToByte[scalar];
    };
    const scalar = at => {
        const result = byteToScalar.get(at);
        if (!Number.isSafeInteger(at) || result === undefined || !graphemes.has(result)) throw RangeError('Text range must use UTF-8 grapheme boundaries');
        return result;
    };
    return Object.freeze({byteLength:size, scalarLength:scalarToByte.length-1, byte, scalar});
}

export function textRangeFromSelection(value, selection) {
    const index = textRangeIndex(value), anchor = index.byte(selection?.anchor), caret = index.byte(selection?.caret);
    return Object.freeze({start:Math.min(anchor,caret),end:Math.max(anchor,caret)});
}

export function textSelectionFromRange(value, range) {
    if (!range || range.start > range.end) throw RangeError('Invalid text range');
    const index = textRangeIndex(value);
    return Object.freeze({anchor:index.scalar(range.start),caret:index.scalar(range.end)});
}
