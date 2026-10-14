/** Fractional SFNT metrics shared with the native unhinted document shaper.
 * Canvas TextMetrics rounds font bounds at small zoom, moving editing baselines. */
export function documentFontMetricsFromBytes(bytes) {
    if(!(bytes instanceof ArrayBuffer)||bytes.byteLength<12||bytes.byteLength>20*1024*1024)throw new RangeError('Invalid bounded document font');
    const view=new DataView(bytes),count=view.getUint16(4),tables=new Map();
    if(count>256||12+count*16>bytes.byteLength)throw new RangeError('Invalid document font tables');
    for(let i=0;i<count;i++){const at=12+i*16,tag=String.fromCharCode(...new Uint8Array(bytes,at,4)),offset=view.getUint32(at+8),length=view.getUint32(at+12);if(offset+length>bytes.byteLength)throw new RangeError('Invalid document font table bounds');tables.set(tag,{offset,length});}
    const head=tables.get('head'),hhea=tables.get('hhea'),os2=tables.get('OS/2');
    if(!head||head.length<20||!hhea||hhea.length<10)throw new RangeError('Document font requires horizontal metrics');
    const units=view.getUint16(head.offset+18);if(!units)throw new RangeError('Invalid font units');
    let ascent=view.getInt16(hhea.offset+4),descent=-view.getInt16(hhea.offset+6),gap=view.getInt16(hhea.offset+8);
    if(os2?.length>=74&&(view.getUint16(os2.offset+62)&128)){ascent=view.getInt16(os2.offset+68);descent=-view.getInt16(os2.offset+70);gap=view.getInt16(os2.offset+72);}
    return Object.freeze({ascent:ascent/units,descent:descent/units,gap:gap/units});
}
