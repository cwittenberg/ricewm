import { inflateRiceRawDeflate } from "../../sdk/versions/1.0.0/runtime/sdk/js/raw-deflate.mjs";
export { inflateRiceRawDeflate, inflateRiceRawDeflateSync } from "../../sdk/versions/1.0.0/runtime/sdk/js/raw-deflate.mjs";

const EOCD = 0x06054b50;
const CENTRAL = 0x02014b50;
const LOCAL = 0x04034b50;
const MAX_ARCHIVE_BYTES = 16 * 1024 * 1024;
const MAX_OUTPUT_BYTES = 32 * 1024 * 1024;
const MAX_FILES = 512;

function pathName(bytes) {
    const name = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    const parts = name.endsWith("/") ? name.slice(0, -1).split("/") : name.split("/");
    if (
        !name || name.startsWith("/") || name.includes("\\") ||
        name.includes("\0") || parts.some(part => part === ".." || part === "")
    ) throw new RangeError("ZIP entry path is unsafe");
    return name;
}

const crc32Table = Uint32Array.from({ length: 256 }, (_, value) => {
    for (let bit = 0; bit < 8; bit += 1) {
        value = (value >>> 1) ^ (0xedb88320 & -(value & 1));
    }
    return value >>> 0;
});

function crc32(bytes) {
    let crc = 0xffffffff;
    for (let index = 0; index < bytes.length; index += 1) {
        crc = (crc >>> 8) ^ crc32Table[(crc ^ bytes[index]) & 0xff];
    }
    return (crc ^ 0xffffffff) >>> 0;
}

/** Extract a bounded, non-encrypted ZIP without allowing filesystem traversal. */
export async function extractRiceExtensionZip(input,{rejectDirectories=false}={}) {
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    if (bytes.byteLength < 22 || bytes.byteLength > MAX_ARCHIVE_BYTES) {
        throw new RangeError("extension ZIP size is invalid");
    }
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let eocd = -1;
    const lower = Math.max(0, bytes.byteLength - 65_557);
    for (let offset = bytes.byteLength - 22; offset >= lower; offset -= 1) {
        if (view.getUint32(offset, true) === EOCD) { eocd = offset; break }
    }
    if (eocd < 0) throw new TypeError("ZIP central directory is missing");
    const count = view.getUint16(eocd + 10, true);
    const centralSize = view.getUint32(eocd + 12, true);
    let cursor = view.getUint32(eocd + 16, true);
    if (count > MAX_FILES || cursor + centralSize > eocd) {
        throw new RangeError("ZIP central directory is invalid");
    }
    const output = new Map();
    let outputBytes = 0;
    for (let index = 0; index < count; index += 1) {
        if (cursor + 46 > bytes.byteLength || view.getUint32(cursor, true) !== CENTRAL) {
            throw new TypeError("ZIP entry header is invalid");
        }
        const flags = view.getUint16(cursor + 8, true);
        const method = view.getUint16(cursor + 10, true);
        const expectedCrc = view.getUint32(cursor + 16, true);
        const compressedSize = view.getUint32(cursor + 20, true);
        const size = view.getUint32(cursor + 24, true);
        const nameLength = view.getUint16(cursor + 28, true);
        const extraLength = view.getUint16(cursor + 30, true);
        const commentLength = view.getUint16(cursor + 32, true);
        const external = view.getUint32(cursor + 38, true);
        const localOffset = view.getUint32(cursor + 42, true);
        const end = cursor + 46 + nameLength + extraLength + commentLength;
        if (end > bytes.byteLength || (flags & 1) !== 0 || ![0, 8].includes(method)) {
            throw new TypeError("ZIP entry uses an unsupported feature");
        }
        const name = pathName(bytes.subarray(cursor + 46, cursor + 46 + nameLength));
        if (((external >>> 16) & 0xf000) === 0xa000) {
            throw new TypeError("ZIP symbolic links are not allowed");
        }
        cursor = end;
        if (name.endsWith("/")) {
            if(rejectDirectories)throw new TypeError('ZIP source reproduction requires regular file entries only');
            continue;
        }
        if (localOffset + 30 > bytes.byteLength || view.getUint32(localOffset, true) !== LOCAL) {
            throw new TypeError("ZIP local entry is invalid");
        }
        const localName = view.getUint16(localOffset + 26, true);
        const localExtra = view.getUint16(localOffset + 28, true);
        const dataOffset = localOffset + 30 + localName + localExtra;
        if (dataOffset + compressedSize > bytes.byteLength) throw new RangeError("ZIP data is truncated");
        const compressed = bytes.subarray(dataOffset, dataOffset + compressedSize);
        if (size > MAX_OUTPUT_BYTES - outputBytes) throw new RangeError("ZIP output is invalid");
        const data = method === 0
            ? compressed.slice()
            : await inflateRiceRawDeflate(compressed, size, MAX_OUTPUT_BYTES - outputBytes);
        if (data.byteLength !== size || crc32(data) !== expectedCrc) {
            throw new TypeError("ZIP entry integrity check failed");
        }
        outputBytes += data.byteLength;
        if (outputBytes > MAX_OUTPUT_BYTES || output.has(name)) {
            throw new RangeError("ZIP output is invalid");
        }
        output.set(name, data);
    }
    return output;
}
