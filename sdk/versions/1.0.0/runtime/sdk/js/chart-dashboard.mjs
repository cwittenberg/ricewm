import {chartRows} from './chart-data.mjs';
/** Shared row-level filters. Each source retains its own context while every
 * other chart re-aggregates the filtered raw rows. No peer messages or eval. */
export class ChartDashboard {
    #data; #filters = new Map(); #listeners = new Set();
    constructor(data=[]){this.#data=chartRows(data);}
    get data(){return this.#data;}
    get filters(){return Object.freeze(Object.fromEntries(this.#filters));}
    rows({except=null}={}){
        const filters=[...this.#filters].filter(([source])=>source!==except).map(([,f])=>({...f,values:f.values?new Set(f.values):null}));
        return this.#data.filter(row=>filters.every(filter=>
            (filter.values?filter.values.has(row[filter.field]??null):typeof row[filter.field]==='number'&&row[filter.field]>=filter.range[0]&&(filter.inclusiveEnd?row[filter.field]<=filter.range[1]:row[filter.field]<filter.range[1]))));
    }
    setData(data){this.#data=chartRows(data);this.notify();return this;}
    setFilter(source,filter){
        if(typeof source!=='string'||!source||source.length>128)throw TypeError('Filter source requires a stable id');
        if(filter==null){this.#filters.delete(source);this.notify();return this;}
        if(!filter||typeof filter.field!=='string'||!filter.field||Object.keys(filter).some(key=>!['field','values','range','inclusiveEnd'].includes(key))||Boolean(filter.values)===Boolean(filter.range))throw TypeError('Filter requires field and either values or range');
        let next;
        if(filter.values){
            if(!Array.isArray(filter.values)||filter.values.length>10000||filter.values.some(v=>v!==null&&!['string','number','boolean'].includes(typeof v)||typeof v==='number'&&!Number.isFinite(v)))throw TypeError('Invalid chart filter values');
            next={field:filter.field,values:Object.freeze([...filter.values])};
        }else{
            if(!Array.isArray(filter.range)||filter.range.length!==2||!filter.range.every(Number.isFinite)||filter.range[1]<filter.range[0])throw TypeError('Invalid chart filter range');
            next={field:filter.field,range:Object.freeze([...filter.range]),inclusiveEnd:filter.inclusiveEnd===true};
        }
        this.#filters.set(source,Object.freeze(next));this.notify();return this;
    }
    toggle(source,field,value,{additive=false}={}){
        const previous=this.#filters.get(source),values=additive&&previous?.field===field&&previous.values?[...previous.values]:[];
        const index=values.indexOf(value);
        if(index>=0)values.splice(index,1);else if(!additive&&previous?.field===field&&previous.values?.length===1&&previous.values[0]===value)return this.setFilter(source,null);else values.push(value);
        return this.setFilter(source,values.length?{field,values}:null);
    }
    clear(){this.#filters.clear();this.notify();return this;}
    subscribe(listener){if(typeof listener!=='function')throw TypeError('Chart subscriber required');this.#listeners.add(listener);return ()=>this.#listeners.delete(listener);}
    notify(){for(const listener of [...this.#listeners])listener(this.filters);}
}
