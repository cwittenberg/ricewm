// Instance ownership metadata only, observed after the native host accepts a
// patch. No application constructor/API catalog is duplicated here.
const decoder = new TextDecoder('utf-8', {fatal:true});
const marker = text => {
    const match = /^drag-handle:(rw-reorder\.([hv])\.r[1-9][0-9]*:flush):([0-9]{1,5})$/.exec(text || '');
    return match && Number(match[3]) <= 65535 ? {group:match[1],axis:match[2],token:match[3]} : null;
};
const validId = value => typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= 0xffffffffffffffffn;

export class NativeReorderPointer {
    constructor(){this.windows=new Map();this.active=new Map();}
    observe(bytes){
        const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);if(v.getUint16(6,true)!==20)return;
        const key=`${v.getBigUint64(12,true)}:${v.getBigUint64(20,true)}`;let p=36;
        const take=n=>{const start=p;p+=n;if(p>v.byteLength)throw Error('Truncated reorder metadata');return start;};
        const u8=()=>v.getUint8(take(1)),u16=()=>v.getUint16(take(2),true),u32=()=>v.getUint32(take(4),true),id=()=>v.getBigUint64(take(8),true).toString();
        const text=()=>{const length=u32(),offset=take(length);return length<=192?decoder.decode(Uint8Array.from(bytes.subarray(offset,offset+length))):null;};
        const skipText=()=>take(u32()),optional=read=>{if(u8()===1)return read();return null;};
        let nodes=new Map(this.windows.get(key)||[]),overlayOpened=false;
        const remove=target=>{const children=new Map();for(const [id,node]of nodes){const list=children.get(node.parent)||[];list.push(id);children.set(node.parent,list);}const pending=[target],seen=new Set();while(pending.length){const next=pending.pop();if(seen.has(next))continue;seen.add(next);nodes.delete(next);pending.push(...children.get(next)||[]);}};
        try{
            const count=u32();if(count>65536)throw Error('Too many reorder patches');
            for(let i=0;i<count;i++)switch(u8()){
                case 0:{const target=id(),parent=id();take(4);const kind=u16(),state=u32(),value=optional(text);optional(()=>take(2));if(kind===50||kind===51)overlayOpened=true;nodes.set(target,{parent,kind,state,marker:kind===12?marker(value):null});break;}
                case 1:{const target=id(),state=optional(u32),mode=u8(),value=mode===1?text():null;optional(()=>take(2));const old=nodes.get(target);if(old)nodes.set(target,{...old,state:state??old.state,marker:mode===0?old.marker:old.kind===12?marker(value):null});break;}
                case 2:{const target=id(),parent=id();take(4);if(nodes.has(target))nodes.set(target,{...nodes.get(target),parent});break;}
                case 3:remove(id());break;
                case 4:nodes.clear();break;
                case 5:id();skipText();break;
                case 6:take(9);break;
                case 7:take(16);break;
                case 8:case 9:id();optional(skipText);break;
                default:throw Error('Unsupported reorder metadata');
            }
            if(p!==v.byteLength||nodes.size>65536)throw Error('Invalid reorder metadata');
            this.windows.delete(key);if(nodes.size)this.windows.set(key,nodes);
            while(this.windows.size>128||[...this.windows.values()].reduce((sum,nodes)=>sum+nodes.size,0)>65536){const oldest=this.windows.keys().next().value;this.windows.delete(oldest);this.active.delete(oldest);}
            const active=this.active.get(key),current=active&&nodes.get(active.target);
            if(active&&(overlayOpened||!(current?.state&1)||current.marker?.group!==active.group))this.active.delete(key);
        }catch{this.windows.delete(key);this.active.delete(key);}
    }
    begin(app,window,target,x,y){
        const key=`${app}:${window}`;this.active.delete(key);
        if(![x,y].every(value=>Number.isFinite(value)&&value>=-2147483648&&value<=2147483647))return;
        const node=this.windows.get(key)?.get(String(target));
        if(node?.marker&&(node.state&1))this.active.set(key,{...node.marker,target:String(target),x,y});
    }
    cancel(app,window){this.active.delete(`${app}:${window}`);}
    gesture(bridge,app,window,kind,x,y){
        const key=`${app}:${window}`,active=this.active.get(key);if(!active)return null;
        if(![x,y].every(value=>Number.isFinite(value)&&value>=-2147483648&&value<=2147483647)){this.active.delete(key);return null;}
        const phase=kind===0?'move':kind===2?'up':null;if(!phase)return null;
        if(phase==='up')this.active.delete(key);
        if(!bridge.widgetActions(app,window,BigInt(active.target))?.activatable)return null;
        const hit=bridge.hitTest(app,window,x,y),node=this.windows.get(key)?.get(String(hit));
        let token=null,side=null;
        if(node?.marker?.group===active.group){
            const size=bridge.renderSizes.get(key),vertical=active.axis==='v',position=Math.trunc(vertical?y:x),maximum=(vertical?size?.height:size?.width)||0;
            if(maximum<1)return null;
            const at=value=>bridge.hitTest(app,window,vertical?x:value,vertical?value:y)===hit;
            let lower=0,upper=position;
            while(lower<upper){const middle=Math.floor((lower+upper)/2);if(at(middle))upper=middle;else lower=middle+1;}
            const first=lower;lower=position;upper=maximum-1;
            while(lower<upper){const middle=Math.ceil((lower+upper)/2);if(at(middle))lower=middle;else upper=middle-1;}
            token=node.marker.token;side=position>(first+lower+1)/2?'after':'before';
        }
        return {target:BigInt(active.target),phase,token,side,x:Math.trunc(x),y:Math.trunc(y)};
    }
    removeApp(app){for(const key of this.windows.keys())if(key.startsWith(`${app}:`)){this.windows.delete(key);this.active.delete(key);}}
    capture(){return [...this.windows].map(([key,nodes])=>({key,nodes:[...nodes].map(([id,node])=>({id,...node}))}));}
    restore(records=[]){
        if(!Array.isArray(records)||records.length>128)throw TypeError('Invalid reorder checkpoint');
        const windows=new Map();let total=0;
        for(const record of records){
            if(typeof record?.key!=='string'||record.key.split(':').length!==2||!record.key.split(':').every(validId)||windows.has(record.key)||!Array.isArray(record.nodes)||(total+=record.nodes.length)>65536)throw TypeError('Invalid reorder owner');
            const nodes=new Map();for(const node of record.nodes){
                const m=node?.marker;
                if(!validId(node?.id)||nodes.has(node.id)||node.parent!=='0'&&!validId(node.parent)||!Number.isInteger(node.kind)||node.kind<1||node.kind>65535||!Number.isInteger(node.state)||node.state<0||node.state>0xffffffff||m!==null&&(!m||!marker(`drag-handle:${m.group}:${m.token}`)||!['h','v'].includes(m.axis)||m.axis!==marker(`drag-handle:${m.group}:${m.token}`).axis))throw TypeError('Invalid reorder node');
                nodes.set(node.id,{parent:node.parent,kind:node.kind,state:node.state,marker:m?{...m}:null});
            }windows.set(record.key,nodes);
        }this.windows=windows;this.active.clear();
    }
}
