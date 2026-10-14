import {documentEditorMetadata,isDocumentEditor} from './rice-document-editor-layout.mjs';

/** A one-shot editor-local pointer request. It traverses the same measured
 * native pointer ingress as real input, including selection drag capture. */
export class DocumentEditorPointRequests {
    constructor(bridge){this.bridge=bridge;this.seen=new Map();this.gestures=new Map();this.owners=new Map();this.replaying=false;}
    pointer(app,window,kind,x,y,button,modifiers){
        if(this.replaying)return;
        const key=`${app}:${window}`,previous=this.gestures.get(key);
        if(kind===1&&button===1){
            this.gestures.set(key,{generation:(previous?.generation||0)+1,down:true,x,y,modifiers});
        }else if(previous&&[0,2,4].includes(kind)){
            this.gestures.set(key,{...previous,x,y,modifiers,...(kind===4?{cancelled:true}:{}),
                ...((kind===2&&button===1)||kind===4?{down:false,release:{x,y}}:{})});
        }
        if(this.gestures.size>64)this.gestures.delete(this.gestures.keys().next().value);
    }
    releaseSuppressed(app,window,x,y,button,modifiers){
        // A scene press can create the editor before its physical release.
        // The controller must still suppress release into unrelated controls,
        // but deferred editor replay needs the actual button state.
        this.pointer(app,window,2,x,y,button,modifiers);
        if(button!==1)return [];
        const key=`${app}:${window}`,target=this.bridge.editorPointer?.capture.get(key);
        const snapshot=this.bridge.editorPointer?.snapshots.get(key)?.snapshot;
        const documentCapture=target&&snapshot?.commands.some(command=>isDocumentEditor(command)&&documentEditorMetadata(command)?.id===target);
        if(!documentCapture||this.bridge.hitTest(app,window,x,y)!==target)return [];
        return this.bridge.pointer(app,window,'up',x,y,button,modifiers);
    }
    observe(snapshot){
        const {bridge}=this;if(!bridge.supportsMeasuredEditor)return;
        const owner=String(snapshot.app);
        if(!this.owners.has(owner))this.owners.set(owner,{});
        const lifetime=this.owners.get(owner);
        for(const command of snapshot.commands){
            if(!isDocumentEditor(command))continue;
            const metadata=documentEditorMetadata(command),request=metadata?.point;if(!request||!metadata.id)continue;
            const {rect}=command,x=Math.max(rect.x,Math.min(rect.x+rect.width-1,rect.x+rect.width*request.x)),y=Math.max(rect.y,Math.min(rect.y+rect.height-1,rect.y+rect.height*request.y));
            const target=metadata.id,hit=bridge.hitTest(snapshot.app,snapshot.window,x,y),capture=bridge.editorPointer.capture.get(`${snapshot.app}:${snapshot.window}`);
            if(hit!==target&&(!['move','up'].includes(request.phase)||capture!==target))continue;
            const key=`${snapshot.app}:${snapshot.window}:${target}`;
            if((this.seen.get(key)||0)>=request.revision)continue;
            this.seen.set(key,request.revision);
            if(this.seen.size>512)this.seen.delete(this.seen.keys().next().value);
            const gestureKey=`${snapshot.app}:${snapshot.window}`,gesture=this.gestures.get(gestureKey);
            void bridge.prepareEditorPointer(snapshot.app,snapshot.window).then(()=>{
                if(this.owners.get(owner)!==lifetime)return;
                const retained=bridge.editorPointer.snapshots.get(`${snapshot.app}:${snapshot.window}`)?.snapshot;
                const current=retained?.commands.find(value=>isDocumentEditor(value)&&documentEditorMetadata(value)?.point?.revision===request.revision&&value.text===command.text);
                if(!current)return;
                const latest=this.gestures.get(gestureKey);
                if(latest?.cancelled||(latest?.generation||0)!==(gesture?.generation||0))return;
                const captured=bridge.editorPointer.capture.get(`${snapshot.app}:${snapshot.window}`)===target;
                const commands=[];
                // A bare hover is not a drag. Only an acknowledged native
                // capture or an explicit retained down origin can extend it.
                if(request.phase==='move'&&!captured&&!request.origin)return;
                // Actor patches may coalesce down/move/up before the editor
                // exists in native UI. Preserve the original down anchor and
                // establish capture before replaying the final pointer phase.
                if(['move','up'].includes(request.phase)&&!captured){
                    const origin=request.origin||request;
                    commands.push({kind:'down',x:Math.max(rect.x,Math.min(rect.x+rect.width-1,rect.x+rect.width*origin.x)),y:Math.max(rect.y,Math.min(rect.y+rect.height-1,rect.y+rect.height*origin.y)),extend:origin.extend});
                }
                for(const kind of request.phase==='click'?['down','up']:request.phase==='up'?['move','up']:[request.phase])commands.push({kind,x,y,extend:request.extend});
                const released=latest?.down===false;
                if(released&&['down','move'].includes(request.phase)){
                    const release=latest.release||{x,y};
                    commands.push({kind:'move',...release,extend:request.extend},{kind:'up',...release,extend:request.extend});
                }
                this.replaying=true;
                try{
                    for(const point of commands){
                        const sample=bridge.editorPointer.sample(bridge,snapshot.app,snapshot.window,point.kind==='move'?0:point.kind==='up'?2:1,point.x,point.y,1);
                        if(!sample||sample.target!==target)return;
                        const events=bridge.pointer(snapshot.app,snapshot.window,point.kind,point.x,point.y,1,point.extend?1:0);if(events.length)bridge.onRenderEvents?.(events);
                    }
                    // Replay must not move hover back to a stale click if the
                    // physical pointer already reached a divider or sibling.
                    if(released){const events=bridge.pointer(snapshot.app,snapshot.window,'move',latest.x,latest.y,0,latest.modifiers);if(events.length)bridge.onRenderEvents?.(events);}
                }finally{
                    this.replaying=false;
                }
            }).catch(error=>{if(this.owners.get(owner)===lifetime)bridge.onDocumentEditorError?.(error);});
        }
    }
    removeApp(app){this.owners.delete(String(app));for(const map of [this.seen,this.gestures])for(const key of map.keys())if(key.startsWith(`${app}:`))map.delete(key);}
}
