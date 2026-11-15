// Bounded XML 1.0 source ranges. Edits preserve original prefixes, whitespace,
// comments and opaque extension markup; no DOM or entity resolver is required.
const XML = 'http://www.w3.org/XML/1998/namespace';
const XMLNS = 'http://www.w3.org/2000/xmlns/';
const NAME = /^[A-Z_a-z\u00c0-\u00d6\u00d8-\u00f6\u00f8-\u02ff\u0370-\u037d\u037f-\u1fff\u200c-\u200d\u2070-\u218f\u2c00-\u2fef\u3001-\ud7ff\uf900-\ufdcf\ufdf0-\ufffd\u{10000}-\u{effff}][A-Z_a-z0-9.\-\u00b7\u00c0-\u00d6\u00d8-\u00f6\u00f8-\u02ff\u0300-\u036f\u0370-\u037d\u037f-\u1fff\u200c-\u200d\u203f-\u2040\u2070-\u218f\u2c00-\u2fef\u3001-\ud7ff\uf900-\ufdcf\ufdf0-\ufffd\u{10000}-\u{effff}]*/u;
const SPACE = /[\x20\t\r\n]/u;
const validPoint = p => p === 9 || p === 10 || p === 13 || p >= 32 && p <= 0xd7ff || p >= 0xe000 && p <= 0xfffd || p >= 0x10000 && p <= 0x10ffff;
const invalid = message => { throw new TypeError(`Invalid XML: ${message}`); };
function characters(value) {
    for (const char of value) if (!validPoint(char.codePointAt(0))) invalid('prohibited character');
}
function entities(value) {
    const named = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };
    return value.replace(/&([^;]*);|&/gu, (whole, name) => {
        if (name !== undefined && Object.hasOwn(named, name)) return named[name];
        if (name !== undefined && /^(?:#\d+|#x[0-9a-fA-F]+)$/u.test(name)) {
            const point = name.startsWith('#x') ? Number.parseInt(name.slice(2), 16) : Number(name.slice(1));
            if (validPoint(point)) return String.fromCodePoint(point);
        }
        return invalid('unknown or malformed entity');
    });
}
export function escapeXmlAttribute(value) {
    if (typeof value !== 'string') throw new TypeError('Expected XML attribute text');
    characters(value);
    return value.replace(/[&<>"'\t\r\n]/gu, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;', '\t': '&#9;', '\r': '&#13;', '\n': '&#10;' })[c]);
}

export function parseXmlSource(source, { maximumBytes = 8 * 1024 * 1024, maximumNodes = 65536, maximumDepth = 128 } = {}) {
    if (typeof source !== 'string' || !Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 16 * 1024 * 1024 ||
        !Number.isSafeInteger(maximumNodes) || maximumNodes < 1 || maximumNodes > 262144 ||
        !Number.isSafeInteger(maximumDepth) || maximumDepth < 1 || maximumDepth > 256 || source.length > maximumBytes ||
        new TextEncoder().encode(source).length > maximumBytes) throw new RangeError('XML source exceeds its bounds');
    characters(source);
    let cursor = source.charCodeAt(0) === 0xfeff ? 1 : 0, root = null;
    const beginning = cursor, stack = [], nodes = [];
    const whitespace = () => { while (SPACE.test(source[cursor] || '')) cursor++; };
    function qname() {
        const first = NAME.exec(source.slice(cursor))?.[0];
        if (!first) return invalid('element or attribute name');
        cursor += first.length;
        let localName = first, prefix = '';
        if (source[cursor] === ':') {
            prefix = first; cursor++;
            localName = NAME.exec(source.slice(cursor))?.[0];
            if (!localName) return invalid('qualified name');
            cursor += localName.length;
        }
        return { name: prefix ? `${prefix}:${localName}` : localName, prefix, localName };
    }
    while (cursor < source.length) {
        if (source[cursor] !== '<') {
            const end = source.indexOf('<', cursor), stop = end < 0 ? source.length : end;
            const text = source.slice(cursor, stop);
            if (text.includes(']]>') || !stack.length && !/^[\x20\t\r\n]*$/u.test(text)) invalid('text outside root or CDATA terminator');
            entities(text); cursor = stop; continue;
        }
        if (source.startsWith('<!--', cursor)) {
            const end = source.indexOf('-->', cursor + 4);
            if (end < 0 || source.slice(cursor + 4, end).includes('--') || source[end - 1] === '-') invalid('comment');
            cursor = end + 3; continue;
        }
        if (source.startsWith('<![CDATA[', cursor)) {
            const end = source.indexOf(']]>', cursor + 9);
            if (!stack.length || end < 0) invalid('CDATA');
            cursor = end + 3; continue;
        }
        if (source.startsWith('<?', cursor)) {
            const start = cursor, end = source.indexOf('?>', cursor + 2); cursor += 2;
            const target = qname();
            if (end < cursor || target.prefix || cursor !== end && !SPACE.test(source[cursor])) invalid('processing instruction');
            if (target.name.toLowerCase() === 'xml') {
                if (start !== beginning || target.name !== 'xml' || !/^\s+version\s*=\s*(['"])1\.0\1(?:\s+encoding\s*=\s*(['"])(?:UTF-8|utf-8)\2)?(?:\s+standalone\s*=\s*(['"])(?:yes|no)\3)?\s*$/u.test(source.slice(cursor, end))) invalid('XML declaration');
            }
            cursor = end + 2; continue;
        }
        if (source.startsWith('<!', cursor)) invalid('DTD and entity declarations are prohibited');
        const start = cursor++;
        if (source[cursor] === '/') {
            cursor++; const name = qname(); whitespace();
            const node = stack.pop();
            if (!node || node.name !== name.name || source[cursor++] !== '>') invalid('unmatched closing tag');
            node.closeStart = start; node.end = cursor; continue;
        }
        const name = qname(), attributes = [], names = new Set();
        while (true) {
            const prior = cursor; whitespace();
            if (source[cursor] === '/' || source[cursor] === '>') break;
            if (cursor === prior || attributes.length >= 1024) invalid('attribute spacing or count');
            const attributeStart = cursor, attribute = qname(); whitespace();
            if (source[cursor++] !== '=') invalid('attribute assignment');
            whitespace(); const quote = source[cursor++], valueStart = cursor;
            if (quote !== '"' && quote !== "'") invalid('attribute quote');
            const end = source.indexOf(quote, cursor);
            if (end < 0 || source.slice(cursor, end).includes('<') || names.has(attribute.name)) invalid('attribute value or duplicate name');
            const value = entities(source.slice(cursor, end).replace(/\r\n|[\t\r\n]/gu, ' '));
            cursor = end + 1; names.add(attribute.name);
            attributes.push({ ...attribute, value, start: attributeStart, valueStart, valueEnd: end, end: cursor, quote });
        }
        const selfClosing = source[cursor] === '/';
        if (selfClosing) cursor++;
        if (source[cursor++] !== '>') invalid('opening tag');
        const parent = stack.at(-1) || null, namespaces = Object.assign(Object.create(null), parent?.namespaces || { xml: XML });
        for (const attribute of attributes) {
            if (attribute.name !== 'xmlns' && attribute.prefix !== 'xmlns') continue;
            const prefix = attribute.name === 'xmlns' ? '' : attribute.localName, uri = attribute.value;
            if (prefix === 'xmlns' || uri === XMLNS || prefix === 'xml' && uri !== XML || prefix !== 'xml' && uri === XML || prefix && !uri) invalid('namespace declaration');
            namespaces[prefix] = uri;
        }
        if (Object.keys(namespaces).length > 256) invalid('namespace count');
        const resolve = (prefix, attribute) => prefix ? namespaces[prefix] || invalid('unbound namespace prefix') : attribute ? '' : namespaces[''] || '';
        const expanded = new Set();
        for (const attribute of attributes) {
            attribute.namespaceURI = attribute.name === 'xmlns' || attribute.prefix === 'xmlns' ? XMLNS : resolve(attribute.prefix, true);
            const key = JSON.stringify([attribute.namespaceURI, attribute.localName]);
            if (expanded.has(key)) invalid('duplicate expanded attribute');
            expanded.add(key); Object.freeze(attribute);
        }
        const node = { ...name, namespaceURI: resolve(name.prefix, false), start, openEnd: cursor,
            closeStart: selfClosing ? cursor : null, end: selfClosing ? cursor : null,
            selfClosing, attributes: Object.freeze(attributes), namespaces: Object.freeze(namespaces), parent, children: [] };
        if (parent) parent.children.push(node);
        else if (root) invalid('multiple root elements');
        else root = node;
        nodes.push(node);
        if (nodes.length > maximumNodes || stack.length + 1 > maximumDepth) invalid('element count or depth');
        if (!selfClosing) stack.push(node);
    }
    if (!root || stack.length) invalid('missing or unclosed root');
    for (const node of nodes) { Object.freeze(node.children); Object.freeze(node); }
    return Object.freeze({ source, root, nodes: Object.freeze(nodes) });
}

export function patchXmlSource(source, edits, { maximumBytes = 8 * 1024 * 1024 } = {}) {
    if (typeof source !== 'string' || !Array.isArray(edits) || edits.length > 65536 || !Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 16 * 1024 * 1024) throw new TypeError('Invalid XML patch');
    const ordered = edits.map(edit => {
        if (!edit || !Number.isSafeInteger(edit.start) || !Number.isSafeInteger(edit.end) || edit.start < 0 || edit.end < edit.start || edit.end > source.length || typeof edit.text !== 'string') throw new TypeError('Invalid XML patch range');
        return { start: edit.start, end: edit.end, text: edit.text };
    }).sort((a, b) => a.start - b.start || a.end - b.end);
    let end = 0, previous = null, size = source.length;
    const pieces = [];
    for (const edit of ordered) {
        if (edit.start < end || previous && previous.start === edit.start) throw new TypeError('Overlapping XML patches');
        size += edit.text.length - (edit.end - edit.start);
        pieces.push(source.slice(end, edit.start), edit.text); end = edit.end; previous = edit;
    }
    if (size > maximumBytes) throw new RangeError('XML patch exceeds its bounds');
    pieces.push(source.slice(end)); const result = pieces.join('');
    if (new TextEncoder().encode(result).length > maximumBytes) throw new RangeError('XML patch exceeds its bounds');
    return result;
}
