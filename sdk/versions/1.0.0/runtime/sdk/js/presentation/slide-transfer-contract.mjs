import {exact, failure} from './contract-values.mjs';
import {checkedStatus} from './status-contract.mjs';

export const SLIDE_CLIPBOARD_MIME='application/vnd.ricewm.presentation-slides+zip';
export const SLIDE_CLIPBOARD_BYTES=4*1024*1024;
const slide=value=>typeof value==='string'&&/^pptx:[1-9]\d{2,9}$/u.test(value)&&Number(value.slice(5))>=256&&Number(value.slice(5))<=2147483647;
const count=value=>Number.isSafeInteger(value)&&value>=1&&value<=32;
export const slideClipboardToken=value=>typeof value==='string'&&/^slides:sha256:[0-9a-f]{64}$/u.test(value);
export function slideClipboardArguments(args) {
    if(!exact(args,['slides'])||!Array.isArray(args.slides)||!count(args.slides.length)||!args.slides.every(slide)||new Set(args.slides).size!==args.slides.length)
        throw failure('INVALID_REQUEST','Choose 1 to 32 distinct PowerPoint slides');
    return {slides:[...args.slides]};
}
export function checkedPasteSlides(command) {
    if(!exact(command,['type','after','selection','count'])||command.type!=='paste-slides'||command.after!==null&&!slide(command.after)||!slideClipboardToken(command.selection)||!count(command.count))
        throw failure('INVALID_REQUEST','Invalid prepared slide paste');
    return {...command};
}
export function checkedSlideClipboardReply(reply,operation,expected) {
    checkedStatus(reply);
    const value=reply.result;
    if(reply.revision!==expected||!slideClipboardToken(value.selection)||!count(value.selectedSlideCount)||value.clipboardMime!==SLIDE_CLIPBOARD_MIME||value.clipboardVersion!==1||
        !['export-slides','prepare-slides'].includes(operation)||operation==='export-slides'&&(!Number.isSafeInteger(value.length)||value.length<1||value.length>SLIDE_CLIPBOARD_BYTES))
        throw failure('INVALID_RESPONSE','Invalid slide clipboard transfer');
    return reply;
}
