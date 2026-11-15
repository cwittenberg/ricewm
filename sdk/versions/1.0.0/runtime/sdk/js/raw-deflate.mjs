// Portable bounded DEFLATE shared by SDK package editing and extension import.
const MAX_OUTPUT_BYTES = 32 * 1024 * 1024;

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
    return decodeBounded(input, expected, true);
}

/** Raw streams such as draw.io pages have no decoded-size field. Keep their
 * allocation and expansion bounded without weakening exact-size ZIP reads. */
export function inflateRiceRawDeflateBounded(input, maximum = 8 * 1024 * 1024) {
    if (!(input instanceof Uint8Array) || !Number.isSafeInteger(maximum) || maximum < 1 || maximum > MAX_OUTPUT_BYTES) throw RangeError('Invalid bounded DEFLATE input');
    return decodeBounded(input, maximum, false);
}

function decodeBounded(input, expected, exact) {
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
    if (exact && cursor !== output.byteLength) throw new RangeError("DEFLATE output size is invalid");
    return exact ? output : output.slice(0, cursor);
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

