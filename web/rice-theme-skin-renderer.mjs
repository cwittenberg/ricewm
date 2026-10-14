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
        context.save();
        try{
            context.beginPath();roundedPath(context,rect,radius);
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
            if(border?.width){context.beginPath();roundedPath(context,rect,radius);context.strokeStyle=css(border.color);context.lineWidth=border.width;context.stroke();}
            if(state==='focused'&&skin.focus){context.beginPath();roundedPath(context,rect,radius);context.strokeStyle=css(skin.focus);const width=chrome?.metrics?.focusWidth??2;if(width>0){context.lineWidth=width;context.stroke();}}
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
