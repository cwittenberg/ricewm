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

class DeflateBits {
    constructor(bytes) {
        this.bytes = bytes;
        this.offset = 0;
        this.bit = 0;
    }

    read(count) {
        let value = 0;
        for (let index = 0; index < count; index += 1) {
            if (this.offset >= this.bytes.byteLength) throw new RangeError("DEFLATE data is truncated");
            value |= ((this.bytes[this.offset] >>> this.bit) & 1) << index;
            this.bit += 1;
            if (this.bit === 8) { this.bit = 0; this.offset += 1 }
        }
        return value;
    }

    align() {
        if (this.bit !== 0) { this.bit = 0; this.offset += 1 }
    }
}

function reverseBits(value, length) {
    let reversed = 0;
    for (let index = 0; index < length; index += 1) {
        reversed = (reversed << 1) | ((value >>> index) & 1);
    }
    return reversed;
}

function huffman(lengths) {
    const counts = Array(16).fill(0);
    for (const length of lengths) {
        if (length < 0 || length > 15) throw new RangeError("DEFLATE code length is invalid");
        if (length) counts[length] += 1;
    }
    const next = Array(16).fill(0);
    let code = 0;
    let available = 1;
    for (let bits = 1; bits <= 15; bits += 1) {
        available = available * 2 - counts[bits];
        if (available < 0) throw new TypeError("DEFLATE Huffman table is oversubscribed");
        code = (code + counts[bits - 1]) << 1;
        next[bits] = code;
    }
    const table = new Map();
    let maximum = 0;
    lengths.forEach((length, symbol) => {
        if (!length) return;
        maximum = Math.max(maximum, length);
        const reversed = reverseBits(next[length]++, length);
        table.set(`${length}:${reversed}`, symbol);
    });
    if (!maximum) throw new TypeError("DEFLATE Huffman table is empty");
    return { table, maximum };
}

function symbol(bits, tree) {
    let code = 0;
    for (let length = 1; length <= tree.maximum; length += 1) {
        code |= bits.read(1) << (length - 1);
        const value = tree.table.get(`${length}:${code}`);
        if (value !== undefined) return value;
    }
    throw new TypeError("DEFLATE Huffman code is invalid");
}

function fixedTrees() {
    const literal = Array(288).fill(0);
    literal.fill(8, 0, 144);
    literal.fill(9, 144, 256);
    literal.fill(7, 256, 280);
    literal.fill(8, 280);
    return { literal: huffman(literal), distance: huffman(Array(32).fill(5)) };
}

function dynamicTrees(bits) {
    const literalCount = bits.read(5) + 257;
    const distanceCount = bits.read(5) + 1;
    const codeCount = bits.read(4) + 4;
    if (literalCount > 286 || distanceCount > 32) throw new RangeError("DEFLATE table is oversized");
    const order = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
    const codeLengths = Array(19).fill(0);
    for (let index = 0; index < codeCount; index += 1) codeLengths[order[index]] = bits.read(3);
    const codeTree = huffman(codeLengths);
    const lengths = [];
    while (lengths.length < literalCount + distanceCount) {
        const value = symbol(bits, codeTree);
        if (value <= 15) {
            lengths.push(value);
        } else if (value === 16) {
            if (!lengths.length) throw new TypeError("DEFLATE repeat has no predecessor");
            const repeated = lengths.at(-1);
            lengths.push(...Array(bits.read(2) + 3).fill(repeated));
        } else if (value === 17) {
            lengths.push(...Array(bits.read(3) + 3).fill(0));
        } else if (value === 18) {
            lengths.push(...Array(bits.read(7) + 11).fill(0));
        } else {
            throw new TypeError("DEFLATE code-length symbol is invalid");
        }
        if (lengths.length > literalCount + distanceCount) {
            throw new RangeError("DEFLATE code-length table overflowed");
        }
    }
    return {
        literal: huffman(lengths.slice(0, literalCount)),
        distance: huffman(lengths.slice(literalCount)),
    };
}

const LENGTH_BASE = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DISTANCE_BASE = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129,
    193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289,
    16385, 24577];
const DISTANCE_EXTRA = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6,
    7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

export function inflateRiceRawDeflateSync(input, expected, maximum = MAX_OUTPUT_BYTES) {
    if (!(input instanceof Uint8Array) || !Number.isSafeInteger(expected) || expected < 0 ||
        !Number.isSafeInteger(maximum) || maximum < 0 || expected > maximum) {
        throw new RangeError("DEFLATE output exceeds its safety bound");
    }
    const bits = new DeflateBits(input);
    const output = new Uint8Array(expected);
    let cursor = 0;
    let final = false;
    while (!final) {
        final = bits.read(1) === 1;
        const kind = bits.read(2);
        if (kind === 0) {
            bits.align();
            const length = bits.read(16);
            if ((length ^ 0xffff) !== bits.read(16)) throw new TypeError("DEFLATE stored block is invalid");
            if (cursor + length > output.byteLength) throw new RangeError("DEFLATE output overflowed");
            for (let index = 0; index < length; index += 1) output[cursor++] = bits.read(8);
            continue;
        }
        if (kind === 3) throw new TypeError("DEFLATE block kind is invalid");
        const trees = kind === 1 ? fixedTrees() : dynamicTrees(bits);
        while (true) {
            const value = symbol(bits, trees.literal);
            if (value < 256) {
                if (cursor >= output.byteLength) throw new RangeError("DEFLATE output overflowed");
                output[cursor++] = value;
                continue;
            }
            if (value === 256) break;
            if (value < 257 || value > 285) throw new TypeError("DEFLATE length symbol is invalid");
            const lengthIndex = value - 257;
            const length = LENGTH_BASE[lengthIndex] + bits.read(LENGTH_EXTRA[lengthIndex]);
            const distanceSymbol = symbol(bits, trees.distance);
            if (distanceSymbol > 29) throw new TypeError("DEFLATE distance symbol is invalid");
            const distance = DISTANCE_BASE[distanceSymbol] + bits.read(DISTANCE_EXTRA[distanceSymbol]);
            if (distance > cursor || cursor + length > output.byteLength) {
                throw new RangeError("DEFLATE back-reference is invalid");
            }
            for (let index = 0; index < length; index += 1) {
                output[cursor] = output[cursor - distance];
                cursor += 1;
            }
        }
    }
    if (cursor !== output.byteLength) throw new RangeError("DEFLATE output size is invalid");
    return output;
}

export async function inflateRiceRawDeflate(bytes, expected, maximum = MAX_OUTPUT_BYTES) {
    if (typeof DecompressionStream === "function") {
        try {
            const stream = new Blob([bytes]).stream().pipeThrough(
                new DecompressionStream("deflate-raw"),
            );
            const reader = stream.getReader();
            const chunks = [];
            let length = 0;
            while (true) {
                const { value, done } = await reader.read();
                if (done) break;
                length += value.byteLength;
                if (length > maximum) {
                    await reader.cancel();
                    throw new RangeError("DEFLATE output exceeds its safety bound");
                }
                chunks.push(value);
            }
            const output = new Uint8Array(length);
            let offset = 0;
            for (const chunk of chunks) {
                output.set(chunk, offset);
                offset += chunk.byteLength;
            }
            return output;
        } catch {
            // Older Worker runtimes expose DecompressionStream without the
            // raw-DEFLATE ZIP format. Use the bounded contained decoder below.
        }
    }
    if (expected > maximum) throw new RangeError("DEFLATE output exceeds its safety bound");
    return inflateRiceRawDeflateSync(bytes, expected, maximum);
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
