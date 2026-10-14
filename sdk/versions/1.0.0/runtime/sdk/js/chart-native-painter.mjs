import {SceneBuilder} from './scene-builder.mjs';

const identity=[1,0,0,1,0,0];
let parseColor;
export function configureEChartsPainter(parser){parseColor=parser;}
function rgba(color,opacity=1){
    if(color==null||color==='none')return null;
    let r,g,b,a=1;
    if(typeof color!=='string')throw TypeError('Unsupported ECharts paint');
    if(/^#[0-9a-f]{6}$/i.test(color)){r=parseInt(color.slice(1,3),16);g=parseInt(color.slice(3,5),16);b=parseInt(color.slice(5,7),16);}
    else if(/^#[0-9a-f]{3}$/i.test(color)){[r,g,b]=[1,2,3].map(i=>parseInt(color[i]+color[i],16));}
    else{const parsed=parseColor?.(color);if(!parsed)throw TypeError('Invalid ECharts color');[r,g,b,a]=parsed;}
    if(![r,g,b,a,opacity].every(Number.isFinite))throw TypeError('Invalid ECharts color');
    return (Math.round(Math.max(0,Math.min(1,a*opacity))*255)*0x1000000+(Math.round(b)<<16)+(Math.round(g)<<8)+Math.round(r))>>>0;
}
/** Adapt zrender PathProxy verbs, not chart layout. Elliptical arcs become
 * cubic Beziers because RiceDisplay's portable path ABI has no arc verb. */
export function echartsPathSegments(proxy){
    const data=proxy.data,segments=[];let i=0;
    while(i<proxy.len()){
        const op=data[i++];
        if(op===1||op===2){segments.push([op===1?'M':'L',data[i++],data[i++]]);}
        else if(op===3){segments.push(['C',...Array.from(data.slice(i,i+6))]);i+=6;}
        else if(op===4){segments.push(['Q',...Array.from(data.slice(i,i+4))]);i+=4;}
        else if(op===6)segments.push(['Z']);
        else if(op===7){const x=data[i++],y=data[i++],w=data[i++],h=data[i++];segments.push(['M',x,y],['L',x+w,y],['L',x+w,y+h],['L',x,y+h],['Z']);}
        else if(op===5){
            const cx=data[i++],cy=data[i++],rx=data[i++],ry=data[i++],start=data[i++],delta=data[i++],rotation=data[i++];i++;
            const count=Math.max(1,Math.ceil(Math.abs(delta)/(Math.PI/2))),angle=delta/count;
            const point=t=>[cx+rx*Math.cos(t)*Math.cos(rotation)-ry*Math.sin(t)*Math.sin(rotation),cy+rx*Math.cos(t)*Math.sin(rotation)+ry*Math.sin(t)*Math.cos(rotation)];
            const tangent=t=>[-rx*Math.sin(t)*Math.cos(rotation)-ry*Math.cos(t)*Math.sin(rotation),-rx*Math.sin(t)*Math.sin(rotation)+ry*Math.cos(t)*Math.cos(rotation)];
            const p=point(start);segments.push([segments.length?'L':'M',...p]);
            for(let step=0;step<count;step++){
                const a=start+step*angle,b=a+angle,k=4/3*Math.tan(angle/4),p0=point(a),p1=point(b),t0=tangent(a),t1=tangent(b);
                segments.push(['C',p0[0]+k*t0[0],p0[1]+k*t0[1],p1[0]-k*t1[0],p1[1]-k*t1[1],...p1]);
            }
        }else throw TypeError('Unsupported zrender path verb '+op);
        if(segments.length>16384)throw RangeError('Chart path is too complex');
    }
    return segments;
}
function clipPath(builder,id,segments,transform){
    builder.definePath(id,segments).command(20,32,view=>{
        view.setUint32(8,id,true);transform.forEach((v,i)=>view.setFloat32(12+i*4,v,true));
    });
}
function transformedRect(element){
    const rect=element.getBoundingRect().clone();if(element.transform)rect.applyTransform(element.transform);
    return rect;
}
/** Native vector paint and native-theme text overlays. ECharts owns scales,
 * stacking, symbols, paths, clipping, layout and exact containment tests. */
export function paintECharts(engine,width,height,theme){
    const builder=new SceneBuilder(width,height),labels=[];let id=1;
    builder.rect([0,0,width,height],rgba(theme.panel));
    const list=engine.getZr().storage.getDisplayList(true);
    if(list.length>12000)throw RangeError('Chart display exceeds 12000 elements');
    for(const element of list){
        if(element.invisible||element.ignore||element.style?.opacity===0)continue;
        const style=element.style||{},opacity=style.opacity??1;
        if(element.type==='tspan'){
            if(!style.text)continue;
            const rect=transformedRect(element);
            if(Math.abs(element.transform?.[1]||0)>.001||Math.abs(element.transform?.[2]||0)>.001)throw TypeError('Native chart text requires unrotated labels');
            // Native theme text metrics differ slightly from zrender's fallback.
            const extra=40,height=Math.max(36,rect.height+24);
            const ink=rgba(style.fill||theme.foreground),foreground='#'+[ink&255,(ink>>>8)&255,(ink>>>16)&255].map(value=>value.toString(16).padStart(2,'0')).join('');
            labels.push({foreground,fontSize:Math.max(8,Math.min(72,Number(style.fontSize)||14)),text:String(style.text),x:rect.x-extra/2,y:rect.y+(rect.height-height)/2,width:Math.max(42,rect.width+extra),height,muted:style.fill===theme.muted,bold:style.fontWeight==='bold'||Number(style.fontWeight)>=600});
            if(labels.length>256)throw RangeError('Chart has too many labels');
            continue;
        }
        if(typeof element.getUpdatedPathProxy!=='function')throw TypeError('Native charts support vector paths and text; images and HTML are unavailable');
        let clips=0;
        for(const clip of element.__clipPaths||[]){
            const segments=echartsPathSegments(clip.getUpdatedPathProxy());
            if(segments.length){clipPath(builder,id++,segments,clip.transform||identity);clips++;}
        }
        const segments=echartsPathSegments(element.getUpdatedPathProxy());
        if(segments.length){
            const path=id++,transform=element.transform||identity;
            builder.definePath(path,segments).pushTransform(transform);
            if(style.fill&&typeof style.fill==='object'){
                const gradient=style.fill,rect=element.getBoundingRect();
                if(gradient.type!=='linear'||gradient.global)throw TypeError('Native charts support local linear gradients');
                clipPath(builder,id++,segments,identity);
                const at=(x,y)=>[rect.x+x*rect.width,rect.y+y*rect.height];
                builder.linearGradient([rect.x,rect.y,Math.max(.01,rect.width),Math.max(.01,rect.height)],at(gradient.x,gradient.y),at(gradient.x2,gradient.y2),gradient.colorStops.map(stop=>({offset:stop.offset,color:rgba(stop.color,opacity*(style.fillOpacity??1))})));
                builder.popClip();
            }else{
                const fill=rgba(style.fill,opacity*(style.fillOpacity??1));if(fill!==null)builder.fillPath(path,fill);
            }
            const stroke=rgba(style.stroke,opacity*(style.strokeOpacity??1));
            if(stroke!==null&&(style.lineWidth??1)>0)builder.strokePath(path,style.lineWidth??1,stroke,{cap:style.lineCap==='round'?1:0,join:style.lineJoin==='round'?1:0});
            builder.popTransform();
        }
        while(clips--)builder.popClip();
    }
    return {frame:builder.finish(),labels};
}

export function echartsMarkMap(engine,pointSeries){
    const marks=new WeakMap(),items=[];
    engine.getModel().eachSeries(series=>{
        const data=series.getData(),points=pointSeries[series.seriesIndex]||[];
        data.each(index=>{
            const raw=data.getRawIndex(index),params=series.getDataParams(index),point=points[raw]||(!pointSeries.length?{name:params.seriesName||series.name,x:params.name,value:params.value,rows:[]}:null),element=data.getItemGraphicEl(index);
            if(!point||!element)return;
            const item={seriesIndex:series.seriesIndex,dataIndex:index,point,element};items.push(item);
            const assign=el=>{marks.set(el,item);if(el.isGroup)el.childrenRef().forEach(assign);};assign(element);
        });
    });
    return {items,hit(x,y){
        const found=engine.getZr().handler.findHover(x,y).target;
        for(let element=found;element;element=element.parent)if(marks.has(element))return marks.get(element);
        // Lines can suppress symbols at dense points. Nearest data geometry
        // still comes from ECharts; never invent an axis-to-index conversion.
        let nearest=null,distance=12;
        for(const item of items){const rect=transformedRect(item.element),d=Math.hypot(x-rect.x-rect.width/2,y-rect.y-rect.height/2);if(d<distance){nearest=item;distance=d;}}
        return nearest;
    }};
}
