import {NATIVE_ICONS} from './icon-data.mjs';
export {NATIVE_ICONS};
/** Numeric ABI constants, safe to send to the native UI worker. */
export const Icons=Object.freeze(Object.fromEntries(NATIVE_ICONS.map(({constant,id})=>[constant,id])));
const names=new Map();
for(const {constant,id,name} of NATIVE_ICONS){
    names.set(constant.toLowerCase(),id);
    if(!names.has(name))names.set(name,id);
}
/** Resolve exact semantic names or constants. No fuzzy selection at runtime. */
export function iconId(value){
    if(typeof value==='number'&&Number.isInteger(value)&&value>=0&&value<=65535)return value;
    if(typeof value==='string'){
        const found=names.get(value.trim().toLowerCase());if(found!==undefined)return found;
        // Preserve the pre-existing serialized numeric ID contract.
        if(/^\d{1,5}$/.test(value)&&Number(value)<=65535)return Number(value);
    }
    throw new RangeError('icon must be a RiceUI unsigned 16-bit icon ID or known name; import {Icons,iconId} from "@ricewm/app" or search sdk.icons. SVG/PNG paths use createIconSurface, not the icon property.');
}
const aliases={calendar:'calendar date time clock alarm reminder schedule',ObjectAdd:'add plus create new',ObjectRemove:'remove minus subtract',SelectionCheck:'check done confirm accept',Close:'close dismiss cancel',Refresh:'refresh reload repeat snooze',Information:'information info help',Settings:'settings preferences cog gear'};
export function findIcons(query='',{limit=20}={}){
    if(typeof query!=='string'||query.length>128||!Number.isInteger(limit)||limit<1||limit>50)throw new TypeError('Expected an icon query and limit 1..50');
    const tokens=query.toLowerCase().split(/[\s_-]+/).filter(Boolean);
    return NATIVE_ICONS.filter(entry=>tokens.every(token=>(entry.constant+' '+entry.name+' '+(aliases[entry.constant]||aliases[entry.name]||'')).toLowerCase().includes(token))).slice(0,limit);
}
