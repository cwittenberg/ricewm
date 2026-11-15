import {roundedPath,rgbaColor} from './rice-native-canvas-primitives.mjs';
import {themeColor,themeStateStyle} from './rice-theme-customization.mjs';
const css=color=>rgbaColor(themeColor(color,[0,0,0,0]));
/** Retained canvas skins share native layout and never create input surfaces. */
export class RiceThemeSkinRenderer {
    constructor(onDamage=()=>{}){this.onDamage=onDamage;this.images=new Map();}
    image(artwork){
        if(!artwork||typeof artwork!=='object'||typeof Image==='undefined')return null;
        const source=artwork.svg?'data:image/svg+xml;charset=utf-8,'+encodeURIComponent(artwork.svg):'data:image/png;base64,'+artwork.png;
        let image=this.images.get(source);
        if(!image){image=new Image();this.images.set(source,image);image.onload=()=>{if(this.images.get(source)===image)this.onDamage();};image.src=source;}
        return image.complete&&image.naturalWidth?image:null;
    }
    reset(){for(const image of this.images.values())image.onload=null;this.images.clear();}
    style(chrome,role,state){return {...themeStateStyle(chrome?.controlColors,role,state),...themeStateStyle(chrome?.surfaceSkins,role,state)};}
    paint(context,rect,chrome,role,state='normal'){
        const signature=JSON.stringify(chrome?.surfaceSkins||{});if(signature!==this.signature){this.reset();this.signature=signature;}
        const skin=this.style(chrome,role,state);if(!Object.keys(skin).length||rect.width<=0||rect.height<=0)return false;
        const radius=skin.radius??chrome?.radius??4;
        const path=(r=rect,inset=0,override)=>{if(skin.cornerRadii&&typeof context.roundRect==='function'){context.beginPath();context.roundRect(r.x,r.y,r.width,r.height,skin.cornerRadii.map(v=>Math.max(0,(override??v)-inset)));}else roundedPath(context,r,override??Math.max(0,radius-inset));};
        context.save();
        try{
            context.beginPath();path();
            if(skin.shadow){context.shadowColor=css(skin.shadow.color);context.shadowBlur=skin.shadow.blur??0;context.shadowOffsetX=skin.shadow.x??0;context.shadowOffsetY=skin.shadow.y??0;context.fillStyle=css(skin.background??'#ffffff');context.fill();context.shadowColor='transparent';}
            if(skin.background!==undefined||skin.gradient){
                context.fillStyle=css(skin.background??'transparent');
                if(skin.gradient){const angle=(skin.gradient.angle??90)*Math.PI/180,dx=Math.cos(angle)*rect.width/2,dy=Math.sin(angle)*rect.height/2,cx=rect.x+rect.width/2,cy=rect.y+rect.height/2,g=context.createLinearGradient(cx-dx,cy-dy,cx+dx,cy+dy);for(const stop of skin.gradient.stops)g.addColorStop(stop.at,css(stop.color));context.fillStyle=g;}
                context.fill();
            }
            const image=this.image(skin.artwork);
            context.save();context.clip();
            if(image){
                if(skin.slice)this.slice(context,image,rect,skin.slice);
                else if(skin.imageMode==='tile'){context.fillStyle=context.createPattern(image,'repeat');context.translate(rect.x,rect.y);context.fillRect(0,0,rect.width,rect.height);context.translate(-rect.x,-rect.y);}
                else if(skin.imageMode==='contain'){const scale=Math.min(rect.width/image.naturalWidth,rect.height/image.naturalHeight),w=image.naturalWidth*scale,h=image.naturalHeight*scale;context.drawImage(image,rect.x+(rect.width-w)/2,rect.y+(rect.height-h)/2,w,h);}
                else context.drawImage(image,rect.x,rect.y,rect.width,rect.height);
            }
            if(skin.texture){const t=skin.texture,spacing=t.spacing;context.globalAlpha*=t.opacity??1;context.fillStyle=css(t.color);for(let y=0;y<rect.height;y+=spacing)context.fillRect(rect.x,rect.y+y,t.kind==='dots'?0:rect.width,1);for(let x=0;x<rect.width;x+=spacing){if(t.kind==='grid')context.fillRect(rect.x+x,rect.y,1,rect.height);if(t.kind==='dots')for(let y=0;y<rect.height;y+=spacing)context.fillRect(rect.x+x,rect.y+y,1,1);}}
            context.restore();
            const border=typeof skin.border==='string'?{color:skin.border,width:chrome?.metrics?.borderWidth??1}:skin.border;
            if(border?.width){
                context.strokeStyle=css(border.color);context.lineWidth=border.width;
                if(border.edges){
                    const radii=(skin.cornerRadii||[radius,radius,radius,radius]).map(r=>Math.min(r,rect.width/2,rect.height/2));
                    const [tl,tr,br,bl]=radii,x=rect.x,y=rect.y,r=x+rect.width,b=y+rect.height;
                    context.beginPath();
                    for(const edge of border.edges){
                        if(edge==='top'){context.moveTo(x,y+tl);context.quadraticCurveTo(x,y,x+tl,y);context.lineTo(r-tr,y);context.quadraticCurveTo(r,y,r,y+tr);}
                        if(edge==='right'){context.moveTo(r,y+tr);context.lineTo(r,b-br);}
                        if(edge==='bottom'){context.moveTo(r,b-br);context.quadraticCurveTo(r,b,r-br,b);context.lineTo(x+bl,b);context.quadraticCurveTo(x,b,x,b-bl);}
                        if(edge==='left'){context.moveTo(x,b-bl);context.lineTo(x,y+tl);}
                    }
                    context.stroke();
                }else{context.beginPath();path();context.stroke();}
            }
            // Rims stay inside the native control bounds, including fractional
            // pixel outlines. They never enlarge pointer targets or the layout.
            for(const rim of skin.rims||[]){const inset=(rim.inset??0)+rim.width/2,r={x:rect.x+inset,y:rect.y+inset,width:rect.width-2*inset,height:rect.height-2*inset};if(r.width<=0||r.height<=0)continue;context.beginPath();path(r,inset,rim.radius);context.strokeStyle=css(rim.color);context.lineWidth=rim.width;context.stroke();}
            if(skin.bevel||skin.bevels?.length||skin.accent){
                context.save();context.beginPath();path();context.clip();
                for(const b of [...(skin.bevels||[]),...(skin.bevel?[skin.bevel]:[])]){const inset=(b.inset??1)+(b.width??1)/2,x=rect.x+inset,y=rect.y+inset,w=rect.width-2*inset,h=rect.height-2*inset;if(w>0&&h>0){context.lineWidth=b.width??1;context.lineJoin='round';context.strokeStyle=css(b.light);context.beginPath();context.moveTo(x,y+h);context.lineTo(x,y);context.lineTo(x+w,y);context.stroke();context.strokeStyle=css(b.dark);context.beginPath();context.moveTo(x+w,y);context.lineTo(x+w,y+h);context.lineTo(x,y+h);context.stroke();}}
                const a=skin.accent;if(a){const i=a.inset??1,w=a.width;context.fillStyle=css(a.color);if(a.edge==='top'||a.edge==='bottom')context.fillRect(rect.x+i,a.edge==='top'?rect.y+i:rect.y+rect.height-i-w,Math.max(0,rect.width-2*i),w);else context.fillRect(a.edge==='left'?rect.x+i:rect.x+rect.width-i-w,rect.y+i,w,Math.max(0,rect.height-2*i));}
                context.restore();
            }
            if((state==='focused'||state.endsWith('Focused'))&&skin.focus){context.beginPath();path();context.strokeStyle=css(skin.focus);const width=chrome?.metrics?.focusWidth??2;if(width>0){context.lineWidth=width;context.stroke();}}
            return true;
        }finally{context.restore();}
    }
    slice(context,image,rect,[top,right,bottom,left]){
        const width=image.naturalWidth,height=image.naturalHeight;
        const sxScale=Math.min(1,width/Math.max(1,left+right)),syScale=Math.min(1,height/Math.max(1,top+bottom));left*=sxScale;right*=sxScale;top*=syScale;bottom*=syScale;
        const dxScale=Math.min(1,rect.width/Math.max(1,left+right)),dyScale=Math.min(1,rect.height/Math.max(1,top+bottom));
        const sx=[0,left,width-right,width],sy=[0,top,height-bottom,height],dx=[rect.x,rect.x+left*dxScale,rect.x+rect.width-right*dxScale,rect.x+rect.width],dy=[rect.y,rect.y+top*dyScale,rect.y+rect.height-bottom*dyScale,rect.y+rect.height];
        for(let y=0;y<3;y++)for(let x=0;x<3;x++)if(sx[x+1]>sx[x]&&sy[y+1]>sy[y]&&dx[x+1]>dx[x]&&dy[y+1]>dy[y])context.drawImage(image,sx[x],sy[y],sx[x+1]-sx[x],sy[y+1]-sy[y],dx[x],dy[y],dx[x+1]-dx[x],dy[y+1]-dy[y]);
    }
}
