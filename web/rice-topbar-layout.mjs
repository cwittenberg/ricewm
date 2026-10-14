export const TOPBAR_DEFAULTS = Object.freeze({version:1,tooltips:true,items:[]});
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
    return (renderer.shellTexts.get(panel.id)||[]).filter(r=>r.text.startsWith('__topbar\t')).map(row=>{
        const [,key,title,icon,position]=row.text.split('\t');
        return {key,title,icon,position:Number(position),x:panel.x+row.x,y:panel.y,width:row.width,height:panel.height};
    });
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
