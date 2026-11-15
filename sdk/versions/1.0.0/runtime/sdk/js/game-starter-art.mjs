import {decodeVisualImage} from './visual-resources.mjs';
import {GameSpriteAtlas} from './game-sprites.mjs';
import {readGameAsset} from './game-model.mjs';
import {openImageEngine} from './image-engine.mjs';
import {Label} from './controls.mjs';
import {createCancellationScope} from './cancellation.mjs';

/** Construct an atlas from this game's own pixels. The SDK supplies no artwork. */
export function createGameStarterAtlas(spec){
 if(!spec?.frames)throw Error('Generate artwork for this game with sdk.assets.generate, then supply its image and measured frames. No starter artwork is bundled.');
 return new GameSpriteAtlas({...spec,...(spec.image?decodeVisualImage(spec.image):{})});
}
createGameStarterAtlas.controlFactory=Object.freeze({topics:['games'],constructor:'createGameStarterAtlas({image:portableRgbaResource,frames,animations})',methods:'Builds an atlas from app-owned pixels and measured frame bounds. There is no default atlas, fixed theme, artwork download or cross-game artwork cache. Generate fresh game-specific art with sdk.assets.generate during authoring. For packaged PNG/JPEG use loadGameAtlas.'});

export async function loadGameAtlas({services,path,frames,animations={},decoderPath='image.wasm',signal,maximumWidth=1024,maximumHeight=1024,maximumPixels=1048576}={}){
 if(!frames||typeof frames!=='object')throw Error('Supply measured frame bounds for the generated game artwork');
 const bytes=await readGameAsset({services,path,signal});
 const decoder=await openImageEngine({services,path:decoderPath,signal});
 try{
  const image=await decoder.decode(bytes,{maximumWidth,maximumHeight,maximumPixels});
  if(signal?.aborted)throw signal.reason??Error('Artwork load cancelled');
  return new GameSpriteAtlas({...image,frames,animations});
 }finally{await decoder.close();}
}
loadGameAtlas.controlFactory=Object.freeze({topics:['games'],constructor:'await loadGameAtlas({services,path:"assets/game-atlas.png",frames,animations,decoderPath:"image.wasm",signal,maximumWidth:1024,maximumHeight:1024,maximumPixels:1048576})',methods:'Reads this app-owned packaged image and decodes through the approved image resource, closing the decoder on success/failure. Requires assets.stat/read and compute.open/invoke/cancel plus sdk.resources profile image. Measure frames at the decoded resolution. No network, generation call, shared artwork cache or fallback pack at runtime.'});

/** Open synchronously, load app-owned art, and reuse this window for the game. */
export function createGameArtworkWindow({createWindow,services,artwork,ready}){
 const message=new Label({text:'Preparing game artwork…'}),window=createWindow({root:message}),controller=createCancellationScope();
 let closed=false;window.onClose(()=>{closed=true;controller.abort(Error('Game closed'));});
 const show=atlas=>{if(!closed)ready({atlas,createWindow:({root})=>{window.setRoot(root);return window;}});};
 let loaded;
 try{
  if(!artwork)throw Error('This game draft needs newly generated artwork. Use sdk.assets.generate and supply the project asset and measured frames before running it.');
  if(artwork.image||artwork.rgba){show(createGameStarterAtlas(artwork));loaded=Promise.resolve();}
  else loaded=loadGameAtlas({...artwork,services,signal:controller.signal}).then(show);
 }catch(error){loaded=Promise.reject(error);}
 loaded=loaded.catch(error=>{if(!closed){message.text='Game artwork unavailable: '+error.message;window.setRoot(message);}return {error:error.message};});
 return {window,loaded};
}
createGameArtworkWindow.controlFactory=Object.freeze({topics:['games'],constructor:'createGameArtworkWindow({createWindow,services,artwork:{path,frames,animations},ready({atlas,createWindow}){ /* createGameView using this window */ }})',methods:'Creates one window before asynchronous loading, cancels on close and reports missing/failed game-owned artwork explicitly. No primitive or cached-art fallback. ready receives the decoded atlas and a factory reusing the original window.'});
