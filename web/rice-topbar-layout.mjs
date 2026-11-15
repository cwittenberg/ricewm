export const TOPBAR_DEFAULTS = Object.freeze({version:1,tooltips:true,items:[]});
// Saved preferences and temporary control owners compose without writing back
// into one another. Callers commit a returned state only after native admission.
export class TopbarLayoutState {
    constructor(base=TOPBAR_DEFAULTS,owners=new Map()) { this.base=base;this.owners=owners; }
    withBase(value) {
        const parsed=parseTopbarLayout(value);
        if(!parsed)throw new TypeError('Invalid saved topbar layout');
        const next=new TopbarLayoutState(parsed,new Map(this.owners));next.effective();return next;
    }
    withOwner(owner,patches) {
        if(typeof owner!=='string'||!owner||owner.length>256||/[\0\r\n]/.test(owner))throw new TypeError('Invalid topbar control owner');
        if(!Array.isArray(patches)||patches.length>64)throw new TypeError('Invalid temporary topbar layout');
        const seen=new Set(),copy=patches.map(patch=>{
            if(!patch||typeof patch!=='object'||Object.keys(patch).some(key=>!['key','position','visible','present','icon'].includes(key))||
                typeof patch.key!=='string'||!/^[a-zA-Z0-9@_.:+-]{1,128}$/.test(patch.key)||seen.has(patch.key)||
                Object.hasOwn(patch,'position')&&![0,1,2].includes(patch.position)||
                Object.hasOwn(patch,'visible')&&typeof patch.visible!=='boolean'||
                Object.hasOwn(patch,'present')&&typeof patch.present!=='boolean'||
                Object.hasOwn(patch,'icon')&&(typeof patch.icon!=='string'||! /^(?:[a-z0-9]+(?:-[a-z0-9]+)*)?$/.test(patch.icon)||patch.icon.length>128))
                throw new TypeError('Invalid temporary topbar item');
            seen.add(patch.key);return {...patch};
        });
        const owners=new Map(this.owners);owners.delete(owner);if(copy.length)owners.set(owner,copy);
        if(owners.size>64)throw new RangeError('Too many topbar control owners');
        const next=new TopbarLayoutState(this.base,owners);next.effective();return next;
    }
    effective() {
        const rows=new Map(this.base.items.map(item=>[item.key,{...item}]));
        for(const patches of this.owners.values())for(const patch of patches)rows.set(patch.key,{...rows.get(patch.key),...patch});
        if(rows.size>64)throw new RangeError('Composed topbar layout exceeds its item bound');
        return {version:1,tooltips:this.base.tooltips,items:[...rows.values()]};
    }
    item(key,defaults={}) { return {...defaults,...this.effective().items.find(item=>item.key===key)}; }
}
export function parseTopbarLayout(value) {
    try {
        const data=typeof value==='string'?JSON.parse(value):value;
        if (!data || data.version!==1 || typeof data.tooltips!=='boolean' || !Array.isArray(data.items) || data.items.length>64) return null;
        const keys=new Set();
        for(const item of data.items) {
            if (!item || typeof item.key!=='string' || !/^[a-zA-Z0-9@_.:+-]{1,128}$/.test(item.key) || keys.has(item.key) || typeof item.visible!=='boolean' || ![0,1,2].includes(item.position) || typeof item.icon!=='string' || !/^(?:[a-z0-9]+(?:-[a-z0-9]+)*)?$/.test(item.icon)) return null;
            keys.add(item.key);
        }
        return {version:1,tooltips:data.tooltips,items:data.items.map(({key,visible,position,icon})=>({key,visible,position,icon}))};
    } catch { return null; }
}
export function topbarRows(renderer) {
    const panel=renderer.windows.get(0xe0000008);
    if(!panel?.viewable)return [];
    const rows=(renderer.shellTexts.get(panel.id)||[]).filter(r=>r.text.startsWith('__topbar\t')).map(row=>{
        const [,key,title,icon,position]=row.text.split('\t');
        return {key,title,icon,position:Number(position),x:panel.x+row.x,y:panel.y,width:row.width,height:panel.height};
    });
    const frames=renderer.nativeCanvasWindow?.gnomePanelSurfaces;
    if(!frames?.size)return rows;
    for(const row of rows){
        row.retainedRect={x:row.x,y:row.y,width:row.width,height:row.height};
        const frame=frames.get(row.key);
        if(frame&&row.width>0)row.width=frame.width;
    }
    return layoutTopbarRows(rows,panel);
}

// Drag previews use the same 32px icon slots, 4px gaps and 8px edge insets
// as the retained compositor. Only the drop commits these preferences.
export function previewTopbarRows(rows, key, before, position, panel) {
    const source=rows.filter(row=>row.width>0).map(row=>({...row}));
    const moving=source.find(row=>row.key===key);
    if(!moving || before===key)return source;
    const items=source.filter(row=>row.key!==key);
    moving.position=position;
    const index=items.findIndex(row=>row.key===before);
    items.splice(index<0?items.length:index,0,moving);
    items.sort((a,b)=>a.position-b.position);
    return layoutTopbarRows(items,panel);
}

export function layoutTopbarRows(rows,panel) {
    const items=rows.map(row=>({...row}));
    let left=panel.x+8,right=panel.x+panel.width-8;
    for(const item of items.filter(row=>row.position===0)) {item.x=left;left+=item.width+4;}
    for(const item of items.filter(row=>row.position===2).reverse()) {
        if(right-item.width<left){item.width=0;continue;}
        right-=item.width;item.x=right;right-=4;
    }
    const center=items.filter(row=>row.position===1);
    const width=center.reduce((sum,row)=>sum+row.width+4,0);
    let cursor=Math.max(left,Math.min(panel.x+(panel.width-width)/2,right-width));
    for(const item of center){if(cursor+item.width>right){item.width=0;continue;}item.x=Math.floor(cursor);cursor+=item.width+4;}
    return items;
}

/** Application menus share the retained top bar; overflow keeps status usable. */
export function applicationTopbarRows(renderer, rows) {
    const snapshot = renderer.nativeCanvasWindow?.snapshot;
    const panel = renderer.windows.get(0xe0000008);
    if (!panel || snapshot?.chrome?.shellAppearance?.menuBar?.mode !== 'application') return rows;
    const snapshots=renderer.nativeCanvasWindow.snapshots||[];
    const hasApplicationContext=snapshots.some(window=>typeof window.applicationActive==='boolean');
    const active = renderer.nativeCanvasWindow.foregroundActive !== false ? snapshots.find(window =>
        (hasApplicationContext ? window.applicationActive===true : window.active&&!window.actorPopup&&!window.transientPopup) &&
        window.layerVisible !== false && !window.minimized) : null;
    const names = {'rice-files':'Files','rice-settings':'Settings','rice-paper':'RicePaper','rice-box':'Rice Box','rice-calc':'Calculator','rice-term':'Terminal','bowl':'Bowl'};
    const title = names[active?.entrypoint] || active?.title || 'Desktop';
    const system = rows.find(row => row.key === '@applications');
    const items = system ? [{...system, title:'RiceWM', label:'RiceWM', width:76, position:0}] : [];
    const status = rows.filter(row => row.key !== '@applications').map(row => ({...row, position:2, ...(row.key === '@clock' ? {width:panel.width<600?78:154} : {})}));
    const available = panel.width - status.reduce((sum, row) => sum + row.width + 4, 16);
    const appWidth = Math.min(160, Math.max(64, title.length * 7 + 20));
    if (active && snapshot.chrome.shellAppearance.menuBar.showAppName !== false && available >= 84 + appWidth + 74) items.push({key:`@active-application:${active.app}`, title, label:title, app:active.app, position:0, width:appWidth, height:panel.height, y:panel.y});
    const menus=active?.applicationMenu?.items || [];
    const widthOf=menu=>Math.max(42, Math.min(180, menu.title.length * 7 + 20));
    let used = items.reduce((sum, row) => sum + row.width + 4, 8);
    const overflow=used+menus.reduce((sum,menu)=>sum+widthOf(menu)+4,0)>available;
    if(overflow&&menus.length){
        items.push({key:`@application-menu-overflow:${active.app}`,title:'Application menus',label:'Menus',app:active.app,menuItems:menus,position:0,width:70,y:panel.y,height:panel.height});
    }else for (const menu of menus) {
        const width=widthOf(menu);
        items.push({key:`@application-menu:${active.app}:${menu.id}`, title:menu.title, label:menu.title, target:menu.id, app:active.app, selected:menu.id===active.applicationMenu.activeTarget, enabled:menu.enabled && !active.dialog, position:0, width, y:panel.y, height:panel.height});
        used += width + 4;
    }
    return layoutTopbarRows([...items, ...status], panel);
}
