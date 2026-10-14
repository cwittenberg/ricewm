const name=value=>typeof value==='string'&&value.length>0&&value.length<=128;
export const validOutputIdentitySlots=slots=>Array.isArray(slots)&&slots.length===16&&slots.every(id=>id===null||name(id))&&new Set(slots.filter(id=>id!==null)).size===slots.filter(id=>id!==null).length;
// Connected entries occupy the leading slots. Disconnected identities retain
// the remaining bounded slots until those slots are needed for new hardware.
export function planOutputIdentities(slots,ids){
    if(!validOutputIdentitySlots(slots)||!Array.isArray(ids)||!ids.length||ids.length>16||!ids.every(name)||new Set(ids).size!==ids.length)throw new RangeError('Invalid output identities');
    const working=[...slots],fresh=new Set();
    for(const id of ids)if(!working.includes(id)){
        let index=working.indexOf(null);
        if(index<0)index=working.findIndex(previous=>!ids.includes(previous));
        working[index]=id;fresh.add(index);
    }
    const order=ids.map(id=>working.indexOf(id));
    for(let index=0;index<16;index++)if(!order.includes(index))order.push(index);
    const oldToNew=Array(16);order.forEach((old,index)=>{oldToNew[old]=index;});
    const resetMask=[...fresh].reduce((mask,old)=>mask|(1<<oldToNew[old]),0);
    return {slots:order.map(old=>working[old]),oldToNew,resetMask};
}
