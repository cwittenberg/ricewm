import {checkedComposition,COMPOSE_LAYOUTS} from './copilot-compose.mjs';
import {checkedRequest} from './engine-contract.mjs';
import {layouts} from './layout-catalog.mjs';

// These examples use the same validators as the UI and native document port.
// Placeholder identities are descriptions, never invented document IDs.
const slide='existing slide ID',object='existing object ID';
export const deckCommandContracts={
 'insert-slide':{example:{type:'insert-slide',after:slide,layout:'blank'},fields:'after: existing slide ID or null (beginning); layout: ID from layouts. Read result.createdSlides before editing the new slide.'},
 'duplicate-slide':{example:{type:'duplicate-slide',slide}},
 'delete-slide':{example:{type:'delete-slide',slide}},
 'move-slide':{example:{type:'move-slide',slide,before:null}},
 'set-slide-layout':{example:{type:'set-slide-layout',slide,layout:'blank'}},
 'set-slide-size':{example:{type:'set-slide-size',width:1280,height:720},fields:'positive numbers up to 5376, in document coordinates'},
 'set-slide-background':{example:{type:'set-slide-background',slide,color:'#F4F0E5'}},
 'insert-object':{example:{type:'insert-object',slide,kind:'text',box:{x:80,y:60,width:900,height:100},text:'Title',fill:null,stroke:null},fields:'kind: text, rect, ellipse, line or shape catalog ID; box: x,y,width,height in scene coordinates, width and height positive; optional text (32768 characters), fill (#RRGGBB or null), stroke ({color:#RRGGBB,width:number} or null). Read result.created before styling the new object.'},
 'set-object-text':{example:{type:'set-object-text',slide,object,text:'Title'},fields:'Replaces the entire text in a real editable object, up to 32768 characters.'},
 'set-text':{example:{type:'set-text',slide,run:'existing projection text run ID',text:'Title'}},
 'set-text-style':{example:{type:'set-text-style',slide,object,style:{fontFamily:'Arial',fontSize:44,bold:true,color:'#24483E',align:'left'}},fields:'style has one or more of fontFamily (1–256 characters), fontSize (1–512), bold/italic/underline (boolean), color (#RRGGBB), align (left|center|right). No other style keys. Apply to text objects.'},
 'set-text-layout':{example:{type:'set-text-layout',slide,object,layout:{left:0,top:0,right:0,bottom:0,vertical:'middle',wrap:true}},fields:'optional nonnegative inset numbers left/top/right/bottom; vertical: top|middle|bottom; wrap:boolean. At least one field.'},
 'set-object-box':{example:{type:'set-object-box',slide,object,box:{x:80,y:60,width:900,height:100}}},
 'set-object-fill':{example:{type:'set-object-fill',slide,object,fill:'#24483E'}},
 'set-object-stroke':{example:{type:'set-object-stroke',slide,object,stroke:{color:'#24483E',width:2}}},
 'set-object-rotation':{example:{type:'set-object-rotation',slide,object,degrees:0}},
 'delete-object':{example:{type:'delete-object',slide,object}},
 'reorder-objects':{example:{type:'reorder-objects',slide,objects:[object],direction:'front'},fields:'direction: front|back|forward|backward; 1–32 distinct real object IDs'},
 'set-slide-transition':{example:{type:'set-slide-transition',slide,effect:'fade',direction:'left',durationMs:450,advanceOnClick:true,advanceAfterMs:null},fields:'effect: none|fade|fade-black|push|wipe|morph; direction:left|right|up|down; durations 0–600000 ms'},
 'set-speaker-notes':{example:{type:'set-speaker-notes',slide,text:'Explain the main point.'},fields:'UTF-8 text up to 16 KiB; no control characters except tab/newline'},
};
export function deckCopilotActions(){return {compose:{description:'Generate a complete presentation in ONE call. Native IDs, varied layouts, typography, visual aids and transitions are resolved locally. Preserves existing slides; reuses only the untouched initial slide. Pictures are fetched from Wikipedia for imageTopic; failures are reported. Do not build a new deck with many apply calls.',slides:'1–12 objects: {title:up to 110 chars,points:1–4 strings up to 220 chars each,notes:up to 2000 chars,layout:optional,imageTopic:optional English Wikipedia article title}',layouts:COMPOSE_LAYOUTS,accent:'optional #RRGGBB'},apply:{description:'Apply 1–32 commands as a checked native batch. Only exact fields are accepted. Use actual identities from scene, outline or createdSlides/created. Never refer to an identity that has not been returned. Geometry uses the scene width/height, not normalized fractions.',commands:deckCommandContracts,layouts:layouts()},undo:{},redo:{}};}
export function validateDeckCopilot(operation,data,revision){
 if(operation==='compose'){try{checkedComposition(data);return {valid:true,revision};}catch(error){return {valid:false,revision,rejectedBeforeMutation:true,error:{code:error.code,message:error.message}};}}
 if(operation!=='apply')return {valid:['undo','redo'].includes(operation),revision};
 try{
  // Validate each command separately to identify its index and correction.
  if(!Array.isArray(data?.commands)||data.commands.length<1||data.commands.length>32)throw Error('Provide data.commands with 1–32 commands');
  for(const [index,command] of data.commands.entries()){
   try{checkedRequest('apply',revision,{commands:[command]});}
   catch(error){
    const suggested=command?.type==='set-object-style'?'set-text-style':command?.type;
    const contract=deckCommandContracts[suggested];
    return {valid:false,revision,rejectedBeforeMutation:true,error:{code:error.code||'INVALID_REQUEST',index,type:command?.type,message:error.message,...(contract?{suggestedType:suggested,contract}:{supportedTypes:Object.keys(deckCommandContracts)})}};
   }
  }
  checkedRequest('apply',revision,{commands:data.commands});
  return {valid:true,revision,notice:'Syntax validated. The native engine checks document identities and editability when applying.'};
 }catch(error){return {valid:false,revision,rejectedBeforeMutation:true,error:{code:error.code||'INVALID_REQUEST',message:error.message}};}
}
