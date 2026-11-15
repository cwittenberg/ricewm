import { exact, failure } from './contract-values.mjs';
import { checkedStatus } from './status-contract.mjs';

export const SECTION_COMMANDS = new Set(['add-section','rename-section','remove-section','remove-all-sections','move-section']);
export const sectionId = value => typeof value==='string' && /^\{[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}\}$/iu.test(value);
const slideId = value => typeof value==='string' && /^pptx:[1-9]\d{2,9}$/u.test(value) && Number(value.slice(5))>=256 && Number(value.slice(5))<=2147483647;
const text = value => typeof value==='string' && new TextEncoder().encode(value).length<=16384 && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\ufffe\uffff\ud800-\udfff]/u.test(value);
export function checkedSectionCommand(command) {
    const invalid=()=>{throw failure('INVALID_REQUEST','Invalid section command');};
    if(!SECTION_COMMANDS.has(command?.type))invalid();
    if(command.type==='add-section') {
        if(!exact(command,['type','at','name','id','defaultId'],['type','at','name']) || !slideId(command.at) || !text(command.name) || command.id!==undefined&&!sectionId(command.id) || command.defaultId!==undefined&&!sectionId(command.defaultId))invalid();
    } else if(command.type==='rename-section') {
        if(!exact(command,['type','section','name']) || !sectionId(command.section) || !text(command.name))invalid();
    } else if(command.type==='remove-all-sections') {
        if(!exact(command,['type']))invalid();
    } else if(command.type==='move-section') {
        if(!exact(command,['type','section','before']) || !sectionId(command.section) || command.before!==null&&!sectionId(command.before))invalid();
    } else if(!exact(command,['type','section','slides'],['type','section']) || !sectionId(command.section) || command.slides!==undefined&&typeof command.slides!=='boolean')invalid();
    return {...command};
}
export function sectionArguments(args) {
    if(!exact(args,['startSection','limit'],[]))throw failure('INVALID_REQUEST','Invalid section page');
    const result={startSection:args.startSection??0,limit:args.limit??32};
    if(!Number.isSafeInteger(result.startSection)||result.startSection<0||result.startSection>4096||!Number.isSafeInteger(result.limit)||result.limit<1||result.limit>32)throw failure('INVALID_REQUEST','Invalid bounded section page');
    return result;
}
export function sectionNameArguments(args) {
    if(!exact(args,['section'])||!sectionId(args.section))throw failure('INVALID_REQUEST','Invalid section identity');
    return {...args};
}
export function checkedSectionReply(reply,args) {
    checkedStatus(reply);const r=reply.result;
    if(!Number.isInteger(r.sectionCount)||r.sectionCount<0||r.sectionCount>4096||r.startSection!==Math.min(args.startSection,r.sectionCount)||!Array.isArray(r.sections)||r.sections.length!==Math.min(args.limit,r.sectionCount-r.startSection)||r.nextSection!==(r.startSection+r.sections.length<r.sectionCount?r.startSection+r.sections.length:null))throw failure('INVALID_RESPONSE','Invalid section page');
    const ids=new Set();let previous=-1;
    for(const [offset,s]of r.sections.entries()) {
        if(!exact(s,['index','id','name','nameTruncated','startSlide','slideCount'])||s.index!==r.startSection+offset||!sectionId(s.id)||ids.has(s.id.toUpperCase())||!text(s.name)||[...s.name].length>256||typeof s.nameTruncated!=='boolean'||!Number.isInteger(s.startSlide)||s.startSlide<0||(offset>0?s.startSlide!==previous:r.startSection===0&&s.startSlide!==0)||!Number.isInteger(s.slideCount)||s.slideCount<0||s.startSlide+s.slideCount>r.slideCount)throw failure('INVALID_RESPONSE','Invalid section record');
        previous=s.startSlide+s.slideCount;ids.add(s.id.toUpperCase());
    }
    if(r.sections.length&&r.nextSection===null&&previous!==r.slideCount)throw failure('INVALID_RESPONSE','Incomplete section coverage');
    return reply;
}
export function checkedSectionNameReply(reply,args) {
    checkedStatus(reply);
    if(reply.result.section!==args.section||typeof reply.result.name!=='string'||new TextEncoder().encode(reply.result.name).length>48000)throw failure('INVALID_RESPONSE','Invalid section name');
    return reply;
}
