import { readPptxPackage } from './slide-package.mjs';
import { OpcPackage } from '../opc-package.mjs';
import { parseXmlSource } from '../xml-source.mjs';
import { readPptxSlideTheme, drawingmlColorReference } from './drawingml-theme.mjs';
import { validGlow } from './glow-contract.mjs';

const P = 'http://schemas.openxmlformats.org/presentationml/2006/main';
const SP = 'http://purl.oclc.org/ooxml/presentationml/main';
const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const SR = 'http://purl.oclc.org/ooxml/officeDocument/relationships';
const decoder = new TextDecoder('utf-8', { fatal: true });
const attr = (node, name) => node?.attributes.find(a => !a.namespaceURI && a.localName === name)?.value;
const fail = message => { throw TypeError('Shape glow: ' + message); };
function one(node, namespace, name) {
    const matches = node?.children.filter(n => n.namespaceURI === namespace && n.localName === name) ?? [];
    if (matches.length > 1) fail('ambiguous ' + name);
    return matches[0] ?? null;
}
function tree(root, p) { return one(one(root, p, 'cSld'), p, 'spTree'); }
function placeholder(node, p) {
    const nv = node.children.find(n => n.namespaceURI === p && /^nv(?:Sp|Pic|CxnSp|GraphicFrame)Pr$/u.test(n.localName));
    return one(one(nv, p, 'nvPr'), p, 'ph');
}
const masterKind = kind => kind === 'ctrTitle' ? 'title' : ['subTitle', 'obj', 'chart', 'tbl', 'clipArt', 'dgm', 'media', 'pic'].includes(kind) ? 'body' : kind;
function unique(items) { if (items.length > 1) fail('ambiguous placeholder'); return items[0]; }
function chain(node, owner, context) {
    const result = [node], p = context.p, ph = placeholder(node, p);
    if (!ph || owner.root.localName !== 'sld') return result;
    const index = attr(ph, 'idx') ?? '0';
    if (!/^\d{1,10}$/u.test(index) || Number(index) >= 0xffffffff) return result;
    const layout = context.owners.find(v => v.root.localName === 'sldLayout');
    const match = unique((tree(layout?.root, p)?.children ?? []).filter(n => {
        const value = placeholder(n, p); return value && Number(attr(value, 'idx') ?? '0') === Number(index);
    }));
    if (!match) return result;
    result.push(match);
    const kind = attr(placeholder(match, p), 'type') ?? 'obj';
    const master = context.owners.find(v => v.root.localName === 'sldMaster');
    const inherited = unique((tree(master?.root, p)?.children ?? []).filter(n => {
        const value = placeholder(n, p); return value && (attr(value, 'type') ?? 'obj') === masterKind(kind);
    }));
    if (inherited) result.push(inherited);
    return result;
}
function resolve(nodes, context) {
    const { a, p } = context;
    const reference = nodes.map(n => one(one(n, p, 'style'), a, 'effectRef')).find(Boolean);
    let list;
    for (const node of nodes) {
        const props = one(node, p, node.localName === 'grpSp' ? 'grpSpPr' : 'spPr');
        if (['scene3d', 'sp3d', 'effectDag'].some(name => one(props, a, name))) return null;
        list = one(props, a, 'effectLst'); if (list) break;
    }
    if (!list && reference) {
        const index = attr(reference, 'idx');
        if (!/^\d{1,4}$/u.test(index ?? '') || Number(index) === 0) return null;
        const style = one(context.scheme('fmtScheme'), a, 'effectStyleLst')?.children[Number(index) - 1];
        if (!style || style.namespaceURI !== a || style.localName !== 'effectStyle' ||
            style.children.some(n => n.namespaceURI !== a || n.localName !== 'effectLst')) return null;
        list = one(style, a, 'effectLst');
    }
    if (!list || list.attributes.some(v => v.namespaceURI !== 'http://www.w3.org/2000/xmlns/') || list.children.length !== 1) return null;
    const glow = list.children[0];
    if (glow.namespaceURI !== a || glow.localName !== 'glow' || glow.children.length !== 1 ||
        glow.attributes.some(v => v.namespaceURI !== 'http://www.w3.org/2000/xmlns/' && (v.namespaceURI || v.localName !== 'rad'))) return null;
    const raw = attr(glow, 'rad') ?? '0'; if (!/^\d{1,12}$/u.test(raw)) return null;
    const ph = reference?.children.length === 1 ? drawingmlColorReference(reference.children[0], a) : null;
    const paint = context.color(glow.children[0], ph);
    if (!paint) return null;
    const value = { color: paint.color, opacity: paint.opacity, radius: Number(raw) / 9525 };
    return validGlow(value) ? value : null;
}

/** Read-only derived effects, anchored in the same unsigned source graph as
 * theme edits. Explicit empty lists stop placeholder inheritance. Unsupported
 * combinations retain their original native diagnostic and source bytes. */
export function inspectPptxObjectGlow(input, slideId) {
    const parsed = readPptxPackage(input), opc = new OpcPackage(parsed.archive);
    const slide = parsed.result.slides.find(s => s.id === slideId); if (!slide) fail('unknown slide');
    const p = parseXmlSource(parsed.result.xml).root.namespaceURI, r = p === P ? R : SR;
    if (![P, SP].includes(p)) fail('unsupported presentation namespace');
    const rel = opc.relationships(parsed.path).find(v => v.id === slide.relationship);
    if (!rel?.part || rel.type !== r + '/slide' || opc.contentType(rel.part) !== 'application/vnd.openxmlformats-officedocument.presentationml.slide+xml') fail('invalid slide identity');
    const context = readPptxSlideTheme(opc, rel.part, p), result = new Map();
    for (const owner of context.owners) {
        const identities = new Set();
        function walk(node) {
            if (!node || node.namespaceURI !== p) return;
            if (['sp', 'pic', 'cxnSp', 'grpSp', 'graphicFrame'].includes(node.localName)) {
                const nv = node.children.find(n => n.namespaceURI === p && n.localName.startsWith('nv'));
                const id = attr(one(nv, p, 'cNvPr'), 'id');
                if (/^\d{1,10}$/u.test(id ?? '') && Number(id) <= 0xffffffff) {
                    const key = String(Number(id)); if (identities.has(key)) fail('ambiguous shape identity'); identities.add(key);
                    const prefix = owner.root.localName === 'sld' ? slideId : `${slideId}/source:${owner.part}`;
                    const glow = resolve(chain(node, owner, context), context);
                    if (glow) result.set(`${prefix}/shape:${key}`, glow);
                }
            }
            for (const child of node.children) if (child.localName === 'grpSp' || node.localName === 'spTree' || node.localName === 'grpSp') walk(child);
        }
        walk(tree(owner.root, p));
    }
    return result;
}
inspectPptxObjectGlow.controlFactory = Object.freeze({ topics: ['documents', 'drawing'],
    constructor: 'inspectPptxObjectGlow(bytes,slideId)', methods: 'Read pure whole-shape glow effects from immutable PowerPoint source, including placeholder chains, theme references, closest overrides and mapped colors. Returns stable native object IDs and resolved color/opacity/radius in slide pixels; never edits source.' });
