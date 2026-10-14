/** Bounded native PNG/JPEG/vector-SVG decoder through the generic approved compute host. */
const CAPACITY=65536, MAX_ENCODED=8*1024*1024, MAX_PIXELS=2*1024*1024;
const EXPORTS=['image_abi_version','image_buffer_capacity','image_buffer_ptr','image_begin','image_write','image_decode','image_output_read'];
const ERRORS=['','INCOMPLETE_INPUT','INVALID_LIMITS','ENCODED_LIMIT','UNSUPPORTED_FORMAT','ANIMATED_IMAGE','INVALID_IMAGE','DIMENSION_LIMIT','DECODED_LIMIT','SNAPSHOT_LIMIT'];
const failure=(message,code='IMAGE_WORKER')=>Object.assign(new Error(`Image: ${message}`),{code});
export async function openImageEngine({services,path='image.wasm',signal}={}) {
    if(!services?.compute || typeof path!=='string' || !/^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}$/.test(path))throw failure('approved package compute is required');
    let handle,pointer,closed=false,pending=false;
    const alive=()=>{if(closed||signal?.aborted)throw failure('worker closed','CLOSED');};
    const invoke=(name,args=[],extra={})=>services.compute.invoke({abi:1,handle,name,args,timeoutMs:5000,...extra});
    async function close(){closed=true;signal?.removeEventListener('abort',abort);const owned=handle;handle=undefined;if(owned)await services.compute.cancel({handle:owned});}
    function abort(){void close().catch(()=>{});}
    signal?.addEventListener('abort',abort,{once:true});
    try{
        alive();const opened=await services.compute.open({abi:1,path,timeoutMs:5000});handle=opened?.handle;alive();
        if(typeof handle!=='string'||opened.abi!==1||!Array.isArray(opened.exports)||EXPORTS.some(name=>!opened.exports.includes(name)))throw failure('incompatible ABI');
        if((await invoke('image_abi_version')).value!==1||(await invoke('image_buffer_capacity')).value!==CAPACITY)throw failure('incompatible staging buffer');
        pointer=(await invoke('image_buffer_ptr')).value;
        if(!Number.isSafeInteger(pointer)||pointer<=0||pointer>0xffffffff-CAPACITY)throw failure('invalid staging address');
        return Object.freeze({version:1,close,async decode(bytes,{maximumWidth=1920,maximumHeight=1080,maximumPixels=MAX_PIXELS}={}){
            alive();if(pending)throw failure('operation already pending','BUSY');
            if(!(bytes instanceof Uint8Array)||!bytes.length||bytes.length>MAX_ENCODED)throw failure('encoded image limit','ENCODED_LIMIT');
            for(const [value,maximum]of [[maximumWidth,8192],[maximumHeight,8192],[maximumPixels,MAX_PIXELS]])if(!Number.isInteger(value)||value<1||value>maximum)throw failure('invalid preview dimensions','INVALID_LIMITS');
            // Capture caller-owned bytes before the first asynchronous boundary.
            const encoded=bytes.slice();pending=true;
            try{
                if((await invoke('image_begin',[encoded.length])).value!==0)throw failure('upload rejected');
                for(let offset=0;offset<encoded.length;offset+=CAPACITY){
                    alive();const chunk=encoded.subarray(offset,Math.min(encoded.length,offset+CAPACITY));
                    if((await invoke('image_write',[offset,chunk.length],{write:{offset:pointer,data:chunk}})).value!==0)throw failure('image chunk rejected');
                }
                const response=await invoke('image_decode',[maximumWidth,maximumHeight,maximumPixels],{read:{offset:pointer,length:40}});alive();
                if(!(response.data instanceof Uint8Array)||response.data.length!==40)throw failure('invalid metadata envelope');
                const view=new DataView(response.data.buffer,response.data.byteOffset,40),value=i=>view.getUint32(i*4,true);
                if(response.value===0){const code=ERRORS[value(0)];if(!code)throw failure('invalid decode status');throw failure('cannot decode this image',code);}
                const snapshot=value(1),width=value(2),height=value(3),sourceWidth=value(4),sourceHeight=value(5),orientation=value(6),format=value(7),length=value(8);
                if(value(0)!==0||value(9)!==1||snapshot!==response.value||!snapshot||!width||!height||width>maximumWidth||height>maximumHeight||width*height>maximumPixels||length!==width*height*4||!sourceWidth||!sourceHeight||sourceWidth>8192||sourceHeight>8192||sourceWidth*sourceHeight>4*1024*1024||orientation<1||orientation>8||![1,2,3].includes(format))throw failure('invalid decoded dimensions');
                const rgba=new Uint8Array(length);
                for(let offset=0;offset<length;offset+=CAPACITY){
                    const count=Math.min(CAPACITY,length-offset);const result=await invoke('image_output_read',[snapshot,offset,count],{read:{offset:pointer,lengthFromResult:true,maxLength:count}});alive();
                    if(result.value!==count||!(result.data instanceof Uint8Array)||result.data.length!==count)throw failure('incomplete pixel snapshot');rgba.set(result.data,offset);
                }
                return Object.freeze({width,height,sourceWidth,sourceHeight,orientation,format:({1:'png',2:'jpeg',3:'svg'})[format],rgba});
            }catch(error){if(!ERRORS.includes(error.code))await close().catch(()=>{});throw error;}finally{pending=false;}
        }});
    }catch(error){await close().catch(()=>{});throw error;}
}
