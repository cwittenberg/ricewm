import { inflateRiceRawDeflateSync } from './raw-deflate.mjs';

const decoder = new TextDecoder('utf-8', { fatal: true });
const table = Uint32Array.from({ length: 256 }, (_, value) => {
    for (let i = 0; i < 8; i++) value = value >>> 1 ^ (value & 1 ? 0xedb88320 : 0);
    return value >>> 0;
});
function crc32(bytes) {
    let crc = 0xffffffff;
    for (const value of bytes) crc = crc >>> 8 ^ table[(crc ^ value) & 255];
    return (crc ^ 0xffffffff) >>> 0;
}
function equal(a, b) { return a.length === b.length && a.every((v, i) => v === b[i]); }
const invalid = message => { throw new TypeError(`Invalid ZIP: ${message}`); };
function extra(bytes) {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    for (let offset = 0; offset < bytes.length;) {
        if (offset + 4 > bytes.length) invalid('truncated extra field');
        const kind = view.getUint16(offset, true), size = view.getUint16(offset + 2, true);
        if (kind === 1) invalid('ZIP64 requires a separate bounded adapter');
        offset += size + 4;
        if (offset > bytes.length) invalid('truncated extra field');
    }
}
function bounds(options) {
    const result = { maximumBytes: 16 * 1024 * 1024, maximumPartBytes: 32 * 1024 * 1024, maximumExpandedBytes: 128 * 1024 * 1024, maximumEntries: 16384, ...options };
    for (const [key, maximum] of Object.entries({ maximumBytes: 256 * 1024 * 1024, maximumPartBytes: 256 * 1024 * 1024, maximumExpandedBytes: 1024 * 1024 * 1024, maximumEntries: 65534 })) {
        if (!Number.isSafeInteger(result[key]) || result[key] < 1 || result[key] > maximum) throw new RangeError('Invalid ZIP bounds');
    }
    if (Object.keys(result).some(key => !['maximumBytes','maximumPartBytes','maximumExpandedBytes','maximumEntries'].includes(key))) throw new TypeError('Unknown ZIP bound');
    return Object.freeze(result);
}

/** Immutable bounded ZIP parts. Replacement retains every untouched local
 * record (compressed bytes, metadata and descriptor) and central extra/comment.
 * replace() changes existing files; edit() also adds/removes file parts. This
 * is not a filesystem extractor or a package/signature policy adapter.
 * CRC validation is lazy on read; a package owner must authorize the whole edit.
 */
export class ZipParts {
    #bytes; #records = new Map(); #limits; #end; #central;
    static controlFactory = Object.freeze({ topics: ['documents', 'workflow'],
        constructor: 'new ZipParts(bytes,{maximumBytes?,maximumPartBytes?,maximumExpandedBytes?,maximumEntries?})',
        methods: 'ZipParts.create(Map<string,Uint8Array>,zipBounds?) returns fresh ZIP bytes. names, has(name), read(name), replace(Map<string,Uint8Array>), edit(Map<string,Uint8Array|null>). edit adds/replaces files and removes existing files with null. Bounded CRC-checked reads and edits retain untouched compressed records. Package signatures, relationships, semantics and document history remain caller-owned.' });
    static create(parts,options={}) {
        const empty=new Uint8Array(22);new DataView(empty.buffer).setUint32(0,0x06054b50,true);
        return new ZipParts(empty,options).edit(parts);
    }
    constructor(input, options = {}) {
        this.#limits = bounds(options);
        if (!(input instanceof Uint8Array) || input.length < 22 || input.length > this.#limits.maximumBytes) throw new RangeError('ZIP source exceeds its bounds');
        const bytes = this.#bytes = input.slice(), view = new DataView(bytes.buffer);
        let end = -1;
        for (let offset = bytes.length - 22; offset >= Math.max(0, bytes.length - 65557); offset--) {
            if (view.getUint32(offset, true) === 0x06054b50 && offset + 22 + view.getUint16(offset + 20, true) === bytes.length) { end = offset; break; }
        }
        if (end < 0 || view.getUint16(end + 4, true) || view.getUint16(end + 6, true)) invalid('missing or multi-disk directory');
        const count = view.getUint16(end + 10, true), size = view.getUint32(end + 12, true), central = view.getUint32(end + 16, true);
        if (count !== view.getUint16(end + 8, true) || count > this.#limits.maximumEntries || count === 0xffff || central + size !== end) invalid('central directory bounds');
        this.#end = end; this.#central = central;
        let cursor = central, expanded = 0; const aliases = new Set(), occupied = [];
        for (let index = 0; index < count; index++) {
            if (cursor + 46 > end || view.getUint32(cursor, true) !== 0x02014b50) invalid('central entry');
            const flags = view.getUint16(cursor + 8, true), method = view.getUint16(cursor + 10, true), crc = view.getUint32(cursor + 16, true);
            const compressed = view.getUint32(cursor + 20, true), length = view.getUint32(cursor + 24, true);
            const nameLength = view.getUint16(cursor + 28, true), extraLength = view.getUint16(cursor + 30, true), comment = view.getUint16(cursor + 32, true);
            const disk = view.getUint16(cursor + 34, true), external = view.getUint32(cursor + 38, true), local = view.getUint32(cursor + 42, true);
            const stop = cursor + 46 + nameLength + extraLength + comment;
            if (stop > end || disk || flags & ~0x080e || ![0, 8].includes(method) || method === 0 && flags & 6 ||
                [compressed, length, local].includes(0xffffffff)) invalid('unsupported entry feature');
            const nameBytes = bytes.subarray(cursor + 46, cursor + 46 + nameLength);
            if (!(flags & 0x800) && nameBytes.some(byte => byte > 127)) invalid('non-UTF-8 legacy name');
            const name = decoder.decode(nameBytes), directory = name.endsWith('/'), segments = (directory ? name.slice(0, -1) : name).split('/');
            if (!name || name.length > 4096 || name.startsWith('/') || name.includes('\\') || /[\x00-\x1f\x7f]/u.test(name) || segments.some(p => !p || p === '.' || p === '..') || aliases.has(name.toLowerCase())) invalid('unsafe or ambiguous part name');
            aliases.add(name.toLowerCase());
            const kind = external >>> 16 & 0xf000;
            if (![0, 0x4000, 0x8000].includes(kind) || kind === 0x4000 && !directory || kind === 0x8000 && directory) invalid('non-regular part');
            expanded += length;
            if (length > this.#limits.maximumPartBytes || expanded > this.#limits.maximumExpandedBytes || directory && length) throw new RangeError('ZIP expansion exceeds its bounds');
            extra(bytes.subarray(cursor + 46 + nameLength, cursor + 46 + nameLength + extraLength));
            if (local + 30 > central || view.getUint32(local, true) !== 0x04034b50 || view.getUint16(local + 6, true) !== flags || view.getUint16(local + 8, true) !== method) invalid('local header mismatch');
            const localName = view.getUint16(local + 26, true), localExtra = view.getUint16(local + 28, true), data = local + 30 + localName + localExtra;
            if (data + compressed > central || !equal(bytes.subarray(local + 30, local + 30 + localName), nameBytes)) invalid('local part name or length mismatch');
            extra(bytes.subarray(local + 30 + localName, data));
            for (const [position, value] of [[14, crc],[18, compressed],[22, length]]) {
                const observed = view.getUint32(local + position, true);
                if (observed !== value && (!(flags & 8) || observed !== 0)) invalid('local size or CRC mismatch');
            }
            let localEnd = data + compressed;
            if (flags & 8) {
                if (localEnd + 12 > central) invalid('missing data descriptor');
                if (view.getUint32(localEnd, true) === 0x08074b50) localEnd += 4;
                if (localEnd + 12 > central || view.getUint32(localEnd, true) !== crc || view.getUint32(localEnd + 4, true) !== compressed || view.getUint32(localEnd + 8, true) !== length) invalid('data descriptor mismatch');
                localEnd += 12;
            }
            occupied.push({ start: local, end: localEnd });
            this.#records.set(name, Object.freeze({ name, directory, flags, method, crc, compressed, length, local, localEnd, data, central: cursor, centralEnd: stop }));
            cursor = stop;
        }
        if (cursor !== end) invalid('uncounted directory data');
        occupied.sort((a,b) => a.start - b.start);
        for (let i = 1; i < occupied.length; i++) if (occupied[i].start < occupied[i-1].end) invalid('overlapping local records');
        Object.freeze(this);
    }
    get names() { return Object.freeze([...this.#records.keys()]); }
    has(name) { return this.#records.has(name); }
    read(name) {
        const record = this.#records.get(name);
        if (!record || record.directory) throw new TypeError('Expected an existing ZIP file part');
        const compressed = this.#bytes.subarray(record.data, record.data + record.compressed);
        const output = record.method === 0 ? compressed.slice() : inflateRiceRawDeflateSync(compressed, record.length, this.#limits.maximumPartBytes);
        if (output.length !== record.length || crc32(output) !== record.crc) invalid('part CRC or decoded size');
        return output;
    }
    replace(replacements) {
        if (!(replacements instanceof Map) || replacements.size > this.#records.size) throw new TypeError('Expected bounded ZIP replacements');
        for(const[name,input]of replacements)if(!this.#records.has(name)||this.#records.get(name).directory||!(input instanceof Uint8Array))throw new TypeError('Only existing ZIP file parts may be replaced');
        return this.edit(replacements);
    }
    edit(replacements) {
        if (!(replacements instanceof Map) || replacements.size > this.#limits.maximumEntries + this.#records.size) throw new TypeError('Expected bounded ZIP part edits');
        const changed = new Map(); let expanded = [...this.#records.values()].reduce((total, r) => total + r.length, 0);
        for (const [name, input] of replacements) {
            const record = this.#records.get(name);
            if(record?.directory || input===null&&!record || input!==null&&!(input instanceof Uint8Array))throw new TypeError('Expected file bytes or an existing file to remove');
            if(typeof name!=='string'||!name||name.length>4096||name.startsWith('/')||name.includes('\\')||/[\x00-\x1f\x7f\ud800-\udfff]/u.test(name)||name.split('/').some(segment=>!segment||segment==='.'||segment==='..'))throw new TypeError('Unsafe ZIP file part name');
            if(input===null){changed.set(name,null);expanded-=record.length;continue;}
            if (input.length > this.#limits.maximumPartBytes) throw new RangeError('ZIP replacement exceeds its bounds');
            if (!record||!equal(input, this.read(name))) { changed.set(name, input.slice()); expanded += input.length - (record?.length??0); }
        }
        if (expanded > this.#limits.maximumExpandedBytes) throw new RangeError('ZIP replacement expansion exceeds its bounds');
        if (!changed.size) return this.#bytes.slice();
        const locals = [], centrals = [], names=new Set(); let offset = 0,count=0;
        const append=(name,local,central)=>{
            if(names.has(name.toLowerCase()))invalid('unsafe or ambiguous part name');names.add(name.toLowerCase());
            if(++count>this.#limits.maximumEntries)throw new RangeError('ZIP edited entry count exceeds its bounds');
            new DataView(central.buffer,central.byteOffset,central.byteLength).setUint32(42,offset,true);
            locals.push(...local);centrals.push(central);offset+=local.reduce((sum,bytes)=>sum+bytes.length,0);
            if(offset>this.#limits.maximumBytes)throw new RangeError('ZIP replacement exceeds its bounds');
        };
        for (const record of this.#records.values()) {
            if(changed.has(record.name)&&changed.get(record.name)===null)continue;
            const central = this.#bytes.slice(record.central, record.centralEnd), cv = new DataView(central.buffer);
            const replacement = changed.get(record.name); let local;
            if (replacement) {
                const header = this.#bytes.slice(record.local, record.data), lv = new DataView(header.buffer), crc = crc32(replacement), flags = record.flags & 0x800;
                lv.setUint16(6, flags, true); lv.setUint16(8, 0, true); lv.setUint32(14, crc, true); lv.setUint32(18, replacement.length, true); lv.setUint32(22, replacement.length, true);
                cv.setUint16(8, flags, true); cv.setUint16(10, 0, true); cv.setUint32(16, crc, true); cv.setUint32(20, replacement.length, true); cv.setUint32(24, replacement.length, true);
                local = [header, replacement];
            } else local = [this.#bytes.subarray(record.local, record.localEnd)];
            append(record.name,local,central);
        }
        for(const[name,data]of changed)if(!this.#records.has(name)) {
            const encoded=new TextEncoder().encode(name),header=new Uint8Array(30+encoded.length),local=new DataView(header.buffer),central=new Uint8Array(46+encoded.length),view=new DataView(central.buffer),crc=crc32(data);
            local.setUint32(0,0x04034b50,true);local.setUint16(4,20,true);local.setUint16(6,0x800,true);local.setUint16(12,0x21,true);local.setUint32(14,crc,true);local.setUint32(18,data.length,true);local.setUint32(22,data.length,true);local.setUint16(26,encoded.length,true);header.set(encoded,30);
            view.setUint32(0,0x02014b50,true);view.setUint16(4,20,true);view.setUint16(6,20,true);view.setUint16(8,0x800,true);view.setUint16(14,0x21,true);view.setUint32(16,crc,true);view.setUint32(20,data.length,true);view.setUint32(24,data.length,true);view.setUint16(28,encoded.length,true);central.set(encoded,46);
            append(name,[header,data],central);
        }
        const directoryLength = centrals.reduce((sum, bytes) => sum + bytes.length, 0), end = this.#bytes.slice(this.#end), ev = new DataView(end.buffer);
        ev.setUint16(8,count,true);ev.setUint16(10,count,true);
        ev.setUint32(12, directoryLength, true); ev.setUint32(16, offset, true);
        const length = offset + directoryLength + end.length;
        if (length > this.#limits.maximumBytes) throw new RangeError('ZIP replacement exceeds its bounds');
        const output = new Uint8Array(length); let cursor = 0;
        for (const piece of [...locals, ...centrals, end]) { output.set(piece, cursor); cursor += piece.length; }
        return output;
    }
}
