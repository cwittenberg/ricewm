/** Reusable cut-metal chrome and signal shutters. No permanent animation loop. */
export function cyberpunkPath(context, rect, cut = 12) {
    const {x,y,width:w,height:h}=rect;
    const c=Math.max(0,Math.min(cut,w/4,h/4));
    context.beginPath();context.moveTo(x+c,y);context.lineTo(x+w-c,y);
    context.lineTo(x+w,y+c);context.lineTo(x+w,y+h-c);context.lineTo(x+w-c,y+h);
    context.lineTo(x+c,y+h);context.lineTo(x,y+h-c);context.lineTo(x,y+c);context.closePath();
}
export function paintCyberpunkFrame(context,rect,surface,active=true) {
    context.save();
    context.strokeStyle=active?surface.edge:'#394247';context.lineWidth=active?1.5:1;
    cyberpunkPath(context,{x:rect.x+.75,y:rect.y+.75,width:rect.width-1.5,height:rect.height-1.5},surface.cornerLength||12);context.stroke();
    context.globalAlpha*=.5;context.strokeStyle=surface.edge;
    context.beginPath();context.moveTo(rect.x+24,rect.y+4);context.lineTo(rect.x+Math.min(116,rect.width/3),rect.y+4);context.stroke();
    context.restore();
}
export function paintCyberpunkCurtain(context,frame,reveal,accent='#f4ef18') {
    if(reveal>=1)return;
    const {x,y,width,height}=frame;
    context.save();cyberpunkPath(context,frame);context.clip();
    // Offset graphite plates retract in alternating directions. No typography.
    const bands=5,bandHeight=height/bands;
    for(let i=0;i<bands;i++){
        const delay=[0,.065,.025,.085,.045][i];
        const progress=Math.max(0,Math.min(1,(reveal-delay)/(1-delay)));
        const remaining=width*(1-progress),reverse=i%2===1,left=reverse?x+width-remaining:x;
        const top=y+i*bandHeight,edge=reverse?left:left+remaining;
        context.save();context.beginPath();context.rect(left,top,remaining,bandHeight+.5);context.clip();
        context.fillStyle=i%2?'#161d24':'#10151b';context.fillRect(left,top,remaining,bandHeight+1);
        context.strokeStyle='#303b46';context.lineWidth=1;
        context.beginPath();context.moveTo(left,top+bandHeight-1);context.lineTo(left+remaining,top+bandHeight-1);context.stroke();
        context.fillStyle=accent;context.globalAlpha*=.14;
        context.fillRect(reverse?edge:edge-22,top,22,bandHeight);
        context.globalAlpha=1;context.fillRect(reverse?edge:edge-2,top,2,bandHeight);
        context.restore();
    }
    context.restore();
}
