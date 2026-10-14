import { inflateRiceRawDeflate } from "./gnome-extension-host/rice-zip-reader.mjs";

import {decompressRiceXz} from "./rice-xz.mjs";

const MAX_ENTRIES = 4_096;
const MAX_BYTES = 256 * 1024 * 1024;
const CHUNK_BYTES = 64 * 1024;
const ZIP_LOCAL = 0x04034b50;
const ZIP_CENTRAL = 0x02014b50;
const ZIP_EOCD = 0x06054b50;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

function failure(message, code = "EINVAL") {
    const error = new Error(message);
    error.code = code;
    return error;
}

function basename(path) {
    return path.slice(path.lastIndexOf("/") + 1);
}

function join(parent, name) {
    return parent === "/" ? `/${name}` : `${parent}/${name}`;
}

function safeEntryName(value) {
    const name = String(value).replace(/\/+$/u, "");
    const parts = name.split("/");
    if (!name || name.startsWith("/") || name.includes("\\") ||
        name.includes("\0") || parts.some(part => !part || part === "." || part === "..")) {
        throw failure("archive contains an unsafe path");
    }
    return name;
}

function crc32(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit += 1) {
            crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
        }
    }
    return (crc ^ 0xffffffff) >>> 0;
}

function concat(parts) {
    const length = parts.reduce((total, part) => total + part.byteLength, 0);
    if (!Number.isSafeInteger(length) || length > MAX_BYTES * 2) {
        throw failure("archive output exceeds the 512 MiB bound", "EFBIG");
    }
    const output = new Uint8Array(length);
    let offset = 0;
    for (const part of parts) {
        output.set(part, offset);
        offset += part.byteLength;
    }
    return output;
}

async function streamTransform(bytes, format, decompress = false) {
    const Transform = decompress ? globalThis.DecompressionStream : globalThis.CompressionStream;
    if (typeof Transform !== "function") {
        throw failure(`${format} compression is unavailable in this browser`, "ENOTSUP");
    }
    let stream;
    try {
        stream = new Blob([bytes]).stream().pipeThrough(new Transform(format));
    } catch {
        throw failure(`${format} compression is unavailable in this browser`, "ENOTSUP");
    }
    const reader = stream.getReader();
    const chunks = [];
    let size = 0;
    while (true) {
        const { value, done } = await reader.read();
        if (done) break;
        size += value.byteLength;
        if (size > MAX_BYTES) {
            await reader.cancel();
            throw failure("expanded archive exceeds the 256 MiB bound", "EFBIG");
        }
        chunks.push(value);
    }
    return concat(chunks);
}

async function deflateRaw(bytes) {
    try {
        return await streamTransform(bytes, "deflate-raw");
    } catch (error) {
        if (error?.code !== "ENOTSUP") throw error;
        const wrapped = await streamTransform(bytes, "deflate");
        if (wrapped.byteLength < 6 || (wrapped[1] & 0x20) !== 0) {
            throw failure("browser returned an invalid DEFLATE stream", "EIO");
        }
        return wrapped.slice(2, -4);
    }
}

function throwIfAborted(signal) {
    if (signal?.aborted) throw failure("archive operation was cancelled", "ECANCELED");
}

function yieldToHost() {
    return new Promise(resolve => setTimeout(resolve, 0));
}

async function collect(filesystem, sources, signal, onProgress) {
    const entries = [];
    let total = 0;
    for (const source of sources) measure(filesystem, source, basename(source), entries, value => {
        total += value;
        if (total > MAX_BYTES) throw failure("archive input exceeds the 256 MiB bound", "EFBIG");
    });
    let completed = 0;
    for (const entry of entries) {
        throwIfAborted(signal);
        if (!entry.directory) {
            const bytes = filesystem.mergedFileBytes(entry.source);
            if (!bytes) throw failure(`archive source disappeared: ${entry.source}`, "ENOENT");
            entry.data = bytes.slice();
            for (let offset = 0; offset < bytes.byteLength; offset += CHUNK_BYTES) {
                completed += Math.min(CHUNK_BYTES, bytes.byteLength - offset);
                onProgress(BigInt(completed), BigInt(total));
                await yieldToHost();
                throwIfAborted(signal);
            }
            if (bytes.byteLength === 0) onProgress(BigInt(completed), BigInt(total));
        }
    }
    return { entries, total };
}

function measure(filesystem, source, name, entries, addBytes) {
    if (entries.length >= MAX_ENTRIES) throw failure("archive exceeds the 4096-entry bound", "E2BIG");
    const kind = filesystem.lstatKind(source);
    if (kind === 1) {
        const size = filesystem.lstatSize(source);
        if (size < 0) throw failure(`cannot stat archive source: ${source}`, "ENOENT");
        addBytes(size);
        entries.push({ source, name, directory: false, data: null });
        return;
    }
    if (kind !== 2) throw failure("archives support regular files and directories only", "ENOTSUP");
    entries.push({ source, name: `${name}/`, directory: true, data: new Uint8Array(0) });
    for (const child of filesystem.listDirectory(source).slice(2)) {
        measure(filesystem, join(source, child.name), `${name}/${child.name}`, entries, addBytes);
    }
}

/** Shared in-memory ZIP writer for declarative asset packages. */
export async function createRiceZip(files) {
    if (!(files instanceof Map) || files.size > MAX_ENTRIES) throw failure("Invalid ZIP file table");
    let bytes = 0;
    const entries = [...files].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([name, value]) => {
        if (safeEntryName(name) !== name || name.endsWith("/") || !(value instanceof Uint8Array)) throw failure("Invalid ZIP file entry");
        bytes += value.byteLength;
        if (bytes > MAX_BYTES) throw failure("ZIP input exceeds its size bound", "EFBIG");
        return {name, data: value.slice(), directory: false};
    });
    return zipArchive(entries);
}

async function zipArchive(entries) {
    const locals = [];
    const centrals = [];
    let offset = 0;
    for (const entry of entries) {
        const name = encoder.encode(entry.name);
        if (name.byteLength > 0xffff) throw failure("ZIP entry name is too long");
        const data = entry.data;
        let stored = data;
        let method = 0;
        if (!entry.directory && data.byteLength > 0) {
            try {
                const compressed = await deflateRaw(data);
                if (compressed.byteLength < data.byteLength) {
                    stored = compressed;
                    method = 8;
                }
            } catch (error) {
                if (error?.code !== "ENOTSUP") throw error;
            }
        }
        const crc = entry.directory ? 0 : crc32(data);
        const local = new Uint8Array(30 + name.byteLength + stored.byteLength);
        const view = new DataView(local.buffer);
        view.setUint32(0, ZIP_LOCAL, true);
        view.setUint16(4, 20, true);
        view.setUint16(6, 0x0800, true);
        view.setUint16(8, method, true);
        view.setUint32(14, crc, true);
        view.setUint32(18, stored.byteLength, true);
        view.setUint32(22, data.byteLength, true);
        view.setUint16(26, name.byteLength, true);
        local.set(name, 30);
        local.set(stored, 30 + name.byteLength);
        locals.push(local);

        const central = new Uint8Array(46 + name.byteLength);
        const centralView = new DataView(central.buffer);
        centralView.setUint32(0, ZIP_CENTRAL, true);
        centralView.setUint16(4, 0x0314, true);
        centralView.setUint16(6, 20, true);
        centralView.setUint16(8, 0x0800, true);
        centralView.setUint16(10, method, true);
        centralView.setUint32(16, crc, true);
        centralView.setUint32(20, stored.byteLength, true);
        centralView.setUint32(24, data.byteLength, true);
        centralView.setUint16(28, name.byteLength, true);
        centralView.setUint32(38, entry.directory ? 0x41ed0010 : 0x81a40000, true);
        centralView.setUint32(42, offset, true);
        central.set(name, 46);
        centrals.push(central);
        offset += local.byteLength;
    }
    const centralSize = centrals.reduce((sum, value) => sum + value.byteLength, 0);
    const end = new Uint8Array(22);
    const view = new DataView(end.buffer);
    view.setUint32(0, ZIP_EOCD, true);
    view.setUint16(8, entries.length, true);
    view.setUint16(10, entries.length, true);
    view.setUint32(12, centralSize, true);
    view.setUint32(16, offset, true);
    return concat([...locals, ...centrals, end]);
}

function tarText(target, offset, length, value) {
    const bytes = encoder.encode(value);
    if (bytes.byteLength > length) throw failure("TAR entry name is too long");
    target.set(bytes, offset);
}

function tarOctal(target, offset, length, value) {
    const text = Math.trunc(value).toString(8).padStart(length - 1, "0");
    tarText(target, offset, length, `${text}\0`);
}

function splitTarName(name) {
    const bytes = encoder.encode(name);
    if (bytes.byteLength <= 100) return [name, ""];
    for (let offset = name.lastIndexOf("/"); offset > 0; offset = name.lastIndexOf("/", offset - 1)) {
        const prefix = name.slice(0, offset);
        const leaf = name.slice(offset + 1);
        if (encoder.encode(prefix).byteLength <= 155 && encoder.encode(leaf).byteLength <= 100) {
            return [leaf, prefix];
        }
    }
    throw failure("TAR entry name is too long");
}

function tarArchive(entries) {
    const parts = [];
    for (const entry of entries) {
        const canonical = entry.directory ? entry.name.slice(0, -1) : entry.name;
        const [name, prefix] = splitTarName(canonical);
        const header = new Uint8Array(512);
        tarText(header, 0, 100, name);
        tarOctal(header, 100, 8, entry.directory ? 0o755 : 0o644);
        tarOctal(header, 108, 8, 0);
        tarOctal(header, 116, 8, 0);
        tarOctal(header, 124, 12, entry.directory ? 0 : entry.data.byteLength);
        tarOctal(header, 136, 12, 0);
        header.fill(0x20, 148, 156);
        header[156] = entry.directory ? 0x35 : 0x30;
        tarText(header, 257, 6, "ustar\0");
        tarText(header, 263, 2, "00");
        tarText(header, 345, 155, prefix);
        tarOctal(header, 148, 8, header.reduce((sum, byte) => sum + byte, 0));
        parts.push(header);
        if (!entry.directory) {
            parts.push(entry.data);
            const padding = (512 - (entry.data.byteLength % 512)) % 512;
            if (padding) parts.push(new Uint8Array(padding));
        }
    }
    parts.push(new Uint8Array(1024));
    return concat(parts);
}

function putAtomic(filesystem, destination, bytes, request) {
    if (filesystem.lstatKind(destination) !== 0) throw failure("archive destination already exists", "EEXIST");
    const parent = destination.slice(0, destination.lastIndexOf("/")) || "/";
    if (filesystem.lstatKind(parent) !== 2) throw failure("archive destination directory is missing", "ENOENT");
    const temporary = `${destination}.ricearchive-${request}`;
    if (filesystem.lstatKind(temporary) !== 0) throw failure("archive temporary path already exists", "EEXIST");
    try {
        filesystem.putFile(temporary, bytes, 0o644);
        const status = filesystem.rename(temporary, destination);
        if (status < 0) throw failure(`archive commit failed (${status})`, "EIO");
    } catch (error) {
        if (filesystem.lstatKind(temporary) !== 0) filesystem.unlink(temporary);
        throw error;
    }
}

export async function createArchive(filesystem, operation) {
    const { sources, destination, format, request, signal, onProgress } = operation;
    if (format === "gzip" && (sources.length !== 1 || filesystem.lstatKind(sources[0]) !== 1)) {
        throw failure("GZIP requires exactly one regular file");
    }
    const { entries } = await collect(filesystem, sources, signal, onProgress);
    throwIfAborted(signal);
    let output;
    if (format === "zip") output = await zipArchive(entries);
    else if (format === "tar-gzip") output = await streamTransform(tarArchive(entries), "gzip");
    else output = await streamTransform(entries[0].data, "gzip");
    throwIfAborted(signal);
    putAtomic(filesystem, destination, output, request);
}

async function zipEntries(bytes, signal, onProgress) {
    if (bytes.byteLength < 22) throw failure("ZIP central directory is missing");
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let eocd = -1;
    for (let offset = bytes.byteLength - 22; offset >= Math.max(0, bytes.byteLength - 65_557); offset -= 1) {
        if (view.getUint32(offset, true) === ZIP_EOCD) { eocd = offset; break; }
    }
    if (eocd < 0) throw failure("ZIP central directory is missing");
    const count = view.getUint16(eocd + 10, true);
    let cursor = view.getUint32(eocd + 16, true);
    if (count > MAX_ENTRIES) throw failure("ZIP exceeds the 4096-entry bound", "E2BIG");
    const output = [];
    let expanded = 0;
    for (let index = 0; index < count; index += 1) {
        throwIfAborted(signal);
        if (cursor + 46 > eocd || view.getUint32(cursor, true) !== ZIP_CENTRAL) throw failure("ZIP directory is invalid");
        const flags = view.getUint16(cursor + 8, true);
        const method = view.getUint16(cursor + 10, true);
        const expectedCrc = view.getUint32(cursor + 16, true);
        const compressedSize = view.getUint32(cursor + 20, true);
        const size = view.getUint32(cursor + 24, true);
        const nameLength = view.getUint16(cursor + 28, true);
        const extraLength = view.getUint16(cursor + 30, true);
        const commentLength = view.getUint16(cursor + 32, true);
        const localOffset = view.getUint32(cursor + 42, true);
        const end = cursor + 46 + nameLength + extraLength + commentLength;
        if (end > eocd || (flags & 1) || ![0, 8].includes(method)) throw failure("ZIP uses an unsupported feature", "ENOTSUP");
        const rawName = decoder.decode(bytes.subarray(cursor + 46, cursor + 46 + nameLength));
        const directory = rawName.endsWith("/");
        const name = safeEntryName(rawName);
        const unixMode = view.getUint8(cursor + 5) === 3 ? view.getUint32(cursor + 38, true) >>> 16 : 0;
        const mode = unixMode ? unixMode & 0o777 : directory ? 0o755 : 0o644;
        cursor = end;
        if (localOffset + 30 > bytes.byteLength || view.getUint32(localOffset, true) !== ZIP_LOCAL) throw failure("ZIP local entry is invalid");
        const dataOffset = localOffset + 30 + view.getUint16(localOffset + 26, true) + view.getUint16(localOffset + 28, true);
        if (dataOffset + compressedSize > bytes.byteLength) throw failure("ZIP data is truncated");
        let data = new Uint8Array(0);
        if (!directory) {
            if (expanded + size > MAX_BYTES) throw failure("ZIP output exceeds the 256 MiB bound", "EFBIG");
            const compressed = bytes.subarray(dataOffset, dataOffset + compressedSize);
            data = method === 0
                ? compressed.slice()
                : await inflateRiceRawDeflate(compressed, size, MAX_BYTES - expanded);
            if (data.byteLength !== size || crc32(data) !== expectedCrc) throw failure("ZIP integrity check failed");
            expanded += data.byteLength;
        }
        output.push({ name, directory, data, mode });
        onProgress(BigInt(Math.min(bytes.byteLength, dataOffset + compressedSize)), BigInt(bytes.byteLength));
        await yieldToHost();
    }
    onProgress(BigInt(bytes.byteLength), BigInt(bytes.byteLength));
    return output;
}

function zipCatalogue(bytes) {
    if (bytes.byteLength < 22) throw failure("ZIP central directory is missing");
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let eocd = -1;
    for (let offset = bytes.byteLength - 22;
        offset >= Math.max(0, bytes.byteLength - 65_557); offset -= 1) {
        if (view.getUint32(offset, true) === ZIP_EOCD) {
            eocd = offset;
            break;
        }
    }
    if (eocd < 0) throw failure("ZIP central directory is missing");
    const count = view.getUint16(eocd + 10, true);
    let cursor = view.getUint32(eocd + 16, true);
    if (count > MAX_ENTRIES) {
        throw failure("ZIP exceeds the 4096-entry bound", "E2BIG");
    }
    const entries = [];
    for (let index = 0; index < count; index += 1) {
        if (cursor + 46 > eocd || view.getUint32(cursor, true) !== ZIP_CENTRAL) {
            throw failure("ZIP directory is invalid");
        }
        const flags = view.getUint16(cursor + 8, true);
        const method = view.getUint16(cursor + 10, true);
        const compressedSize = view.getUint32(cursor + 20, true);
        const size = view.getUint32(cursor + 24, true);
        const nameLength = view.getUint16(cursor + 28, true);
        const extraLength = view.getUint16(cursor + 30, true);
        const commentLength = view.getUint16(cursor + 32, true);
        const end = cursor + 46 + nameLength + extraLength + commentLength;
        if (end > eocd || (flags & 1) || ![0, 8].includes(method)) {
            throw failure("ZIP uses an unsupported feature", "ENOTSUP");
        }
        const rawName = decoder.decode(
            bytes.subarray(cursor + 46, cursor + 46 + nameLength),
        );
        entries.push(Object.freeze({
            name: safeEntryName(rawName),
            directory: rawName.endsWith("/"),
            size,
            compressedSize,
        }));
        cursor = end;
    }
    return entries;
}

/**
 * Return the real top-level Shell namespace of a ZIP without inflating file
 * data. The catalogue and extraction paths enforce the same name, feature,
 * entry-count, and byte bounds.
 */
export function listArchiveTopLevel(filesystem, archive) {
    const source = filesystem.mergedFileBytes(archive);
    if (!source) throw failure("archive file was not found", "ENOENT");
    if (source.byteLength > MAX_BYTES) {
        throw failure("archive exceeds the 256 MiB bound", "EFBIG");
    }
    if (source[0] !== 0x50 || source[1] !== 0x4b) {
        throw failure("Shell archive namespace is not supported", "ENOTSUP");
    }
    const catalogue = zipCatalogue(source);
    const topLevel = new Map();
    for (const entry of catalogue) {
        const separator = entry.name.indexOf("/");
        const name = separator < 0 ? entry.name : entry.name.slice(0, separator);
        const directory = separator >= 0 || entry.directory;
        const previous = topLevel.get(name);
        if (previous && !previous.directory && directory) {
            throw failure("archive places a child below a regular file");
        }
        if (!previous) {
            topLevel.set(name, Object.freeze({
                name,
                directory,
                size: directory ? 0 : entry.size,
            }));
        }
    }
    return Object.freeze([...topLevel.values()]);
}

function tarNumber(bytes) {
    const text = new TextDecoder().decode(bytes).replace(/\0.*$/su, "").trim();
    return text ? Number.parseInt(text, 8) : 0;
}

function tarPax(data) {
    if (data.length > 256 * 1024) throw failure("TAR metadata exceeds size limit", "EFBIG");
    const attributes = Object.create(null);
    for (let offset = 0; offset < data.length;) {
        const space = data.indexOf(32, offset);
        if (space < 0 || space - offset > 10) throw failure("Invalid TAR metadata record");
        const lengthText = decoder.decode(data.subarray(offset, space));
        const length = Number(lengthText), end = offset + length;
        if (!/^[1-9][0-9]*$/.test(lengthText) || !Number.isSafeInteger(end) ||
            end > data.length || end <= space + 2 || data[end - 1] !== 10)
            throw failure("Invalid TAR metadata length");
        const record = decoder.decode(data.subarray(space + 1, end - 1));
        const equal = record.indexOf("=");
        if (equal < 1) throw failure("Invalid TAR metadata value");
        const key = record.slice(0, equal);
        if (key.startsWith("GNU.sparse")) throw failure("Sparse TAR entries are unsupported", "ENOTSUP");
        if (key === "path" || key === "size") attributes[key] = record.slice(equal + 1);
        offset = end;
    }
    return attributes;
}

function tarEntries(bytes, signal, onProgress) {
    const output = [];
    let cursor = 0, expanded = 0, headers = 0, local = {}, global = {};
    while (cursor + 512 <= bytes.byteLength) {
        throwIfAborted(signal);
        const header = bytes.subarray(cursor, cursor + 512);
        if (header.every(byte => byte === 0)) break;
        if (++headers > MAX_ENTRIES * 4) throw failure("TAR has too many headers", "EFBIG");
        const storedChecksum = tarNumber(header.subarray(148, 156));
        const actualChecksum = header.reduce((sum, byte, index) => sum + (index >= 148 && index < 156 ? 0x20 : byte), 0);
        if (storedChecksum !== actualChecksum) throw failure("TAR header checksum is invalid");
        const text = (start, end) => decoder.decode(header.subarray(start, end)).replace(/\0.*$/su, "");
        const kind = header[156], metadata = [0x78, 0x67, 0x4c].includes(kind);
        const attrs = {...global, ...local};
        const sizeText = !metadata && attrs.size;
        const size = sizeText ? (/^[0-9]+$/.test(sizeText) ? Number(sizeText) : NaN) : tarNumber(header.subarray(124, 136));
        if (!Number.isSafeInteger(size) || size < 0 || size > MAX_BYTES) throw failure("Invalid TAR entry size");
        const dataStart = cursor + 512, dataEnd = dataStart + size;
        if (dataEnd > bytes.byteLength) throw failure("TAR data is truncated");
        const data = bytes.subarray(dataStart, dataEnd);
        if (metadata) {
            if (kind === 0x4c) {
                if (size > 256 * 1024) throw failure("TAR name exceeds size limit", "EFBIG");
                local.path = decoder.decode(data).replace(/\0.*$/su, "");
            } else {
                const values = tarPax(data);
                if (kind === 0x67) {
                    for (const [key, value] of Object.entries(values)) {
                        if (value === "") delete global[key]; else global[key] = value;
                    }
                } else local = {...local, ...values};
            }
        } else {
            if (![0, 0x30, 0x35].includes(kind)) throw failure("TAR contains an unsupported entry type", "ENOTSUP");
            const prefix = text(345, 500), namePart = text(0, 100);
            const path = (attrs.path || (prefix ? `${prefix}/${namePart}` : namePart)).replace(/^(?:\.\/)+/u, "");
            local = {};
            // A TAR root directory represents the destination itself.
            if (!(kind === 0x35 && (path === "" || path === "."))) {
                const name = safeEntryName(path);
                expanded += size;
                if (expanded > MAX_BYTES || output.length >= MAX_ENTRIES) throw failure("TAR output exceeds its safety bound", "EFBIG");
                output.push({name, directory: kind === 0x35, data: data.slice(), mode: tarNumber(header.subarray(100,108)) & 0o777});
            }
        }
        cursor = dataStart + Math.ceil(size / 512) * 512;
        onProgress(BigInt(Math.min(cursor, bytes.byteLength)), BigInt(bytes.byteLength));
    }
    onProgress(BigInt(bytes.byteLength), BigInt(bytes.byteLength));
    return output;
}

function uniqueTopLevel(filesystem, destination, entries) {
    const mapping = new Map();
    for (const entry of entries) {
        const top = entry.name.split("/", 1)[0];
        if (mapping.has(top)) continue;
        let candidate = top;
        let suffix = 1;
        while (filesystem.lstatKind(join(destination, candidate)) !== 0) {
            candidate = suffix === 1 ? `${top} (extracted)` : `${top} (extracted ${suffix})`;
            suffix += 1;
        }
        mapping.set(top, candidate);
    }
    return entries.map(entry => {
        const [top, ...rest] = entry.name.split("/");
        return { ...entry, name: [mapping.get(top), ...rest].join("/") };
    });
}

function validateEntryTree(entries) {
    const kinds = new Map();
    for (const entry of entries) {
        if (kinds.has(entry.name)) throw failure("archive contains duplicate paths");
        kinds.set(entry.name, entry.directory);
    }
    for (const entry of entries) {
        const parts = entry.name.split("/");
        let parent = "";
        for (const part of parts.slice(0, -1)) {
            parent = parent ? `${parent}/${part}` : part;
            if (kinds.get(parent) === false) {
                throw failure("archive places a child below a regular file");
            }
        }
    }
}

function selectedEntries(entries, members) {
    if (members === undefined) return entries;
    if (!Array.isArray(members) || members.length < 1 || members.length > MAX_ENTRIES) {
        throw failure("archive member selection is invalid");
    }
    const selections = new Map();
    for (const member of members) {
        if (typeof member?.name !== "string" ||
            (member.replacement !== undefined && typeof member.replacement !== "string")) {
            throw failure("archive member selection is invalid");
        }
        const name = safeEntryName(member?.name);
        const replacement = member?.replacement
            ? safeEntryName(member.replacement)
            : name;
        if (name.includes("/") || replacement.includes("/") || selections.has(name)) {
            throw failure("archive member selection is invalid");
        }
        selections.set(name, replacement);
    }
    const found = new Set();
    const output = [];
    for (const entry of entries) {
        const [top, ...rest] = entry.name.split("/");
        const replacement = selections.get(top);
        if (replacement === undefined) continue;
        found.add(top);
        output.push({
            ...entry,
            name: [replacement, ...rest].join("/"),
        });
    }
    if (found.size !== selections.size) {
        throw failure("selected archive member does not exist", "ENOENT");
    }
    return output;
}

function ensureDirectory(filesystem, path) {
    if (filesystem.lstatKind(path) === 2) return;
    if (filesystem.lstatKind(path) !== 0) throw failure(`extraction path is not a directory: ${path}`, "EEXIST");
    const status = filesystem.mkdir(path, 0o755);
    if (status < 0) throw failure(`cannot create extraction directory (${status})`, "EIO");
}

async function writeExtracted(filesystem, destination, entries) {
    if (filesystem.lstatKind(destination) !== 2) throw failure("extraction destination is missing", "ENOENT");
    validateEntryTree(entries);
    const normalized = uniqueTopLevel(filesystem, destination, entries);
    const directories = new Set();
    for (const entry of normalized) {
        const parts = entry.name.split("/");
        if (!entry.directory) parts.pop();
        let path = destination;
        for (const part of parts) {
            path = join(path, part);
            directories.add(path);
        }
    }
    for (const path of [...directories].sort((a, b) => a.length - b.length)) ensureDirectory(filesystem, path);
    for (const entry of normalized) {
        if (entry.directory) continue;
        const path = join(destination, entry.name);
        if (filesystem.lstatKind(path) !== 0) throw failure(`extraction destination exists: ${path}`, "EEXIST");
        if (entry.data.byteLength > 16 * 1024 * 1024 &&
            typeof filesystem.putFileRangeAsync === "function" &&
            typeof filesystem.finishFileRange === "function") {
            await filesystem.putFileRangeAsync(
                path,
                entry.data,
                0,
                entry.mode ?? 0o644,
                true,
                true,
            );
            await filesystem.finishFileRange(path);
        } else {
            filesystem.putFile(path, entry.data, entry.mode ?? 0o644);
        }
    }
}

export async function extractArchive(filesystem, operation) {
    const { archive, destination, signal, onProgress, members } = operation;
    const source = filesystem.mergedFileBytes(archive);
    if (!source) throw failure("archive file was not found", "ENOENT");
    if (source.byteLength > MAX_BYTES) throw failure("archive exceeds the 256 MiB bound", "EFBIG");
    const lower = archive.toLowerCase();
    let entries;
    if (source[0] === 0x50 && source[1] === 0x4b) {
        entries = await zipEntries(source, signal, onProgress);
    } else if (source[0] === 0x1f && source[1] === 0x8b) {
        const expanded = await streamTransform(source, "gzip", true);
        throwIfAborted(signal);
        if (lower.endsWith(".tar.gz") || lower.endsWith(".tgz")) {
            entries = tarEntries(expanded, signal, onProgress);
        } else {
            const name = basename(archive).replace(/\.gz$/iu, "") || "expanded";
            entries = [{ name: safeEntryName(name), directory: false, data: expanded }];
            onProgress(BigInt(source.byteLength), BigInt(source.byteLength));
        }
    } else if (source[0] === 0xfd && source[1] === 0x37 && source[2] === 0x7a &&
               source[3] === 0x58 && source[4] === 0x5a && source[5] === 0) {
        const expanded = await decompressRiceXz(source, {signal, maximumBytes: MAX_BYTES});
        throwIfAborted(signal);
        if (lower.endsWith(".tar.xz") || lower.endsWith(".txz")) {
            entries = tarEntries(expanded, signal, onProgress);
        } else {
            const name = basename(archive).replace(/\.xz$/iu, "") || "expanded";
            entries = [{name: safeEntryName(name), directory: false, data: expanded}];
            onProgress(BigInt(source.byteLength), BigInt(source.byteLength));
        }
    } else if (lower.endsWith(".tar")) {
        entries = tarEntries(source, signal, onProgress);
    } else {
        throw failure("archive format is not supported", "ENOTSUP");
    }
    throwIfAborted(signal);
    await writeExtracted(
        filesystem,
        destination,
        selectedEntries(entries, members),
    );
}
