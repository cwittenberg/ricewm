import {validateGridDecorations,GRID_DECORATION_SCALE} from '../sdk/versions/1.0.0/runtime/sdk/js/grid-decorations.mjs';
export const GRID_DECORATIONS_RENDERER_VERSION=1;

// Vector symbols use document colors and stay independent of desktop icon
// themes, installed fonts, emoji rendering and network assets.
function symbol(context,kind,x,y,size,color,filled=true){
    const c=context;c.save();
    try{
        c.translate(x+size/2,y+size/2);c.scale(size/16,size/16);
        const angle={ArrowRight:Math.PI/2,ArrowDown:Math.PI,ArrowAngleUp:Math.PI/4,ArrowAngleDown:3*Math.PI/4,TriangleDown:Math.PI,TriangleDownFilled:Math.PI,ThumbsDown:Math.PI}[kind]||0;
        if(angle)c.rotate(angle);c.translate(-8,-8);c.fillStyle=color;c.strokeStyle=color;c.lineWidth=1.6;c.lineCap='round';c.lineJoin='round';c.beginPath();
        const polygon=points=>{c.moveTo(...points[0]);for(const point of points.slice(1))c.lineTo(...point);c.closePath();};
        let lineOnly=false;
        if(kind.startsWith('Arrow'))polygon([[8,1],[1,8],[5,8],[5,15],[11,15],[11,8],[15,8]]);
        else if(kind.startsWith('Triangle')){polygon([[8,2],[1,14],[15,14]]);if(!kind.endsWith('Filled'))lineOnly=true;}
        else if(kind==='Circle')c.arc(8,8,6,0,2*Math.PI);
        else if(kind==='FlatRectangle')c.rect(2,6,12,4);
        else if(kind==='Rhombus')polygon([[8,1],[15,8],[8,15],[1,8]]);
        else if(kind==='Flag'){polygon([[4,2],[14,3],[11,7],[4,7]]);c.moveTo(4,1);c.lineTo(4,15);}
        else if(kind==='Check'){c.moveTo(2,8);c.lineTo(6,12);c.lineTo(14,3);lineOnly=true;c.lineWidth=2.5;}
        else if(kind==='Cross'){c.moveTo(3,3);c.lineTo(13,13);c.moveTo(13,3);c.lineTo(3,13);lineOnly=true;c.lineWidth=2.5;}
        else if(kind==='Exclamation'){c.moveTo(8,2);c.lineTo(8,10);c.moveTo(8,13);c.lineTo(8,13.3);lineOnly=true;c.lineWidth=2.5;}
        else if(kind==='Star')polygon(Array.from({length:10},(_,index)=>{const angle=-Math.PI/2+index*Math.PI/5,radius=index%2?3:7;return[8+Math.cos(angle)*radius,8+Math.sin(angle)*radius];}));
        else if(kind==='Heart'){c.moveTo(8,14);c.bezierCurveTo(-4,6,2,-1,8,4);c.bezierCurveTo(14,-1,20,6,8,14);c.closePath();}
        else if(kind==='ThumbsUp'||kind==='ThumbsDown')polygon([[1,7],[4,7],[7,3],[7,1],[10,1],[10,6],[14,6],[15,8],[12,14],[5,14],[5,8],[4,8],[4,14],[1,14]]);
        if(filled&&!lineOnly)c.fill();c.stroke();
    }finally{c.restore();}
}
const rgb=hex=>[1,3,5].map(offset=>parseInt(hex.slice(offset,offset+2),16));
export function paintGridDecorations(context,{rect,decorations,zoomPercent=100,background=[255,255,255,255]}){
    validateGridDecorations(decorations);
    if(!rect||!['x','y','width','height'].every(key=>Number.isFinite(rect[key]))||rect.width<0||rect.height<0||!Number.isInteger(zoomPercent)||zoomPercent<25||zoomPercent>400)throw RangeError('Invalid grid decoration geometry');
    const c=context,scale=zoomPercent/100,padding=Math.max(1,2*scale);
    c.save();
    try{
        c.beginPath();c.rect(rect.x,rect.y,rect.width,rect.height);c.clip();
        const bar=decorations.data_bar;
        if(bar){
            const width=Math.max(0,rect.width-padding*2),height=Math.max(0,rect.height-padding*2);
            const axis=rect.x+padding+width*bar.axis/GRID_DECORATION_SCALE,value=rect.x+padding+width*bar.value/GRID_DECORATION_SCALE;
            const color=value>=axis?bar.positive_color:bar.negative_color;
            if(value!==axis&&height){
                c.fillStyle=color;
                if(bar.gradient){const gradient=c.createLinearGradient(axis,0,value,0),foreground=rgb(color);gradient.addColorStop(0,color);gradient.addColorStop(1,`rgb(${foreground.map((channel,index)=>Math.round(channel*.25+background[index]*.75)).join(',')})`);c.fillStyle=gradient;}
                c.fillRect(Math.min(axis,value),rect.y+padding,Math.abs(value-axis),height);
            }
            if(bar.axis>0&&bar.axis<GRID_DECORATION_SCALE){c.fillStyle=background.slice(0,3).reduce((a,b)=>a+b,0)<384?'#C0C0C0':'#707070';c.fillRect(Math.round(axis),rect.y+padding,Math.max(1,scale),height);}
        }
        let x=rect.x+padding;const size=Math.max(0,Math.min(16*scale,rect.height-padding*2)),y=rect.y+(rect.height-size)/2,step=Math.max(1,18*scale);
        if(decorations.icon){const icon=decorations.icon;symbol(c,icon.kind,x,y,size,icon.color);x+=step;}
        if(decorations.rating){const rating=decorations.rating;for(let index=0;index<rating.total;index++){symbol(c,rating.kind,x,y,size,rating.color,index<rating.count);x+=step;}}
    }finally{c.restore();}
}
