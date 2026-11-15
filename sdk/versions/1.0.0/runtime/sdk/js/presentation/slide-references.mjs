import { parseXmlSource, patchXmlSource, escapeXmlAttribute } from '../xml-source.mjs';
import { checkedSectionCommand, sectionId } from './section-contract.mjs';

const P = 'http://schemas.openxmlformats.org/presentationml/2006/main';
const STRICT_P = 'http://purl.oclc.org/ooxml/presentationml/main';
const R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const STRICT_R = 'http://purl.oclc.org/ooxml/officeDocument/relationships';
const SECTIONS = 'http://schemas.microsoft.com/office/powerpoint/2010/main';
const XMLNS = 'http://www.w3.org/2000/xmlns/';
const fail = message => { throw new TypeError(`Invalid presentation references: ${message}`); };
const attr = (node, name, namespaceURI = '') => node.attributes.find(a => a.localName === name && a.namespaceURI === namespaceURI)?.value;
const is = (node, namespace, name) => node?.namespaceURI === namespace && node.localName === name;
const children = (node, namespace, name) => node.children.filter(n => is(n, namespace, name));
function only(node, namespace, name, required = true) {
    const found = children(node, namespace, name);
    if (found.length > 1 || required && !found.length) fail(`ambiguous or missing ${name}`);
    return found[0];
}
function number(value, minimum, maximum, label) {
    if (typeof value !== 'string' || !/^(?:0|[1-9]\d*)$/u.test(value)) fail(label);
    const result = Number(value);
    if (!Number.isSafeInteger(result) || result < minimum || result > maximum) fail(label);
    return result;
}
const identity = value => `pptx:${number(value, 256, 2147483647, 'slide identity')}`;

function read(source) {
    const document = parseXmlSource(source), { root } = document, p = root.namespaceURI;
    if (![P, STRICT_P].includes(p) || root.localName !== 'presentation') fail('presentation root');
    const r = p === P ? R : STRICT_R, list = only(root, p, 'sldIdLst', false);
    const slides = list ? children(list, p, 'sldId').map(node => {
        const relationship = attr(node, 'id', r);
        if (typeof relationship !== 'string' || !relationship || relationship.length > 4096) fail('slide relationship');
        return { id: identity(attr(node, 'id')), relationship, node };
    }) : [];
    if (slides.length > 4096 || new Set(slides.map(s => s.id)).size !== slides.length || new Set(slides.map(s => s.relationship)).size !== slides.length) fail('duplicate slides or slide limit');
    const known = new Set(slides.map(s => s.id));
    const sectionLists = document.nodes.filter(node => node.localName === 'sectionLst');
    if (sectionLists.length > 1 || sectionLists.some(node => node.namespaceURI !== SECTIONS ||
        !is(node.parent, p, 'ext') || attr(node.parent, 'uri')?.toUpperCase() !== '{521415D9-36F7-43E2-AB2F-B90AF26B5E84}' ||
        !is(node.parent?.parent, p, 'extLst') || node.parent?.parent?.parent !== root)) fail('unsupported section extension');
    const memberships = new Map(), sectionIds = new Set();
    const sections = sectionLists.length ? children(sectionLists[0], SECTIONS, 'section').map(node => {
        const id = attr(node, 'id'), name = attr(node, 'name'), list = only(node, SECTIONS, 'sldIdLst');
        if (!/^\{[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}\}$/iu.test(id || '') || sectionIds.has(id.toUpperCase()) || typeof name !== 'string' || name.length > 65536) fail('section identity or name');
        sectionIds.add(id.toUpperCase());
        const members = children(list, SECTIONS, 'sldId').map(member => {
            const slide = identity(attr(member, 'id'));
            if (!known.has(slide) || memberships.has(slide)) fail('unknown or multiply sectioned slide');
            const value = { id: slide, node: member }; memberships.set(slide, { section: node, member: value }); return value;
        });
        return { id, name, node, list, members };
    }) : [];
    if (sections.length > 4096 || sections.length && memberships.size !== slides.length) fail('incomplete section membership');
    // Section membership is defined in presentation order. Do not silently
    // repair a malformed source or reinterpret a different section schema.
    const order = new Map(slides.map((slide, index) => [slide.id, index]));
    let previous = -1;
    for (const section of sections) for (const member of section.members) {
        const index = order.get(member.id);
        if (index <= previous) fail('non-contiguous or unordered section members');
        previous = index;
    }
    const customList = only(root, p, 'custShowLst', false), showIds = new Set();
    const customShows = customList ? children(customList, p, 'custShow').map(node => {
        const id = number(attr(node, 'id'), 0, 0xffffffff, 'custom show identity'), name = attr(node, 'name');
        if (showIds.has(id) || typeof name !== 'string' || name.length > 65536) fail('custom show identity or name');
        showIds.add(id); const list = only(node, p, 'sldLst');
        const byRelationship = new Map(slides.map(slide => [slide.relationship, slide.id]));
        const members = children(list, p, 'sld').map(member => {
            const slide = byRelationship.get(attr(member, 'id', r));
            if (!slide) fail('unknown custom show slide');
            return slide;
        });
        return { id, name, slides: members };
    }) : [];
    // Ranges are 1-based slide positions, not slide identities. Reordering
    // preserves their positions; custom shows preserve their own explicit order.
    const ranges = document.nodes.filter(node => is(node, p, 'sldRg')).map(node => {
        // These unsigned positions are retained presentation settings, not
        // references to existing IDs. The reference writer preserves them even
        // when deletion leaves fewer slides (including an empty presentation).
        const start = number(attr(node, 'st'), 1, 0xffffffff, 'range start');
        const end = number(attr(node, 'end'), start, 0xffffffff, 'range end');
        return { start, end };
    });
    return { document, list, slides, sections, memberships, customShows, ranges, sectionList:sectionLists[0]||null,
        requiresStructuralAdapter: Boolean(sectionLists.length || customList || ranges.length) };
}
function value(state, xml = state.document.source, changed = false) {
    const slides = Object.freeze(state.slides.map(({ id, relationship }) => Object.freeze({ id, relationship })));
    const sections = Object.freeze(state.sections.map(({ id, name, members }) => Object.freeze({ id, name, slides: Object.freeze(members.map(member => member.id)) })));
    const customShows = Object.freeze(state.customShows.map(show => Object.freeze({ ...show, slides: Object.freeze([...show.slides]) })));
    return Object.freeze({ xml, changed, slides, sections, customShows, requiresStructuralAdapter: state.requiresStructuralAdapter,
        ranges: Object.freeze(state.ranges.map(range => Object.freeze({ ...range }))) });
}

/** Inspect the original presentation part. This does not authorize edits to a
 * package: its owner must separately enforce signatures, relationships, source
 * identity, revision checks, atomic package replacement and unified history. */
export function inspectPptxSlideReferences(xml) { return value(read(xml)); }

function standalone(source, node, destination = null) {
    // A member can move between section-local namespace scopes. Preserve its
    // original markup and bind every inherited prefix on that member itself.
    const declared = new Set(node.attributes.filter(a => a.namespaceURI === XMLNS).map(a => a.name === 'xmlns' ? '' : a.localName));
    const bindings = Object.entries(node.namespaces).filter(([prefix,uri]) => prefix !== 'xml' && !declared.has(prefix) && (!destination || destination[prefix]!==uri))
        .map(([prefix, uri]) => ` xmlns${prefix ? `:${prefix}` : ''}="${escapeXmlAttribute(uri)}"`).join('');
    const at = node.start + 1 + node.name.length;
    return source.slice(node.start, at) + bindings + source.slice(at, node.end);
}

function appendXml(source,parent,content) {
    if(parent.selfClosing)return source.slice(parent.start,parent.openEnd-2)+'>'+content+`</${parent.name}>`;
    return source.slice(parent.start,parent.closeStart)+content+source.slice(parent.closeStart,parent.end);
}
function sectionXml(source,section,inherited) {
    const namespaces=section.list?.namespaces || {...inherited,rs:SECTIONS};
    const member=entry=>entry.node?standalone(source,entry.node,namespaces):`<rs:sldId xmlns:rs="${SECTIONS}" id="${entry.id.slice(5)}"/>`;
    if(!section.node)return `<rs:section xmlns:rs="${SECTIONS}" name="${escapeXmlAttribute(section.name)}" id="${section.id}"><rs:sldIdLst>${section.members.map(member).join('')}</rs:sldIdLst></rs:section>`;
    const original=source.slice(section.node.start,section.node.end),name=section.node.attributes.find(a=>a.localName==='name'&&a.namespaceURI==='');
    const edits=[];
    if(name.value!==section.name)edits.push({start:name.valueStart-section.node.start,end:name.valueEnd-section.node.start,text:escapeXmlAttribute(section.name)});
    const list=replaceChildren(source,section.list,section.list.children.filter(n=>is(n,SECTIONS,'sldId')),section.members,member);
    if(list!==source.slice(section.list.start,section.list.end))edits.push({start:section.list.start-section.node.start,end:section.list.end-section.node.start,text:list});
    return patchXmlSource(original,edits);
}

/** Stable-ID section authoring. New IDs are caller supplied, so a validated
 * candidate can be replayed without changing identities. Unknown markup on
 * retained sections stays byte-bound to its source. Removing sections retains
 * slides and merges membership exactly as the reference editor does. */
export function editPptxSectionReferences(xml,command) {
    command=checkedSectionCommand(command);
    const state=read(xml),sections=state.sections.map(s=>({...s,members:[...s.members]}));
    const locate=id=>{const index=sections.findIndex(s=>s.id.toUpperCase()===id.toUpperCase());if(index<0)fail('unknown section');return index;};
    if(command.type==='add-section') {
        const index=state.slides.findIndex(s=>s.id===command.at);if(index<0)fail('unknown section start slide');
        if(!sectionId(command.id)||sections.some(s=>s.id.toUpperCase()===command.id.toUpperCase()))fail('new section identity');
        const members=state.slides.map(s=>state.memberships.get(s.id)?.member||{id:s.id,node:null});
        if(!sections.length) {
            if(index) {
                if(!sectionId(command.defaultId)||command.defaultId.toUpperCase()===command.id.toUpperCase())fail('default section identity');
                sections.push({id:command.defaultId,name:'Default Section',node:null,list:null,members:members.slice(0,index)});
            }
            sections.push({id:command.id,name:command.name,node:null,list:null,members:members.slice(index)});
        } else {
            const owner=state.memberships.get(command.at).section,at=sections.findIndex(s=>s.node===owner),old=sections[at];
            const split=old.members.findIndex(m=>m.id===command.at),tail=old.members.splice(split);
            sections.splice(at+1,0,{id:command.id,name:command.name,node:null,list:null,members:tail});
            for(let i=sections.length-1;i>=0;i--)if(!sections[i].members.length&&sections[i].name==='Default Section')sections.splice(i,1);
        }
    } else if(command.type==='rename-section')sections[locate(command.section)].name=command.name;
    else if(command.type==='remove-all-sections')sections.length=0;
    else if(command.type==='remove-section') {
        if(command.slides&&sections[locate(command.section)].members.length)fail('remove section slides through the package transaction owner');
        const index=locate(command.section),[removed]=sections.splice(index,1);
        if(index>0)sections[index-1].members.push(...removed.members);
        else if(sections.length)sections[0].members.unshift(...removed.members);
    } else {
        const from=locate(command.section),target=command.before===null?sections.length:locate(command.before),to=target-Number(target>from);
        if(from===to)return value(state);
        const [section]=sections.splice(from,1);sections.splice(to,0,section);
    }
    const edits=[];
    if(command.type==='move-section'&&state.list) {
        const byId=new Map(state.slides.map(slide=>[slide.id,slide]));
        const order=sections.flatMap(s=>s.members.map(m=>byId.get(m.id)));
        edits.push({start:state.list.start,end:state.list.end,text:replaceChildren(xml,state.list,state.slides.map(s=>s.node),order,s=>xml.slice(s.node.start,s.node.end))});
    }
    if(state.sectionList) {
        if(!sections.length) {
            const parent=state.sectionList.parent;
            const remove=parent.children.length===1&&parent.attributes.every(a=>a.namespaceURI===XMLNS||a.localName==='uri'&&!a.namespaceURI)?parent:state.sectionList;
            edits.push({start:remove.start,end:remove.end,text:''});
        } else edits.push({start:state.sectionList.start,end:state.sectionList.end,text:replaceChildren(xml,state.sectionList,state.sections.map(s=>s.node),sections,s=>sectionXml(xml,s,state.sectionList.namespaces))});
    } else if(sections.length) {
        const root=state.document.root,p=root.namespaceURI;
        const list=`<rs:sectionLst xmlns:rs="${SECTIONS}">${sections.map(s=>sectionXml(xml,s,root.namespaces)).join('')}</rs:sectionLst>`;
        const extension=`<rp:ext xmlns:rp="${p}" uri="{521415D9-36F7-43E2-AB2F-B90AF26B5E84}">${list}</rp:ext>`;
        const ext=only(root,p,'extLst',false);
        if(ext&&ext.children.some(n=>is(n,p,'ext')&&attr(n,'uri')?.toUpperCase()==='{521415D9-36F7-43E2-AB2F-B90AF26B5E84}'))fail('ambiguous section extension');
        if(ext)edits.push({start:ext.start,end:ext.end,text:appendXml(xml,ext,extension)});
        else edits.push({start:root.start,end:root.end,text:appendXml(xml,root,`<rp:extLst xmlns:rp="${p}">${extension}</rp:extLst>`)});
    }
    const output=patchXmlSource(xml,edits);return value(read(output),output,output!==xml);
}
editPptxSectionReferences.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'editPptxSectionReferences(xml,command)',methods:'Source-preserving add-section (caller GUIDs), rename-section, remove-section (retain slides), remove-all-sections and move-section. Stable section IDs, unknown retained markup and source order are validated. Commit through the document transaction owner.'});
function replaceChildren(source, parent, old, ordered, render) {
    const values = ordered.map(render);
    if (parent.selfClosing) {
        if (!values.length) return source.slice(parent.start, parent.end);
        return source.slice(parent.start, parent.openEnd - 2) + '>' + values.join('') + `</${parent.name}>`;
    }
    const edits = old.map((node, index) => ({ start: node.start - parent.start, end: node.end - parent.start, text: values[index] || '' }));
    if (values.length > old.length) {
        const at = (old.at(-1)?.end ?? parent.openEnd) - parent.start;
        const text = values.slice(old.length).join('');
        // Fold an insertion into the last replacement at the same boundary.
        if (edits.length) edits.at(-1).text += text;
        else edits.push({ start: at, end: at, text });
    }
    return patchXmlSource(source.slice(parent.start, parent.end), edits);
}

/** Private candidate adapter for a native slide-part transaction. Original
 * metadata stays owned by this closure until it is restored and checked against
 * both the requested commands and the native-created identities. The detached
 * XML must never be exposed as the committed or saved presentation. */
export function preparePptxSlideStructure(xml) {
    const state=read(xml),p=state.document.root.namespaceURI;
    const custom=only(state.document.root,p,'custShowLst',false);
    const roots=[state.sectionList,custom,...state.document.nodes.filter(n=>is(n,p,'sldRg'))].filter(Boolean);
    const fragments=roots.filter(node=>!roots.some(other=>other!==node&&other.start<node.start&&other.end>node.end));
    let token;do {token='rice-slide-structure-'+crypto.randomUUID();}while(xml.includes(token));
    const markers=fragments.map((node,index)=>({node,marker:`<!--${token}:${index}-->`}));
    const detached=patchXmlSource(xml,markers.map(({node,marker})=>({start:node.start,end:node.end,text:marker})));
    // The native engine still applies its own original guard and package graph
    // policy. Only the understood reference metadata is handled by this adapter.
    if(read(detached).requiresStructuralAdapter)fail('unhandled structural metadata');
    return Object.freeze({xml:detached,restore(candidate,commands,createdSlides=[]) {
        const next=read(candidate);
        if(next.requiresStructuralAdapter)fail('unexpected candidate metadata');
        if(!Array.isArray(commands)||commands.length>32||!Array.isArray(createdSlides))fail('invalid structural transaction');
        const order=state.slides.map(s=>s.id),sections=state.sections.map(s=>({...s,members:[...s.members]}));
        const owner=id=>sections.find(s=>s.members.some(m=>m.id===id));
        const original=new Set(order),created=new Set();let cursor=0;
        const locate=id=>{const index=order.indexOf(id);if(index<0)fail('unknown structural slide');return index;};
        for(const command of commands) {
            if(command.type==='insert-slide'||command.type==='duplicate-slide') {
                const from=command.type==='duplicate-slide'?locate(command.slide):command.after===null?-1:locate(command.after);
                const at=from+1,id=createdSlides[cursor++];
                if(typeof id!=='string'||identity(id.slice(5))!==id||original.has(id)||created.has(id))fail('invalid created slide identity');
                created.add(id);
                const destination=command.type==='duplicate-slide'?owner(command.slide):from>=0?owner(order[from]):sections[0];
                order.splice(at,0,id);destination?.members.push({id,node:null});
            } else if(command.type==='delete-slide'||command.type==='delete-selected-slides') {
                const removed=command.type==='delete-slide'?[command.slide]:command.slides;
                if(!Array.isArray(removed)||!removed.length||new Set(removed).size!==removed.length)fail('invalid deleted slides');
                for(const id of removed){order.splice(locate(id),1);for(const section of sections)section.members=section.members.filter(m=>m.id!==id);}
            } else if(command.type==='move-slide'||command.type?.endsWith('-section')||command.type==='remove-all-sections')fail('unexpected package command in native group');
        }
        if(cursor!==createdSlides.length||order.length!==next.slides.length||order.some((id,index)=>id!==next.slides[index].id))fail('native slide order disagrees with structural transaction');
        const relationships=new Map(state.slides.map(s=>[s.id,s.relationship]));
        for(const slide of next.slides)if(relationships.has(slide.id)&&relationships.get(slide.id)!==slide.relationship)fail('retained slide relationship changed');
        const positions=new Map(order.map((id,index)=>[id,index]));
        for(const section of sections)section.members.sort((a,b)=>positions.get(a.id)-positions.get(b.id));
        const deletedRelationships=new Set(state.slides.filter(s=>!positions.has(s.id)).map(s=>s.relationship));
        const edits=markers.map(({node,marker})=>{
            const start=candidate.indexOf(marker);
            if(start<0||candidate.indexOf(marker,start+marker.length)>=0)fail('structural metadata anchor was lost or duplicated');
            const parent=next.document.nodes.filter(n=>n.openEnd<=start&&n.closeStart>=start+marker.length).at(-1);
            if(!parent||parent.localName!==node.parent.localName||parent.namespaceURI!==node.parent.namespaceURI||
                attr(parent,'uri')!==attr(node.parent,'uri')||Object.entries(node.parent.namespaces).some(([prefix,uri])=>parent.namespaces[prefix]!==uri))fail('structural metadata context changed');
            let text=xml.slice(node.start,node.end);
            if(node===state.sectionList)text=replaceChildren(xml,node,state.sections.map(s=>s.node),sections,s=>sectionXml(xml,s,node.namespaces));
            else if(node===custom&&deletedRelationships.size) {
                const removed=state.document.nodes.filter(n=>is(n,p,'sld')&&n.start>node.start&&n.end<node.end&&deletedRelationships.has(attr(n,'id',p===P?R:STRICT_R)));
                text=patchXmlSource(text,removed.map(n=>({start:n.start-node.start,end:n.end-node.start,text:''})));
            }
            return {start,end:start+marker.length,text};
        });
        const output=patchXmlSource(candidate,edits),result=read(output);
        if(output.includes(token)||result.sections.flatMap(s=>s.members).length!==(sections.length?order.length:0))fail('incomplete structural metadata restoration');
        return value(result,output,output!==xml);
    }});
}

/** Move a stable slide ID before another ID (null appends). Section transfer
 * follows the reference: join the preceding slide's section, or the first section
 * at the beginning. Preserve custom shows, positional ranges and unknown XML. */
export function movePptxSlideReferences(xml, { slide, before } = {}) {
    const state = read(xml), from = state.slides.findIndex(item => item.id === slide);
    const target = before === null ? state.slides.length : state.slides.findIndex(item => item.id === before);
    if (from < 0 || target < 0) fail('unknown move identity');
    const to = target - Number(target > from);
    if (to === from) return value(state);
    const order = [...state.slides], [moved] = order.splice(from, 1); order.splice(to, 0, moved);
    const edits = [{ start: state.list.start, end: state.list.end, text: replaceChildren(xml, state.list, state.slides.map(item => item.node), order, item => xml.slice(item.node.start, item.node.end)) }];
    if (state.sections.length) {
        const old = state.memberships.get(slide), preceding = to ? state.memberships.get(order[to - 1].id)?.section : null;
        const destination = state.sections.find(section => section.node === preceding) || state.sections[0];
        const positions = new Map(order.map((item, index) => [item.id, index]));
        for (const section of state.sections) {
            const members = section.members.filter(member => member.id !== slide);
            if (section === destination) members.push(old.member);
            members.sort((a, b) => positions.get(a.id) - positions.get(b.id));
            if (members.length === section.members.length && members.every((member, index) => member === section.members[index])) continue;
            edits.push({ start: section.list.start, end: section.list.end,
                text: replaceChildren(xml, section.list, section.members.map(member => member.node), members,
                    member => member === old.member && old.section !== section.node ? standalone(xml, member.node) : xml.slice(member.node.start, member.node.end)) });
        }
    }
    const output = patchXmlSource(xml, edits);
    // Prove the final references as a whole; never return a partially patched
    // section graph if a source extension cannot retain a coherent ordering.
    return value(read(output), output, true);
}

for (const helper of [inspectPptxSlideReferences, movePptxSlideReferences]) helper.controlFactory = Object.freeze({
    topics: ['documents', 'workflow'],
    constructor: helper === inspectPptxSlideReferences ? 'inspectPptxSlideReferences(presentationXml)' : 'movePptxSlideReferences(presentationXml,{slide:"pptx:256",before:null})',
    methods: 'Immutable XML/slide/section/custom-show/range result. Source-preserving metadata transform only; package signature, relationship, revision and unified Undo/Redo checks remain mandatory before commit.',
});
