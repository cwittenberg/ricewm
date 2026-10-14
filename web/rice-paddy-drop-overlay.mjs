// Compositor-only feedback: never inserts a hit target or changes the draft.
export function paddyDropOverlayRect(snapshots, visual) {
    if (!visual?.accepted) return null;
    const native=[...snapshots].reverse().find(w=>w.layerVisible!==false&&!w.minimized&&w.frame&&
        visual.x>=w.frame.x&&visual.x<w.frame.x+w.frame.width&&visual.y>=w.frame.y&&visual.y<w.frame.y+w.frame.height);
    if(native?.entrypoint!=='rice-paddy')return null;
    const scene=native.commands?.find(c=>c.type==='web-scene');
    if(!scene?.rect)return null;
    const r={x:native.frame.x+scene.rect.x+3,y:native.frame.y+native.frame.titleHeight+scene.rect.y+3,width:scene.rect.width-6,height:scene.rect.height-6};
    if(r.width<20||r.height<20)return null;
    return r;
}
export function paintPaddyDropOverlay(context, snapshots, visual) {
    const r=paddyDropOverlayRect(snapshots,visual);
    if(!r)return false;
    const native=[...snapshots].reverse().find(w=>w.entrypoint==='rice-paddy'&&w.frame&&r.x>=w.frame.x&&r.x<w.frame.x+w.frame.width);
    context.save();
    try {
        const accent=native.chrome?.active||[238,85,32,255];
        const rgb=accent.slice(0,3).join(',');
        context.fillStyle=`rgba(${rgb},0.10)`;context.fillRect(r.x,r.y,r.width,r.height);
        context.strokeStyle=`rgba(${rgb},0.95)`;context.lineWidth=2;context.setLineDash([7,5]);context.strokeRect(r.x,r.y,r.width,r.height);context.setLineDash([]);
        const label='Drop here',cx=r.x+r.width/2,cy=r.y+r.height/2;
        context.font='14px "RiceWM Sans", sans-serif';
        const width=context.measureText(label).width+32;
        context.fillStyle='rgba(28,31,36,0.94)';context.fillRect(cx-width/2,cy-20,width,40);
        context.fillStyle='#fff';context.textAlign='center';context.textBaseline='middle';context.fillText(label,cx,cy);
        return true;
    } finally {context.restore();}
}
