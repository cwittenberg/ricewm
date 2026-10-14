// Mail is a live document. The native scene supplies geometry and inert HTML,
// never a bitmap; the browser owns text selection, accessibility and scrolling.
const decodedDocuments = new WeakMap();
export function mailDocumentHtml(scene) {
    if (scene?.displayList && decodedDocuments.has(scene.displayList)) return decodedDocuments.get(scene.displayList);
    const bytes = Array.isArray(scene?.displayList) ? Uint8Array.from(scene.displayList) : scene?.displayList;
    if (!(bytes instanceof Uint8Array) || bytes.length < 12 || bytes[0] !== 28) return null;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const length = view.getUint32(8, true);
    if (bytes[1] || view.getUint16(2, true) || view.getUint32(4, true) !== bytes.length ||
        length > 512 * 1024 || ((12 + length + 3) & ~3) !== bytes.length) return null;
    try {
        const html = new TextDecoder('utf-8', { fatal: true }).decode(bytes.subarray(12, 12 + length));
        decodedDocuments.set(scene.displayList, html);
        return html;
    }
    catch { return null; }
}

function inertDocument(document, source) {
    const policy = "default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; font-src 'none'; connect-src 'none'; form-action 'none'; base-uri 'none'";
    // Template fragment parsing is inert even for image URLs. DOMParser's
    // detached documents can start requests before a CSP meta is applied.
    const template = document.createElement('template');
    template.innerHTML = source;
    const parsed = document.implementation.createHTMLDocument('Message');
    const bodyTag = source.match(/<body\b(?:[^>"']|"[^"]*"|'[^']*')*>/i)?.[0];
    if (bodyTag) {
        const attributes = document.createElement('template');
        attributes.innerHTML = '<div' + bodyTag.slice(5) + '</div>';
        for (const attribute of attributes.content.firstElementChild?.attributes || []) {
            if (['class','id','style','dir','lang','bgcolor','text'].includes(attribute.name)) parsed.body.setAttribute(attribute.name, attribute.value);
        }
    }
    // Sanitize while still inside the inert template, before moving nodes.
    const fragment = template.content;
    fragment.querySelectorAll('script,iframe,frame,frameset,object,embed,base,meta,link').forEach(node => node.remove());
    for (const node of fragment.querySelectorAll('*')) {
        for (const attribute of [...node.attributes]) {
            if (/^on/i.test(attribute.name) || ['srcdoc','action','formaction','ping','autofocus','shadowrootmode'].includes(attribute.name)) node.removeAttribute(attribute.name);
        }
    }
    parsed.body.append(fragment);
    const csp = parsed.createElement('meta');
    csp.httpEquiv = 'Content-Security-Policy';
    csp.content = policy;
    parsed.head.prepend(csp);
    const style = parsed.createElement('style');
    style.textContent = 'html{min-height:100%;}body{min-height:100%;overflow-wrap:anywhere}pre{white-space:pre-wrap}img{max-width:100%;height:auto}table{max-width:100%}';
    parsed.head.append(style);
    return '<!doctype html>' + parsed.documentElement.outerHTML;
}
const intersect = (a, b) => ({ x: Math.max(a.x, b.x), y: Math.max(a.y, b.y),
    width: Math.max(0, Math.min(a.x+a.width,b.x+b.width)-Math.max(a.x,b.x)),
    height: Math.max(0, Math.min(a.y+a.height,b.y+b.height)-Math.max(a.y,b.y)) });
const subtract = (regions, cut) => regions.flatMap(r => {
    const c = intersect(r, cut);
    if (!c.width || !c.height) return [r];
    return [
        { x:r.x,y:r.y,width:r.width,height:c.y-r.y },
        { x:r.x,y:c.y+c.height,width:r.width,height:r.y+r.height-c.y-c.height },
        { x:r.x,y:c.y,width:c.x-r.x,height:c.height },
        { x:c.x+c.width,y:c.y,width:r.x+r.width-c.x-c.width,height:c.height },
    ].filter(piece => piece.width > 0 && piece.height > 0);
});
const polygon = r => `M${r.x} ${r.y}h${r.width}v${r.height}h${-r.width}Z`;

export class RiceMailDocumentSurfaces {
    constructor({ layer, canvas, onNavigate = () => {}, onFocus = () => {}, onOpenMessage = () => {}, onLayout = () => {}, onScroll = () => {} }) {
        Object.assign(this, { layer, canvas, onNavigate, onFocus, onOpenMessage, onLayout, onScroll });
        this.records = new Map();
    }
    setTheme(theme) {
        this.theme = theme;
        for (const record of this.records.values()) this.applyTheme(record);
    }
    applyTheme(record) {
        const doc = record.iframe.contentDocument;
        if (!doc?.body || !doc.querySelector('template[data-rice-mail-appearance="dark"],template[data-rice-mail-follow-theme]')) return;
        const color = (packed, fallback) => Number.isInteger(packed) ? '#' + (packed & 0xffffff).toString(16).padStart(6,'0') : fallback;
        doc.documentElement.style.setProperty('--rice-mail-background',color(this.theme?.menu?.[0],'#342f38'));
        doc.documentElement.style.setProperty('--rice-mail-text',color(this.theme?.menu?.[5],'#eeeeee'));
        if (doc.getElementById('rice-mail-theme')) return;
        const neutral = value => {
            const parts = value.match(/[\d.]+/g)?.map(Number);
            return parts?.length >= 3 && (parts.length < 4 || parts[3] > 0) && Math.max(...parts.slice(0,3))-Math.min(...parts.slice(0,3)) < 24;
        };
        // Adapt neutral sender canvases and text; retain colored cards, links,
        // images, borders and the original layout. Never invert photographs.
        for (const node of doc.body.querySelectorAll('*')) {
            if (node.closest('svg,template,style')) continue;
            const computed = doc.defaultView.getComputedStyle(node);
            if (neutral(computed.backgroundColor)) node.setAttribute('data-rice-neutral-background','');
            if (neutral(computed.color) && !node.closest('a')) node.setAttribute('data-rice-neutral-text','');
        }
        const style = doc.createElement('style'); style.id='rice-mail-theme';
        style.textContent='html,body{background:var(--rice-mail-background)!important;color:var(--rice-mail-text)!important;filter:none!important}[data-rice-neutral-background]{background-color:transparent!important}[data-rice-neutral-text]{color:var(--rice-mail-text)!important}';
        doc.head.append(style);
    }
    sync(snapshot, { blocked = false, inputBlocked = false } = {}) {
        const document = this.layer.ownerDocument;
        const canvas = this.canvas.getBoundingClientRect(), layer = this.layer.getBoundingClientRect();
        const sx = canvas.width / this.canvas.width, sy = canvas.height / this.canvas.height;
        const windows = snapshot?.windows || [], present = new Set();
        for (const [index, native] of windows.entries()) {
            if (!native.frame) continue;
            const frame = native.frame, clips = [];
            for (const scene of native.commands || []) {
                if (scene.type === 'push-clip') { clips.push(scene.rect); continue; }
                if (scene.type === 'pop-clip') { clips.pop(); continue; }
                if (scene.type !== 'web-scene') continue;
                const html = mailDocumentHtml(scene);
                if (html === null) continue;
                const key = `${native.app}:${scene.surface}`;
                present.add(key);
                let record = this.records.get(key);
                if (!record) {
                    const holder = document.createElement('div'), iframe = document.createElement('iframe');
                    holder.className = 'rice-mail-document-portal';
                    holder.style.cssText = 'position:absolute;overflow:hidden;pointer-events:auto;';
                    iframe.title = 'Message body';
                    // No sender script executes, even though the trusted parent can
                    // inspect the document to handle links and browser selection.
                    iframe.setAttribute('sandbox', 'allow-same-origin');
                    iframe.referrerPolicy = 'no-referrer';
                    iframe.style.cssText = 'border:0;display:block;transform-origin:0 0;background:white;';
                    holder.append(iframe); this.layer.append(holder);
                    record = { holder, iframe, html: null };
                    iframe.addEventListener('load', () => {
                        const content = iframe.contentDocument;
                        if (!content) return;
                        const sizing = content.createElement('style');
                        sizing.textContent = 'html,body{height:auto!important;min-height:0!important;overflow:hidden!important}';
                        content.head.append(sizing);
                        const message = [...content.querySelectorAll('template[data-rice-mail-message]')].at(-1)?.getAttribute('data-rice-mail-message');
                        if (message) content.addEventListener('dblclick', event => {
                            if (!event.target.closest?.('a,button,input,textarea,select')) this.onOpenMessage(message);
                        });
                        const fallback = content.querySelector('template[data-rice-mail-fallback]');
                        const fallbackText = fallback?.content.textContent || '';
                        fallback?.remove();
                        const readable = [...content.body.querySelectorAll('*')].some(node => {
                            const style = content.defaultView.getComputedStyle(node);
                            return node.getBoundingClientRect().height > 0 && style.visibility !== 'hidden' && style.display !== 'none' && Number(style.opacity) > 0 &&
                                (node.matches('img[src^="data:"],img[src^="blob:"]') || [...node.childNodes].some(child => child.nodeType === 3 && child.textContent.trim()));
                        }) || [...content.body.childNodes].some(node => node.nodeType === 3 && node.textContent.trim());
                        if (!readable) {
                            content.querySelectorAll('style').forEach(node => node.remove());
                            const recoveredText = content.body.textContent.trim();
                            content.body.replaceChildren();
                            for (const attribute of [...content.body.attributes]) content.body.removeAttribute(attribute.name);
                            const pre = content.createElement('pre');
                            pre.style.cssText = 'white-space:pre-wrap;overflow-wrap:anywhere;font:15px/1.6 system-ui;padding:20px;color:#202124;background:white';
                            pre.textContent = fallbackText.trim() || recoveredText || 'This message has no readable content.';
                            content.body.append(pre);
                        }
                        // Recovery may have removed sender styles and the sizing rule.
                        content.head.append(sizing);
                        this.applyTheme(record);
                        const measure = () => {
                            if (!message || !record.iframe.isConnected) return;
                            const height = Math.min(100000, Math.max(40, Math.ceil(content.body.getBoundingClientRect().bottom + (parseFloat(content.defaultView.getComputedStyle(content.body).marginBottom) || 0))));
                            const signature = `${message}:${height}:${record.zoom}`;
                            if (signature !== record.measured) { record.measured = signature; this.onLayout(String(native.app), message, Math.ceil(height * (record.zoom || 1))); }
                        };
                        record.observer?.disconnect();
                        record.observer = new content.defaultView.ResizeObserver(measure);
                        record.observer.observe(content.body);
                        content.addEventListener('wheel', event => { event.preventDefault(); this.onScroll(String(native.app), event.deltaY * (event.deltaMode === 1 ? 22 : event.deltaMode === 2 ? 400 : 1), {x:record.rect.x + event.clientX * record.zoom, y:record.rect.y + event.clientY * record.zoom}); }, {passive:false});
                        measure();
                        content.addEventListener('pointerdown', () => { if (!record.active) this.onFocus(String(native.app)); });
                        content.addEventListener('submit', event => event.preventDefault());
                        content.addEventListener('click', event => {
                            const link = event.target.closest?.('a,area');
                            if (!link) return;
                            event.preventDefault();
                            const href = link.getAttribute('href') || link.getAttribute('xlink:href') || '';
                            if (href.startsWith('#')) { content.getElementById(href.slice(1))?.scrollIntoView(); return; }
                            try {
                                const url = new URL(href);
                                if (['http:', 'https:', 'mailto:'].includes(url.protocol)) this.onNavigate(url.href);
                            } catch {}
                        });
                    });
                    this.records.set(key, record);
                }
                if (record.html !== html) {
                    record.html = html;
                    record.iframe.srcdoc = inertDocument(document, html);
                }
                const r = scene.rect, x = frame.x+r.x, y = frame.y+frame.titleHeight+r.y;
                const width = r.width*sx, height = r.height*sy;
                Object.assign(record.holder.style, { left: `${canvas.left-layer.left+x*sx}px`,
                    top: `${canvas.top-layer.top+y*sy}px`, width: `${width}px`, height: `${height}px`, zIndex: String(index+1) });
                const zoom = Math.max(.7, Math.min(1.4, Number(html.match(/data-rice-mail-zoom="(\d+)"/)?.[1] || 1000) / 1000));
                record.zoom = zoom;
                record.rect = r;
                Object.assign(record.iframe.style, { width: `${r.width/zoom}px`, height: `${r.height/zoom}px`, transform: `scale(${zoom*sx},${zoom*sy})` });
                let visible = { ...r };
                for (const clip of clips) visible = intersect(visible, clip);
                const region = { x: (visible.x-r.x)*sx, y: (visible.y-r.y)*sy, width: visible.width*sx, height: visible.height*sy };
                let regions = [region];
                const covers = windows.slice(index+1).filter(w => w.layerVisible !== false && w.frame).map(w => w.frame);
                for (const window of windows.slice(index)) {
                    for (const overlay of [window.popup, window.dialog]) {
                        if (!overlay?.rect || !window.frame || window.layerVisible === false) continue;
                        covers.push({ ...overlay.rect, x: window.frame.x + overlay.rect.x, y: window.frame.y + window.frame.titleHeight + overlay.rect.y });
                    }
                }
                for (const cover of covers) {
                    const cut = intersect({ x, y, width:r.width, height:r.height }, cover);
                    const hole = intersect(region, { x:(cut.x-x)*sx, y:(cut.y-y)*sy, width:cut.width*sx, height:cut.height*sy });
                    if (hole.width && hole.height) regions = subtract(regions, hole);
                }
                record.holder.style.clipPath = `path("${regions.map(polygon).join('') || 'M0 0Z'}")`;
                record.active = native.active === true;
                record.holder.hidden = blocked || native.layerVisible === false;
                record.holder.style.pointerEvents = inputBlocked || blocked || native.dialog || native.popup ? 'none' : 'auto';
                record.holder.style.filter = native.dialog ? 'brightness(.6)' : '';
            }
        }
        for (const [key, record] of this.records) if (!present.has(key)) { record.observer?.disconnect(); record.holder.remove(); this.records.delete(key); }
    }
    hide() { for (const record of this.records.values()) record.holder.hidden = true; }
    destroy() { for (const record of this.records.values()) record.holder.remove(); this.records.clear(); }
}
