/** Bounded ownership metadata from patches already accepted by the native host.
 * Text payloads are skipped in place: this never parses or copies grid data. */
export class NativeGridFocus {
    constructor(){this.windows=new Map();}
    observe(bytes){
        const v=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
        if(v.getUint16(6,true)!==20)return;
        const key=`${v.getBigUint64(12,true)}:${v.getBigUint64(20,true)}`;
        let p=36;
        const take=n=>{const start=p;p+=n;if(p>v.byteLength)throw Error('Truncated metadata');return start;};
        const u8=()=>v.getUint8(take(1)),u32=()=>v.getUint32(take(4),true),id=()=>v.getBigUint64(take(8),true).toString();
        const text=()=>take(u32()),optional=read=>{if(u8()===1)read();};
        let nodes=this.windows.get(key)||new Map(),copied=false;
        const change=()=>{if(!copied){nodes=new Map(nodes);copied=true;}};
        const remove=widget=>{change();const children=new Map();for(const [id,node]of nodes){const list=children.get(node.parent)||[];list.push(id);children.set(node.parent,list);}const pending=[widget],seen=new Set();while(pending.length){const id=pending.pop();if(seen.has(id))continue;seen.add(id);nodes.delete(id);pending.push(...children.get(id)||[]);}};
        try {
            const count=u32();if(count>65536)throw Error('Too many patches');
            for(let i=0;i<count;i++)switch(u8()){
                case 0:{const widget=id(),parent=id();take(4);const kind=v.getUint16(take(2),true);take(4);optional(text);optional(()=>take(2));change();nodes.set(widget,{parent,grid:kind===45});break;}
                case 1:id();optional(()=>take(4));optional(text);optional(()=>take(2));break;
                case 2:{const widget=id(),parent=id();take(4);if(nodes.has(widget)){change();nodes.set(widget,{...nodes.get(widget),parent});}break;}
                case 3:remove(id());break;
                case 4:change();nodes.clear();break;
                case 5:id();text();break;
                case 6:take(9);break;
                case 7:take(16);break;
                case 8:id();optional(text);break;
                default:throw Error('Unknown metadata');
            }
            if(p!==v.byteLength||nodes.size>4096)throw Error('Unsupported metadata');
            if(copied){this.windows.delete(key);if(nodes.size)this.windows.set(key,nodes);}
            while(this.windows.size>128)this.windows.delete(this.windows.keys().next().value);
        }catch{this.windows.delete(key);}
    }
    has(app,window,widget){return this.windows.get(`${app}:${window}`)?.get(String(widget))?.grid===true;}
    removeApp(app){for(const key of this.windows.keys())if(key.startsWith(`${app}:`))this.windows.delete(key);}
    capture(){return [...this.windows].map(([key,nodes])=>({key,nodes:[...nodes].map(([id,node])=>({id,...node}))}));}
    restore(records=[]){
        const next=new Map();if(!Array.isArray(records)||records.length>128)throw Error('Invalid grid focus checkpoint');
        const valid=value=>typeof value==='string'&&/^[1-9][0-9]{0,19}$/.test(value)&&BigInt(value)<=0xffffffffffffffffn;
        for(const record of records){if(typeof record?.key!=='string'||record.key.split(':').length!==2||!record.key.split(':').every(valid)||!Array.isArray(record.nodes)||record.nodes.length>4096||!record.nodes.every(n=>valid(n?.id)&&(n.parent==='0'||valid(n.parent))&&typeof n.grid==='boolean'))throw Error('Invalid grid focus checkpoint');next.set(record.key,new Map(record.nodes.map(({id,parent,grid})=>[id,{parent,grid}])));}
        this.windows=next;
    }
}

/** Search yields only while a real native grid owns application keyboard focus. */
export function nativeGridOwnsSearchChord(renderer,event,searchOpen=false){
    if(searchOpen||event.input!=='key'||Number(event.keycode)!==65||!(Number(event.state)&4)||(Number(event.state)&72))return false;
    const windows=renderer?.nativeCanvasWindow?.snapshots||[];
    const active=windows.findLast(window=>window.active&&window.layerVisible!==false&&!window.minimized);
    return Boolean(active&&active.role!=='desktop'&&!active.dialog&&!active.popup&&active.focusedWidgetKind===45);
}
