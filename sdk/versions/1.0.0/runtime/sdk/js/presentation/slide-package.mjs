import { ZipParts } from '../zip-parts.mjs';
import { parseXmlSource } from '../xml-source.mjs';
import { inspectPptxSlideReferences, movePptxSlideReferences, editPptxSectionReferences, preparePptxSlideStructure } from './slide-references.mjs';
import { checkedSectionCommand, SECTION_COMMANDS } from './section-contract.mjs';

const REL = 'http://schemas.openxmlformats.org/package/2006/relationships';
const TYPES = 'http://schemas.openxmlformats.org/package/2006/content-types';
const OFFICE = new Set(['http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument', 'http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument']);
const MIME = new Set(['presentation','template','slideshow'].map(kind => `application/vnd.openxmlformats-officedocument.presentationml.${kind}.main+xml`));
const decoder = new TextDecoder('utf-8', { fatal: true }), encoder = new TextEncoder();
const attr = (node, name) => node.attributes.find(a => a.localName === name && a.namespaceURI === '')?.value;
const fail = message => { throw new TypeError(`Presentation package: ${message}`); };
function part(target) {
    if (typeof target !== 'string' || !target || target.length > 4096 || /[\\?#\x00-\x20\x7f]/u.test(target) || /^[a-z][a-z0-9+.-]*:/iu.test(target)) fail('invalid document part target');
    const segments = [];
    for (const raw of target.replace(/^\//u, '').split('/')) {
        const segment = decodeURIComponent(raw);
        if (!segment || /[\\/\x00-\x1f\x7f]/u.test(segment)) fail('invalid part segment');
        if (segment === '.') continue;
        if (segment === '..') { if (!segments.length) fail('part target escapes package'); segments.pop(); }
        else segments.push(segment);
    }
    if (!segments.length) fail('empty document part target');
    return segments.join('/');
}

function readPackage(input) {
    const archive = new ZipParts(input, { maximumBytes: 8 * 1024 * 1024, maximumPartBytes: 16 * 1024 * 1024, maximumExpandedBytes: 64 * 1024 * 1024, maximumEntries: 16384 });
    if (archive.names.some(name => name.toLowerCase().startsWith('_xmlsignatures/'))) fail('signed presentations require a signature-aware workflow');
    if (!archive.has('_rels/.rels') || !archive.has('[Content_Types].xml')) fail('missing document identity');
    const relationships = parseXmlSource(decoder.decode(archive.read('_rels/.rels'))).root;
    if (relationships.namespaceURI !== REL || relationships.localName !== 'Relationships') fail('invalid root relationships');
    const all = relationships.children.filter(node => node.namespaceURI === REL && node.localName === 'Relationship');
    if (all.some(node => attr(node, 'Type')?.includes('/digital-signature/'))) fail('signed presentations require a signature-aware workflow');
    const office = all.filter(node => OFFICE.has(attr(node, 'Type')));
    if (office.length !== 1 || attr(office[0], 'TargetMode') !== undefined && attr(office[0], 'TargetMode') !== 'Internal') fail('ambiguous or external document relationship');
    const path = part(attr(office[0], 'Target'));
    if (!archive.has(path)) fail('missing presentation part');
    const types = parseXmlSource(decoder.decode(archive.read('[Content_Types].xml'))).root;
    if (types.namespaceURI !== TYPES || types.localName !== 'Types') fail('invalid content types');
    const matching = types.children.filter(node => node.namespaceURI === TYPES && node.localName === 'Override' && part(attr(node, 'PartName')) === path);
    if (matching.length !== 1 || !MIME.has(attr(matching[0], 'ContentType'))) fail('unsupported presentation content type');
    return { archive, path, result: inspectPptxSlideReferences(decoder.decode(archive.read(path))) };
}
// Shared codec internals use the same signed-source/document identity policy.
export {readPackage as readPptxPackage};
export function inspectPptxPackageSlides(input) { return readPackage(input).result; }

// Internal native-candidate lifecycle. Signature/source identity is verified on
// both sides. The caller must discard detached bytes on any failure and import
// the fully restored package before committing history or exposing a save.
export function preparePptxPackageStructure(input) {
    const original=readPackage(input),structure=preparePptxSlideStructure(original.result.xml);
    return Object.freeze({bytes:original.archive.replace(new Map([[original.path,encoder.encode(structure.xml)]])),restore(output,commands,createdSlides) {
        const candidate=readPackage(output);
        if(candidate.path!==original.path)fail('native candidate changed document identity');
        const result=structure.restore(candidate.result.xml,commands,createdSlides);
        const bytes=candidate.archive.replace(new Map([[candidate.path,encoder.encode(result.xml)]]));
        return Object.freeze({...result,bytes});
    }});
}

/** Produce a candidate package, retaining original resources and signature
 * policy. A document owner must still import this candidate successfully and
 * commit it under the source revision with its existing unified history. */
export function movePptxPackageSlides(input, commands) {
    if (!Array.isArray(commands) || !commands.length || commands.length > 128 || commands.some(command =>
        !command || Object.keys(command).length !== 2 || !Object.hasOwn(command, 'slide') || !Object.hasOwn(command, 'before') ||
        typeof command.slide !== 'string' || command.slide.length > 4096 || command.before !== null && (typeof command.before !== 'string' || command.before.length > 4096))) throw new TypeError('Invalid slide-move transaction');
    return editPptxPackageReferences(input,commands.map(command=>({type:'move-slide',...command})));
}

export function editPptxPackageReferences(input,commands) {
    if(!Array.isArray(commands)||!commands.length||commands.length>128)throw new TypeError('Invalid presentation reference transaction');
    commands=commands.map(command=>{
        if(SECTION_COMMANDS.has(command?.type))return checkedSectionCommand(command);
        if(command?.type!=='move-slide'||Object.keys(command).length!==3||typeof command.slide!=='string'||command.slide.length>4096||command.before!==null&&(typeof command.before!=='string'||command.before.length>4096))throw new TypeError('Invalid slide reference command');
        return {...command};
    });
    let { archive, path, result } = readPackage(input);
    const original = result.xml;
    for (const command of commands) result = command.type==='move-slide'?movePptxSlideReferences(result.xml,command):editPptxSectionReferences(result.xml,command);
    const changed = result.xml !== original;
    const bytes = archive.replace(changed ? new Map([[path, encoder.encode(result.xml)]]) : new Map());
    return Object.freeze({ ...result, bytes, changed });
}
editPptxPackageReferences.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'editPptxPackageReferences(bytes,commands)',methods:'Atomic candidate for slide moves and stable-ID section add/rename/remove/move commands. Retains all other package parts and signature policy. Validate/import and commit through one document history.'});
inspectPptxPackageSlides.controlFactory = Object.freeze({ topics: ['documents', 'workflow'],
    constructor: 'inspectPptxPackageSlides(bytes)', methods: 'Resolve an unsigned PPTX/POTX/PPSX main part and inspect immutable slide/section references. This read does not acknowledge an edit or saved file.' });
movePptxPackageSlides.controlFactory = Object.freeze({ topics: ['documents', 'workflow'],
    constructor: 'movePptxPackageSlides(bytes,[{slide:"pptx:256",before:null}])',
    methods: 'Bounded source-preserving PPTX/POTX/PPSX move candidate. Maintains sections, custom shows and positional ranges; refuses signed packages. Caller must validate candidate import and commit one revision-checked transaction in the existing document history.' });
