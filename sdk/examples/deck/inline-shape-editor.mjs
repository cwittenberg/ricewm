import {SceneTextEditor,ShapedSceneTextEditor,SceneBuilder,TextRunDraft,textRangeFromSelection,textInsertTokens,Message} from '@ricewm/app';
import {transformRect,transformPoint} from './affine.mjs';
import {paragraphSpace} from './paragraph-layout.mjs';

const intersects=(a,b)=>a[0]<b[0]+b[2]&&b[0]<a[0]+a[2]&&a[1]<b[1]+b[3]&&b[1]<a[1]+a[3];
const objectRect=object=>transformRect(object.transform,[object.bounds.x||0,object.bounds.y||0,object.bounds.width,object.bounds.height]);

/** Resolve supported native editor geometry from source; never repaint a
 * transparent text box with a guessed desktop background or flatten rich text. */
export function shapeEditorPresentation(object,scene,scale=1) {
    const text=object?.text,b=object?.bounds,m=object?.transform,size=scene?.size;
    // These diagnostics also apply to the retained preview. Editing its resolved
    // uniform layout does not claim to implement source autofit, and never
    // removes the original source policy or its diagnostic.
    if(!text?.editable||!b||!m||!size||b.width<=0||b.height<=0||object.parentId||object.outerShadow||text.unsupported?.some(code=>!['TEXT_AUTOFIT_PENDING','PARAGRAPH_LAYOUT_PENDING'].includes(code)))return null;
    const [a,bx,c,d,e,f]=m;
    if(Math.abs(bx)>1e-6||Math.abs(c)>1e-6||a<=0||d<=0||Math.abs(a-d)>1e-6||text.mixed||(text.style?.opacity??1)!==1)return null;
    const font=text.style||{},layout=text.layout||{};
    const paragraphs=text.paragraphs||[],lineHeight=paragraphs[0]?.lineHeight||{kind:'normal',value:1};
    if(paragraphs.some(p=>JSON.stringify(p.lineHeight||{kind:'normal',value:1})!==JSON.stringify(lineHeight)))return null;
    // Apply the same first/last paragraph spacing policy as the vector preview.
    const spacing=paragraphs.map((p,index)=>({
        before:index===0&&layout.useParagraphSpacing===false?0:paragraphSpace(p.spaceBefore,font.fontSize||20),
        after:index===paragraphs.length-1&&layout.useParagraphSpacing===false?0:paragraphSpace(p.spaceAfter,font.fontSize||20),
    }));
    if(paragraphs.length>1&&spacing.some(p=>p.before||p.after))return null;
    const before=spacing[0]?.before||0,after=spacing[0]?.after||0;
    let background;
    if(object.fill?.color&&(object.fill.opacity??1)===1&&object.geometry==='rect')background=object.fill.color;
    else{
        if(object.fill&&(object.fill.opacity??1)!==0)return null;
        if(scene.backgroundPicture||scene.backgroundGradient||['gradient','pattern','picture'].includes(scene.backgroundKind)||['gradient','pattern','picture'].includes(scene.backgroundFill?.kind))return null;
        background=scene.background?.color||'#FFFFFF';
        if(scene.background&&(scene.background.opacity??1)!==1)return null;
        const box=objectRect(object);
        for(const other of scene.objects||[]){
            if(other.id===object.id)break;
            if(other.kind==='group'||(!other.fill&&!other.stroke&&!other.image&&!other.text?.text))continue;
            if(other.bounds&&other.transform&&intersects(box,objectRect(other)))return null;
        }
    }
    return {
        id:object.id,value:text.text,
        bounds:{x:(a*(b.x||0)+e)/size.width,y:(d*(b.y||0)+f)/size.height,width:a*b.width/size.width,height:d*b.height/size.height},
        font:{family:font.fontFamily||'Liberation Sans',size_pt:(font.fontSize||20)*.75,weight:font.bold?700:400,italic:Boolean(font.italic),underline:Boolean(font.underline)},
        paint:{foreground:font.color||'#000000',background},wrap:layout.wrap!==false,
        layout:{left:layout.left??0,right:layout.right??0,top:(layout.top??0)+before,bottom:(layout.bottom??0)+after,align:['center','right'].includes(font.align)?font.align:'left',vertical:layout.vertical||'top',lineHeight},
        zoom:Math.max(1,Math.min(1600,scale*Math.min(1,4096/Math.max(size.width,size.height))*d*100)),
    };
}

export function richShapeEditorPresentation(object,scene) {
    if(!object?.text?.rangeEditable||object.parentId||object.outerShadow||!scene?.size||!object.bounds||!object.transform)return null;
    // Existing stock-layout text can retain this inheritance warning after an
    // empty paragraph was saved without a local run. The native source owner
    // still admits range edits; use its actual glyph map and retain the warning
    // instead of handing the textbox to a different editor.
    if(object.text.unsupported?.some(code=>!['MIXED_TEXT_STYLE','TEXT_AUTOFIT_PENDING','PARAGRAPH_LAYOUT_PENDING','MISSING_MASTER_PLACEHOLDER'].includes(code)))return null;
    const box=objectRect(object),[x,y,w,h]=box;
    if(!box.every(Number.isFinite)||w<=0||h<=0)return null;
    const factor=Math.min(1,4096/Math.max(w,h)),width=Math.max(1,Math.ceil(w*factor)),height=Math.max(1,Math.ceil(h*factor));
    const m=object.transform,transform=[m[0]*factor,m[1]*factor,m[2]*factor,m[3]*factor,(m[4]-x)*factor,(m[5]-y)*factor];
    const font=object.text.style||{};
    return {id:object.id,value:object.text.text,
        bounds:{x:x/scene.size.width,y:y/scene.size.height,width:w/scene.size.width,height:h/scene.size.height},
        font:{family:font.fontFamily||'Liberation Sans',size_pt:(font.fontSize||20)*.75,weight:font.bold?700:400,italic:Boolean(font.italic),underline:Boolean(font.underline)},
        paint:null,width,height,transform,
        mapPoint(pointer){if(!pointer)return null;const b=object.bounds,p=transformPoint(transform,[(b.x||0)+pointer.x*b.width,(b.y||0)+pointer.y*b.height]);return {...pointer,x:p[0]/width,y:p[1]/height};},
    };
}

export function createInlineShapeEditor({surface,viewport,stack,change,exit,notice,renderText,selectionChanged=()=>{},navigate=null,onParagraphFormat=null,
    namePrefix='shape',accessibleName='Shape text',rangeTarget=target=>({object:target.id}),clipText=false}) {
    let object=null,scene=null,mode='plain',opening=null,entryKeys=[],entryPointer=null,draft=null,commands=[],sequence=0,typingStyle={},typingAt=null,insertionAt=null,formatPending=0;
    const overlay=new SceneTextEditor({name:namePrefix+'-text-editor',surface,tabBehavior:navigate?'navigate':'insert',onChange:change,onCommit:exit,onNavigate:navigate,onError:error=>notice?.(error.message)});
    overlay.editor.setProperty('accessibleName',accessibleName);stack.append(overlay);
    const rich=new ShapedSceneTextEditor({name:namePrefix+'-rich-text-editor',surface,tabBehavior:navigate?'navigate':'insert',onCommit:exit,onNavigate:navigate,onError:error=>notice?.(error.message),
        onEditKey(event){
            if(navigate||!onParagraphFormat||!draft||!object?.text?.paragraphFormatEditable)return false;
            const key=event.detail;if(key.modifiers&12)return false;
            const range=textRangeFromSelection(draft.text,rich.editor.selection),rows=draft.paragraphs(range),listed=rows.length&&rows.every(row=>['bullet','number'].includes(row.paragraphFormat?.list?.kind));
            if(!listed)return false;
            let format=null;
            if(key.keyCode===23||key.key==='Tab')format={levelDelta:key.modifiers&1?-1:1};
            else if(!(key.modifiers&1)&&(key.keyCode===36||key.key==='Enter')&&range.start===range.end&&rows.length===1&&rows[0].start===rows[0].end)format={list:{kind:'none'},level:0};
            else if(key.keyCode===22&&range.start===range.end&&rows.length===1&&range.start===rows[0].start)format=rows[0].paragraphFormat.level?{levelDelta:-1}:{list:{kind:'none'}};
            if(!format)return false;
            Promise.resolve(onParagraphFormat(format)).catch(error=>notice?.(error.message));return true;
        },
        onChange(value,edit){
            if(!draft||edit?.unit!=='unicode-scalar')throw Error('Rich input requires its source range');
            const range=textRangeFromSelection(draft.text,{anchor:edit.range.start,caret:edit.range.end});
            const handle=rich.shapedHandle,stop=handle?.layout.caret(handle.selection.caret);
            // After replacing a selection, the native caret can have downstream
            // affinity at the untouched next run. Consecutive typing belongs to
            // the text just inserted until an actual selection move occurs.
            const continuing=range.start===range.end&&insertionAt===JSON.stringify({anchor:edit.range.start,caret:edit.range.end});
            const command={type:'replace-text-range',slide:scene.slideId,...rangeTarget(object),range,affinity:continuing?'before':range.start!==range.end||stop?.affinity==='downstream'?'after':'before',
                insert:textInsertTokens(edit.text,{breakKind:edit.breakKind||'paragraphBreak',style:Object.keys(typingStyle).length?typingStyle:null})};
            draft.replace(command);if(draft.text!==value)throw Error('Native input and source range differ');
            commands.push({sequence:++sequence,command});typingAt=selectionKey();insertionAt=edit.text?typingAt:null;change(value,edit);selectionChanged();
        },
        async renderText({value}){
            if(!draft||draft.text!==value)throw Error('Rich glyph source changed');
            const target=object,source=scene,appearance=richShapeEditorPresentation(target,source),text=draft.snapshot();
            if(!appearance)throw Error('Rich text geometry is unavailable');
            const result=await renderText({...source,objects:[{...target,text}]},{editing:true}),frame=result.frames.get(target.id);
            if(!frame?.editing)throw Error(result.diagnostics?.[0]?.message||'Native editing geometry is unavailable');
            const builder=new SceneBuilder(appearance.width,appearance.height).pushTransform(appearance.transform);
            if(clipText)builder.pushClip([target.bounds.x||0,target.bounds.y||0,target.bounds.width,target.bounds.height]);
            builder.append(frame.displayList);if(clipText)builder.popClip();
            const rendered=builder.popTransform().finish();
            return {frame:rendered,map:frame.editing,presentation:{width:appearance.width,height:appearance.height,transform:appearance.transform,caretColor:text.style.color||'#000000'}};
        }});
    rich.editor.setProperty('accessibleName',accessibleName);stack.append(rich);
    const current=()=>mode==='rich'?rich:overlay;
    const selectionKey=()=>JSON.stringify(rich.editor.selection);
    const moved=()=>{if(mode!=='rich'||!rich.active)return;if(typingAt!==selectionKey()){typingStyle={};typingAt=null;insertionAt=null;}selectionChanged();};
    rich.editor.on(Message.TEXT_SELECTION,()=>{typingStyle={};typingAt=null;insertionAt=null;selectionChanged();});rich.editor.on(Message.KEY,moved);
    const config=()=>shapeEditorPresentation(object,scene,viewport.dimensions().scale);
    viewport.onViewChanged(()=>{if(current().active){const value=mode==='rich'?richShapeEditorPresentation(object,scene):config();if(value)current().refresh(value);}});
    function rebase(target,nextScene){
        const next=new TextRunDraft(target.text);for(const item of commands)next.replace(item.command);
        object=target;scene=nextScene;draft=next;
    }
    return Object.freeze({
        get overlay(){return current();},get editor(){return current().editor;},editors:Object.freeze([overlay.editor,rich.editor]),
        get active(){return Boolean(opening)||current().active;},get key(){return opening?object?.id:current().key;},get value(){return current().value;},
        get rich(){return mode==='rich'&&(Boolean(opening)||rich.active);},get hasPending(){return commands.length>0;},get formatting(){return formatPending>0;},
        get layoutWaiting(){return opening||rich.layoutWork||rich.shapedHandle?.pending;},
        get range(){return mode==='rich'&&!opening&&draft&&rich.active&&rich.value===draft.text?textRangeFromSelection(draft.text,rich.editor.selection):null;},
        get typingStyle(){return {...typingStyle};},
        get paragraphFormatting(){return Boolean(object?.text?.paragraphFormatEditable);},
        paragraphs(){return mode==='rich'&&!opening&&draft&&rich.active?draft.paragraphs(textRangeFromSelection(draft.text,rich.editor.selection)):[];},
        get rangeTarget(){return object?{slide:scene.slideId,...rangeTarget(object)}:null;},
        setSelection(anchor,caret=anchor){const changed=current().setSelection(anchor,caret);if(changed)moved();return changed;},
        selectPoint(pointer){if(mode==='rich'){const point=richShapeEditorPresentation(object,scene)?.mapPoint(pointer);return point?rich.setSelectionAt(point):false;}return pointer?overlay.setSelectionAt(pointer):false;},
        input(keys){
            if(opening){for(const key of keys)if(entryKeys.length<512&&entryKeys.reduce((n,k)=>n+(k.text?.length||0),0)+(key.text?.length||0)<=16000)entryKeys.push({...key});}
            else current().input(keys);
        },
        styles(){if(mode!=='rich'||opening||!draft||!rich.active||rich.value!==draft.text)return null;const range=textRangeFromSelection(draft.text,rich.editor.selection),handle=rich.shapedHandle,continuing=range.start===range.end&&insertionAt===selectionKey(),affinity=!continuing&&handle?.layout.text===draft.text&&handle.layout.caret(handle.selection.caret).affinity==='downstream'?'after':'before';return draft.styles(range,{affinity}).map(style=>range.start===range.end?{...style,...typingStyle}:style);},
        capture(){const batch=commands.slice(0,32);return batch.length?{through:batch.at(-1).sequence,commands:batch.map(item=>structuredClone(item.command))}:null;},
        acknowledge(batch){commands=commands.filter(item=>item.sequence>batch.through);},
        discard(){commands=[];typingStyle={};typingAt=null;insertionAt=null;if(mode==='rich'&&object){draft=new TextRunDraft(object.text);rich.editor.value=draft.text;return rich.refreshText();}},
        format(action){
            if(mode!=='rich'||!rich.active)return Promise.resolve(false);
            // Claim the editor through the SDK's single-focus-owner rule.
            // A synthetic blur edge can race with keys already in transit.
            rich.editor.setProperty('focused',true);
            formatPending++;selectionChanged();let finished=false;
            const finish=()=>{if(!finished){finished=true;formatPending--;selectionChanged();}};
            return rich.runAction(async()=>{try{return await action();}finally{finish();}}).finally(finish);
        },
        setTypingStyle(style){typingStyle={...typingStyle,...style};typingAt=selectionKey();selectionChanged();},
        begin(target,nextScene,{pointer=null,keys=[],preserve=false,value=target.text?.text||''}={}) {
            if(opening&&target.id===object?.id){entryKeys.push(...keys);if(pointer)entryPointer={...pointer};return opening;}
            const richAppearance=renderText&&richShapeEditorPresentation(target,nextScene);
            if(richAppearance){
                object=target;scene=nextScene;mode='rich';overlay.close();
                if(preserve&&rich.active&&rich.key===target.id){if(pointer)this.selectPoint(pointer);rich.input(keys);return true;}
                draft=new TextRunDraft(target.text);commands=[];typingStyle={};typingAt=null;insertionAt=null;entryKeys=[...keys];entryPointer=null;
                const point=richAppearance.mapPoint(pointer);
                opening=rich.open({...richAppearance,...(point?{point}:{})}).then(ok=>{if(ok){
                    if(entryPointer){const pointer=richAppearance.mapPoint(entryPointer);entryPointer=null;if(pointer)rich.setSelectionAt(pointer);}
                    const pending=entryKeys;entryKeys=[];rich.input(pending);
                }return Boolean(ok);}).finally(()=>{opening=null;});
                return opening;
            }
            const appearance=shapeEditorPresentation(target,nextScene,viewport.dimensions().scale);
            if(!appearance)return false;
            object=target;scene=nextScene;mode='plain';rich.close();
            try{
                if(preserve&&overlay.active&&overlay.key===target.id)overlay.refresh(appearance);
                else overlay.open({...appearance,value,...(pointer?{point:pointer}:{})});
            }catch(error){if(error instanceof TypeError||error instanceof RangeError)return false;throw error;}
            overlay.input(keys);return true;
        },
        refresh(target,nextScene) {
            if(mode==='rich'){
                if(!rich.active||target?.id!==rich.key||!richShapeEditorPresentation(target,nextScene))return false;
                rebase(target,nextScene);const selection=rich.editor.selection;
                if(rich.value!==draft.text){rich.editor.value=draft.text;const end=Array.from(draft.text).length;rich.editor.setSelection(Math.min(selection.anchor,end),Math.min(selection.caret,end));}
                rich.refresh(richShapeEditorPresentation(target,nextScene));void rich.refreshText();return true;
            }
            if(!overlay.active||target?.id!==overlay.key)return false;
            const appearance=shapeEditorPresentation(target,nextScene,viewport.dimensions().scale);
            if(!appearance)return false;
            object=target;scene=nextScene;return overlay.refresh(appearance);
        },
        close(){object=null;scene=null;commands=[];draft=null;typingStyle={};typingAt=null;insertionAt=null;entryKeys=[];entryPointer=null;opening=null;overlay.close();rich.close();},
    });
}
