import { Message, SceneOverlay, Stack, State, TextEditor } from '@ricewm/app';
import { cellKey } from './table-values.mjs';
import { inlineCellBox } from './table-geometry.mjs';

// Navigation must run before TextEditorController's ordinary Tab insertion.
class CellEditor extends TextEditor {
    windowProc(event) {
        if(event.type===Message.KEY && event.detail?.pressed && this.routeKey?.(event)) return true;
        return super.windowProc(event);
    }
}

/** One retained native editor follows the selected cell; no sidebar draft. */
export function createInlineTableEditor({surface,viewport,change,navigate,exit}) {
    let active=null,size=null,pending=false,opened=false,transition=null,queued=[],pointerBounds=null;
    const editor=new CellEditor({name:'cell-editor',value:'',rows:1,wrap:true,state:State.EXPANSIVE,
        accessibleName:'Table cell text',enabled:true,readOnly:false});
    const overlay=new SceneOverlay({name:'cell-editor-overlay',surface,bounds:{x:0,y:0,width:1,height:1},visible:false},[editor]);
    const stack=new Stack({name:'slide-editor-stack',text:'panel:flush',state:State.EXPANSIVE});
    viewport.clip.remove(surface);stack.append(surface);stack.append(overlay);viewport.clip.append(stack);
    const style = () => {
        if(!active || !size) return false;
        const box=inlineCellBox(active,size);if(!box)return false;
        const {scaleX,scaleY,...bounds}=box;
        
        const text=active.cell.text, font=text?.style || {},layout=text?.layout||{};
        const color=font.color || '#000000', fill=active.cell.fill?.color || '#FFFFFF';
        editor.setProperty('documentFont',{family:font.fontFamily||'Arial',size_pt:(font.fontSize||20)*.75,weight:font.bold?700:400,italic:Boolean(font.italic),underline:Boolean(font.underline)});
        editor.setProperty('documentPaint',{foreground:color,background:fill});
        // SceneSurface's own native page transform determines placement. The
        // same view scale controls the native editor's document-space metrics.
        const zoom=viewport.dimensions().scale*Math.min(1,4096/Math.max(size.width,size.height))*100;
        const documentZoom=Math.max(1,Math.min(1600,zoom*scaleY||100));
        editor.setProperty('documentZoom',documentZoom);
        const screenScale=documentZoom/100;
        const margins={left:(layout.left??4)*scaleX/scaleY,right:(layout.right??4)*scaleX/scaleY,top:layout.top??4,bottom:layout.bottom??4};
        const inset=Math.max(0,Math.min(2,active.cell.bounds.width*screenScale*scaleX/scaleY/4,active.cell.bounds.height*screenScale/4,...Object.values(margins).map(value=>value*screenScale)));
        const ix=inset/(screenScale*scaleX/scaleY),iy=inset/screenScale,b=active.cell.bounds;
        pointerBounds={x:b.x+ix,y:b.y+iy,width:b.width-2*ix,height:b.height-2*iy};
        overlay.setBounds({...bounds,x:bounds.x+ix*scaleX/size.width,y:bounds.y+iy*scaleY/size.height,
            width:bounds.width-2*ix*scaleX/size.width,height:bounds.height-2*iy*scaleY/size.height});
        editor.setProperty('wrap',layout.wrap!==false);
        editor.setProperty('documentLayout',{left:margins.left-inset/screenScale,right:margins.right-inset/screenScale,
            top:margins.top-inset/screenScale,bottom:margins.bottom-inset/screenScale,align:font.align||'left',vertical:layout.vertical||'top',
            lineHeight:text?.paragraphs?.[0]?.lineHeight || {kind:'multiple',value:1}});
        return true;
    };
    const point = pointer => {
        if(!pointer?.point || !active) return;
        const b=pointerBounds||active.cell.bounds;
        editor.setSelectionAt({x:(pointer.point[0]-b.x)/b.width,y:(pointer.point[1]-b.y)/b.height,
            phase:pointer.phase||'click',extend:Boolean(pointer.extend)});
    };
    editor.on(Message.VALUE_CHANGED,event=>{if(opened)change?.(String(event.detail?.value??editor.getProperty('value')),pending);});
    function queue(action) {
        if(transition) return transition;
        transition=Promise.resolve().then(action).finally(()=>{
            transition=null;const keys=queued;queued=[];
            if(opened)for(const key of keys)editor.sendMessage(Message.KEY,key);
        });
        return transition;
    }
    editor.routeKey = event => {
        const key=event.detail;
        if(transition) {
            if(queued.length<512)queued.push({...key});
            event.preventDefault();return true;
        }
        if(key.key==='Tab'||key.keyCode===23) {event.preventDefault();void queue(()=>navigate?.(Boolean(key.modifiers&1)));return true;}
        if(key.key==='Escape'||key.keyCode===9) {event.preventDefault();void queue(()=>exit?.());return true;}
        return false;
    };
    viewport.onViewChanged(()=>{if(opened)style();});
    return Object.freeze({editor,overlay,stack,
        get active(){return opened;},get pending(){return pending;},get target(){return active;},get waiting(){return transition;},
        get key(){return active?cellKey(active.object.id,active.row,active.column):null;},
        get value(){return editor.getProperty('value');},
        begin(target,slideSize,{pointer=null,value=target.cell.text?.text||'',preserve=false}={}) {
            const text=target?.cell?.text;
            if(!inlineCellBox(target,slideSize) || Math.abs(target.object.transform[0]-target.object.transform[3])>1e-6 || text?.mixed || (text?.style?.opacity??1)!==1 || (target.cell.fill?.opacity??1)!==1) {
                this.close();return false;
            }
            active=target;size=slideSize;pending=false;
            try {if(!style()){this.close();return false;}} catch {this.close();return false;}
            if(!preserve)editor.setProperty('value',value);
            opened=true;overlay.setProperty('visible',true);surface.setProperty('focused',false);editor.setProperty('focused',true);
            point(pointer);return true;
        },
        beginPending(bounds,slideSize,columns,rows) {
            const target={object:{id:'pending-table',transform:[1,0,0,1,0,0]},row:0,column:0,cell:{editable:true,
                bounds:{x:bounds.x,y:bounds.y,width:bounds.width/columns,height:bounds.height/rows},fill:{color:'#FFFFFF',opacity:1},
                text:{text:'',style:{fontFamily:'Arial',fontSize:20,color:'#000000'},layout:{left:4,right:4,top:4,bottom:4,vertical:'top',wrap:true}}}};
            this.begin(target,slideSize);pending=true;
        },
        refresh(target,slideSize) {if(opened&&!pending&&target){active=target;size=slideSize;style();}},
        input(keys) {for(const key of keys)editor.sendMessage(Message.KEY,key);},
        selectPoint:point,
        close() {opened=false;pending=false;active=null;queued=[];editor.setProperty('focused',false);overlay.setProperty('visible',false);},
        queue,
    });
}
