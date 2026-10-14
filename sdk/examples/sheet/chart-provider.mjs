import {checkedChart,checkedChartList,CHART_LIMITS} from './chart-contract.mjs';
import {checkedChartProjection} from './chart-scene.mjs';
import {checkedViewport} from './viewport-contract.mjs';

const encoder=new TextEncoder();
const fail=message=>{throw Object.assign(new Error(message),{code:'CHART_PROTOCOL'});};
const idValue=id=>{if(!Number.isSafeInteger(id)||id<1||id>0xffffffff)fail('Invalid chart identity');return id;};
const integer=value=>Number.isSafeInteger(value)&&value>=0;
const exact=(value,keys)=>value&&Object.getPrototypeOf(value)===Object.prototype&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
function envelope(reply) {
    if(!exact(reply,['revision','dataRevision','result'])||!integer(reply.revision)||!integer(reply.dataRevision)||!reply.result||Object.getPrototypeOf(reply.result)!==Object.prototype)fail('Invalid chart query envelope');
    return reply;
}

/** Bounded, serialized access to Wasm-owned chart results. Callers supply the
 * ordinary query transport; no worksheet rows or formula values are requested.
 * The projection object is retained on unchanged replies, even when an unrelated
 * workbook edit advances the outer revision/dataRevision cursor. */
export function openChartProvider({query,maximumCacheBytes=4*1024*1024}={}) {
    if(typeof query!=='function'||!Number.isSafeInteger(maximumCacheBytes)||maximumCacheBytes<CHART_LIMITS.projectionBytes||maximumCacheBytes>4*1024*1024)throw new TypeError('Invalid chart provider');
    const cache=new Map(),versions=new Map();let cacheBytes=0,closed=false,generation=0,queue=Promise.resolve();
    const live=(stamp,id,version)=>{if(closed||stamp!==generation||id!==undefined&&(versions.get(id)||0)!==version)throw Object.assign(new Error('Chart request was invalidated'),{code:'CHART_STALE'});};
    function serial(run,id) {
        const stamp=generation,version=id===undefined?0:versions.get(id)||0;
        const check=()=>live(stamp,id,version);
        const result=queue.catch(()=>{}).then(()=>{check();return run(async request=>{check();const reply=envelope(await query(request));check();return reply;});});
        queue=result.then(()=>{},()=>{});return result;
    }
    function retain(id,value) {
        if(cache.has(id)){cacheBytes-=cache.get(id).bytes;cache.delete(id);}
        while(cacheBytes+value.bytes>maximumCacheBytes&&cache.size){const oldest=cache.keys().next().value;cacheBytes-=cache.get(oldest).bytes;cache.delete(oldest);}
        cache.set(id,value);cacheBytes+=value.bytes;
    }
    return {
        list(sheetId,{offset=0,limit=16,viewport}={}) {
            if(typeof sheetId!=='string'||!sheetId.length||sheetId.length>128||!integer(offset)||offset>CHART_LIMITS.charts||!integer(limit)||limit<1||limit>16)throw new TypeError('Invalid chart list request');
            const request={action:'charts-list',sheetId,offset,limit,...(viewport?{viewport:checkedViewport(viewport)}:{})};
            return serial(async send=>{const reply=await send(request);return Object.freeze({revision:reply.revision,dataRevision:reply.dataRevision,result:checkedChartList(reply.result,{offset,limit})});});
        },
        get(id) {
            idValue(id);return serial(async send=>{const reply=await send({action:'chart-get',id});if(!exact(reply.result,['chart']))fail('Invalid chart metadata response');const chart=checkedChart(reply.result.chart);if(chart.id!==id)fail('Chart identity changed');return Object.freeze({revision:reply.revision,dataRevision:reply.dataRevision,result:Object.freeze({chart})});},id);
        },
        projection(id) {
            idValue(id);return serial(async send=>{
                const previous=cache.get(id);let reply=await send({action:'chart-projection',id,offset:0,...(previous?{token:previous.token}:{})}),page=reply.result;
                if(page.unchanged===true){
                    if(!exact(page,['id','token','unchanged'])||page.id!==id||!previous||page.token!==previous.token)fail('Unchanged chart has no matching retained projection');
                    cache.delete(id);cache.set(id,previous);
                    return Object.freeze({revision:reply.revision,dataRevision:reply.dataRevision,token:page.token,unchanged:true,projection:previous.projection});
                }
                const revision=reply.revision,dataRevision=reply.dataRevision;let token,total,offset=0,parts=[];
                for(let index=0;index<64;index++) {
                    if(!exact(page,['id','token','offset','nextOffset','bytes','payload'])||page.id!==id||!integer(page.token)||page.token<1||page.offset!==offset||!integer(page.bytes)||page.bytes<1||page.bytes>CHART_LIMITS.projectionBytes||typeof page.payload!=='string'||/[\ud800-\udfff]/u.test(page.payload))fail('Invalid chart projection page');
                    const count=encoder.encode(page.payload).length,end=offset+count;
                    if(!count||count>CHART_LIMITS.pageBytes||end>page.bytes||page.nextOffset!==(end===page.bytes?null:end))fail('Invalid chart byte cursor');
                    if(index===0){token=page.token;total=page.bytes;}else if(page.token!==token||page.bytes!==total||reply.revision!==revision||reply.dataRevision!==dataRevision)fail('Chart changed while retrieving its projection');
                    parts.push(page.payload);
                    if(page.nextOffset===null){
                        let decoded;try{decoded=JSON.parse(parts.join(''));}catch{fail('Invalid chart projection JSON');}
                        const projection=Object.freeze({...checkedChartProjection(decoded),dataRevision});
                        if(projection.chartId!==id)fail('Projection belongs to another chart');
                        retain(id,{projection,token,bytes:total});
                        return Object.freeze({revision,dataRevision,token,unchanged:false,projection});
                    }
                    offset=page.nextOffset;reply=await send({action:'chart-projection',id,offset,token});page=reply.result;
                }
                fail('Chart projection exceeded its page budget');
            },id);
        },
        invalidate(id) {
            if(id===undefined){generation++;cache.clear();cacheBytes=0;versions.clear();return;}
            idValue(id);versions.set(id,(versions.get(id)||0)+1);if(cache.has(id)){cacheBytes-=cache.get(id).bytes;cache.delete(id);}
        },
        close(){if(!closed){closed=true;generation++;cache.clear();cacheBytes=0;versions.clear();}},
        get byteLength(){return cacheBytes;},
        get size(){return cache.size;},
    };
}
