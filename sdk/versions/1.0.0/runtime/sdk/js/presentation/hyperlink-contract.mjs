import { exact, failure } from './contract-values.mjs';
const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
export function webLink(value) {
    if (typeof value !== 'string' || value.length > 8192 || !/^https?:\/\//iu.test(value) || /[\s\\\u0000-\u001f\u007f]/u.test(value)) return false;
    try { const url = new URL(value); return ['http:', 'https:'].includes(url.protocol) && Boolean(url.hostname) && !url.username && !url.password; }
    catch { return false; }
}
export const supportedLink = link => link?.target?.kind === 'url' ? webLink(link.target.value) :
    link?.target?.kind === 'slide' && id(link.target.value) && Number.isSafeInteger(link.slideIndex) && link.slideIndex >= 0 && link.slideIndex < 4096;
export function checkedHyperlinkCommand(command) {
    if (!exact(command, ['type','slide','object','run','target']) || command.type !== 'set-hyperlink' || !id(command.slide) || !id(command.object) ||
        !(command.run === null || id(command.run)) || !(command.target === null || exact(command.target, ['kind','value']) &&
        (command.target.kind === 'url' ? webLink(command.target.value) : command.target.kind === 'slide' && id(command.target.value)))) {
        throw failure('INVALID_REQUEST', 'Choose an HTTP/HTTPS address or a slide in this presentation.');
    }
    return { ...command, target: command.target && { ...command.target } };
}
export function sceneHyperlinks(object) {
    return (object.canEditHyperlink === undefined || typeof object.canEditHyperlink === 'boolean') &&
        (object.hyperlinks === undefined || Array.isArray(object.hyperlinks) && object.hyperlinks.length <= 4096 && object.hyperlinks.every(link =>
            link && (link.run === null || id(link.run)) && typeof link.text === 'string' && link.text.length <= 65536 &&
            link.target && typeof link.target.value === 'string' && link.target.value.length <= 8192 &&
            (['unsupported','url'].includes(link.target.kind) ? link.slideIndex === null : supportedLink(link))));
}
export const linkIdentity = link => link ? JSON.stringify([link.object,link.run,link.target]) : null;
