// Retain menu ownership from patches already accepted by the native host. This
// is presentation metadata; the original native tree still executes commands.
export class RiceApplicationMenus {
    constructor() { this.windows = new Map(); }
    observe(bytes) {
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        if (view.getUint16(6, true) !== 20) return;
        const key = `${view.getBigUint64(12, true)}:${view.getBigUint64(20, true)}`;
        let offset = 36;
        const take = count => { const start = offset; offset += count; if (offset > bytes.length) throw Error('Truncated menu metadata'); return start; };
        const u8 = () => view.getUint8(take(1));
        const u32 = () => view.getUint32(take(4), true);
        const id = () => view.getBigUint64(take(8), true).toString();
        const text = retain => { const length = u32(), start = take(length); return retain && length <= 256 ? new TextDecoder().decode(bytes.subarray(start, start + length)) : ''; };
        const labelFor = (kind,value) => kind===3 ? /^panel:(?:[^:]+:)*menu-gallery(?::|$)/.test(value||'') ? value : '' : value||'';
        const optional = read => u8() === 1 ? read() : undefined;
        const nodes = new Map(this.windows.get(key) || []);
        const remove = target => {
            const pending = [target], seen = new Set();
            while (pending.length) { const current = pending.pop(); if (seen.has(current)) continue; seen.add(current); for (const [id, node] of nodes) if (node.parent === current) pending.push(id); nodes.delete(current); }
        };
        try {
            const count = u32(); if (count > 65536) throw Error('Menu patch limit');
            for (let i = 0; i < count; i++) switch (u8()) {
                case 0: { const target = id(), parent = id(), index = u32(), kind = view.getUint16(take(2), true), state = u32(); const label = optional(() => text(kind === 3 || kind === 30 || kind === 31)); optional(() => take(2)); nodes.set(target, {parent, index, kind, state, text: labelFor(kind,label)}); break; }
                case 1: { const target = id(), node = nodes.get(target), state = optional(u32), label = optional(() => text(node?.kind === 3 || node?.kind === 30 || node?.kind === 31)); optional(() => take(2)); if (node) nodes.set(target, {...node, ...(state === undefined ? {} : {state}), ...(label === undefined ? {} : {text: labelFor(node.kind,label)})}); break; }
                case 2: { const target = id(), parent = id(), index = u32(); if (nodes.has(target)) nodes.set(target, {...nodes.get(target), parent, index}); break; }
                case 3: remove(id()); break;
                case 4: nodes.clear(); break;
                case 5: id(); text(false); break;
                case 6: take(9); break;
                case 7: take(16); break;
                case 8: case 9: id(); optional(() => text(false)); break;
                default: throw Error('Unsupported menu metadata');
            }
            if (offset !== bytes.length || nodes.size > 8192) throw Error('Menu metadata limit');
            if (nodes.size) this.windows.set(key, nodes); else this.windows.delete(key);
            while (this.windows.size > 128) this.windows.delete(this.windows.keys().next().value);
        } catch { this.windows.delete(key); }
    }
    contextMenuFor(app,window,target) {
        const nodes=this.windows.get(`${app}:${window}`),seen=new Set();
        let id=String(target);
        for(let depth=0;nodes&&depth<64;depth++){
            const node=nodes.get(id);if(!node||seen.has(id)||!(node.state&1))return null;seen.add(id);
            const bars=[...nodes].filter(([,child])=>child.parent===id&&child.kind===30&&child.text==='context'&&(child.state&1));
            for(const[bar]of bars){const menu=[...nodes].find(([,child])=>child.parent===bar&&child.kind===31&&(child.state&1));if(menu)return BigInt(menu[0]);}
            id=node.parent;
        }
        return null;
    }
    removeApp(app) { for (const key of this.windows.keys()) if (key.startsWith(`${app}:`)) this.windows.delete(key); }
    hasMenuGallery(app,window) { for(const node of this.windows.get(`${app}:${window}`)?.values()||[])if(node.kind===3&&typeof node.text==='string'&&node.text.split(':').includes('menu-gallery'))return true;return false; }
    galleryKeyTarget(app,window,snapshot) {
        if(!snapshot?.popup||snapshot.dialog||snapshot.app!==BigInt(app)||snapshot.window!==BigInt(window))return null;
        const nodes=this.windows.get(`${app}:${window}`);if(!nodes)return null;
        const marked=node=>node?.kind===3&&typeof node.text==='string'&&node.text.split(':').includes('menu-gallery');
        let id=String(snapshot.focusedWidget),gallery=null,menu=null;const seen=new Set();
        for(let depth=0;depth<64;depth++){
            const node=nodes.get(id);if(!node||seen.has(id)||!(node.state&1))return null;seen.add(id);
            if(marked(node))gallery=id;
            if(node.kind===31){menu=id;break;}
            id=node.parent;
        }
        if(!menu)return null;
        if(!gallery){
            const children=new Map();for(const[id,node]of nodes){const list=children.get(node.parent)||[];list.push(id);children.set(node.parent,list);}
            const pending=[menu],visited=new Set();
            while(pending.length&&visited.size<8192){
                const child=pending.pop(),node=nodes.get(child);if(!node||visited.has(child)||!(node.state&1))continue;visited.add(child);
                if(child!==menu&&node.kind===31)continue;
                if(marked(node)){if(gallery)return null;gallery=child;continue;}
                pending.push(...(children.get(child)||[]));
            }
        }
        return gallery?BigInt(gallery):null;
    }
    describe(app, window, snapshot, hitTest) {
        const nodes = this.windows.get(`${app}:${window}`);
        if (!nodes || snapshot.dialog) return null;
        const bar = [...nodes].find(([, node]) => node.kind === 30 && node.text !== 'context' && (node.state & 1));
        if (!bar) return null;
        const menus = [...nodes].filter(([, node]) => node.parent === bar[0] && node.kind === 31 && node.text).sort((a, b) => a[1].index - b[1].index);
        if (menus.length > 32) return null;
        const items = menus.map(([id, node]) => {
            const command = snapshot.commands.find(command => command.type === 'text' && command.rect.y >= 0 && command.rect.y < 80 && String(hitTest(command.rect.x + command.rect.width / 2, command.rect.y + command.rect.height / 2)) === id);
            return command ? {id, title: command.text, enabled: Boolean(node.state & 1), rect: command.rect} : null;
        }).filter(Boolean);
        if (!items.length || items.length !== menus.length) return null;
        const bottom = Math.max(...items.map(item => item.rect.y + item.rect.height));
        const band = snapshot.commands.find(command => command.type === 'fill' && command.rect.x <= Math.min(...items.map(item => item.rect.x)) && command.rect.y >= 0 && command.rect.y <= Math.min(...items.map(item => item.rect.y)) && command.rect.x + command.rect.width >= Math.max(...items.map(item => item.rect.x + item.rect.width)) && command.rect.y + command.rect.height >= bottom && command.rect.height <= 80);
        return band ? {height: band.rect.height, y: band.rect.y, items} : null;
    }
}

export function translateMenuCommands(commands, x, y) {
    return commands.map(command => command.rect ? {...command, rect: {...command.rect, x: command.rect.x + x, y: command.rect.y + y}} : command);
}

/** Remove one native header band while retaining preceding navigation rows. */
export function removeApplicationMenuBand(commands, top, height) {
    if (!height) return commands;
    const bottom = top + height;
    return commands.flatMap(command => {
        const rect = command.rect;
        if (!rect || rect.y + rect.height <= top) return [command];
        if (rect.y >= bottom) return [{...command, rect:{...rect, y:rect.y-height}}];
        if (rect.y >= top && rect.y + rect.height <= bottom) return command.type === 'push-clip' ? [{...command, rect:{...rect, y:top, height:0}}] : [];
        const overlap = Math.min(bottom, rect.y + rect.height) - Math.max(top, rect.y);
        return [{...command, rect:{...rect, y:Math.min(rect.y, top), height:rect.height-overlap}}];
    });
}
