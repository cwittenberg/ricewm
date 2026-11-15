import {checkedPublicHttpsUrl} from './network-origin-policy.mjs';

export const READER_SURFACE_OPERATIONS=Object.freeze(['showDocument','navigate','back','forward','stop','find','restorePosition','state','imageSources','displayImage','setZoom','decodeSiteIcon','registerSiteIcon']);
const bounded=(value,max,label)=>{if(typeof value!=='string'||value.length>max||value.includes('\0'))throw TypeError(`Invalid ${label}`);return value;};
export function checkedReaderOperation(operation,input={}) {
    if(!READER_SURFACE_OPERATIONS.includes(operation)||!input||typeof input!=='object'||Array.isArray(input))throw TypeError('Unknown reader operation');
    const fields={showDocument:['id','title','source','author','date','html','text','textSize','paper','position','zoom'],navigate:['url','generation'],
        back:[],forward:[],stop:[],find:['text','backwards'],restorePosition:['fraction'],state:[],setZoom:['percent'],decodeSiteIcon:['bytes'],registerSiteIcon:['id','rgba'],imageSources:[],displayImage:['source','mimeType','bytes','id']}[operation];
    if(Object.keys(input).some(key=>!fields.includes(key)))throw TypeError('Unknown reader operation field');
    if(operation==='showDocument') {
        const id=bounded(input.id,128,'article identity'),title=bounded(input.title||'',2048,'article title');
        const html=bounded(input.html||'',512*1024,'article content'),text=bounded(input.text||'',512*1024,'article text');
        const source=input.source?checkedPublicHttpsUrl(input.source).href:'';
        const textSize=input.textSize??18;
        if(!Number.isFinite(textSize)||textSize<16||textSize>24)throw RangeError('Reader text size must be between 16 and 24');
        if(input.paper!==undefined&&typeof input.paper!=='boolean')throw TypeError('Invalid reading surface');
        return {id,title,html,text,source,textSize,zoom:checkedReaderOperation('setZoom',{percent:input.zoom??100}).percent,paper:input.paper===true,
            author:bounded(input.author||'',1024,'article author'),date:bounded(input.date||'',128,'article date'),
            position:checkedReaderOperation('restorePosition',{fraction:input.position??0}).fraction};
    }
    if(operation==='navigate')return {url:checkedPublicHttpsUrl(input.url).href,generation:bounded(input.generation||'',128,'document generation')};
    if(operation==='find')return {text:bounded(input.text||'',256,'find text'),backwards:input.backwards===true};
    if(operation==='setZoom'){if(!Number.isFinite(input.percent)||input.percent<75||input.percent>200)throw TypeError('Article zoom must be 75–200%');return {percent:input.percent};}
    if(operation==='registerSiteIcon'){if(!(input.rgba instanceof Uint8Array)||input.rgba.length!==4096)throw TypeError('Site icon must be 32-pixel RGBA');return {id:bounded(input.id,160,'feed identity'),rgba:input.rgba};}
    if(operation==='decodeSiteIcon'){if(!(input.bytes instanceof Uint8Array)||input.bytes.length<6||input.bytes.length>128*1024)throw TypeError('Site icon is oversized');return {bytes:input.bytes};}
    if(operation==='displayImage'){
        const source=checkedPublicHttpsUrl(input.source).href;
        if(!['image/png','image/jpeg','image/gif','image/webp','image/avif','image/bmp','image/x-icon','image/vnd.microsoft.icon'].includes(input.mimeType)||
            !(input.bytes instanceof Uint8Array)||input.bytes.length>2*1024*1024||!input.bytes.length)throw TypeError('Article image exceeds its type or byte limits');
        return {source,mimeType:input.mimeType,bytes:input.bytes,id:bounded(input.id,128,'article identity')};
    }
    if(operation==='restorePosition') {
        if(!Number.isFinite(input.fraction)||input.fraction<0||input.fraction>1)throw TypeError('Invalid reading position');
        return {fraction:input.fraction};
    }
    return {};
}
