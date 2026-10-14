#!/usr/bin/env node
import { lstat, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { deflateRawSync } from 'node:zlib';
import process from "node:process";

import { validateRiceAppManifest,pinRiceAppSourceFiles } from "../sdk/package-manifest.mjs";

function crc32(bytes) {
    let crc = 0xffffffff;
    for (const byte of bytes) {
        crc ^= byte;
        for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
    return (crc ^ 0xffffffff) >>> 0;
}

function u16(output, value) { output.push(value & 0xff, (value >>> 8) & 0xff); }
function u32(output, value) { u16(output, value); u16(output, value >>> 16); }

async function collect(root, relative = "") {
    const output = [];
    for (const name of (await readdir(path.join(root, relative))).sort()) {
        if (name.startsWith(".") || name === "node_modules") continue;
        const child = path.posix.join(relative, name);
        const info = await lstat(path.join(root, child));
        if (info.isSymbolicLink()) throw new TypeError(`symbolic link ${child} is not packageable`);
        if (info.isDirectory()) output.push(...await collect(root, child));
        else if (info.isFile()) output.push({ name: child, bytes: new Uint8Array(await readFile(path.join(root, child))) });
    }
    return output;
}

function archive(files) {
    const parts = [];
    let localLength = 0;
    let expandedLength = 0;
    const central = [];
    if (files.length > 65535) throw new RangeError(".riceapp has too many files");
    for (const file of files) {
        const local = [];
        const name = new TextEncoder().encode(file.name);
        if (name.length > 65535) throw new RangeError(".riceapp filename exceeds ZIP limits");
        const offset = localLength;
        const checksum = crc32(file.bytes);
        expandedLength += file.bytes.length;
        if (expandedLength > 32 * 1024 * 1024) throw new RangeError('.riceapp expanded data exceeds 32 MiB');
        const compressed = deflateRawSync(file.bytes, { level: 6 });
        const method = compressed.length < file.bytes.length ? 8 : 0;
        const payload = method ? compressed : file.bytes;
        u32(local, 0x04034b50); u16(local, 20); u16(local, 0x0800); u16(local, method);
        u16(local, 0); u16(local, 0); u32(local, checksum); u32(local, payload.length);
        u32(local, file.bytes.length); u16(local, name.length); u16(local, 0);
        local.push(...name);
        const header = Uint8Array.from(local);
        parts.push(header, payload);
        localLength += header.byteLength + payload.byteLength;
        if (localLength > 16 * 1024 * 1024) throw new RangeError(".riceapp exceeds 16 MiB");
        u32(central, 0x02014b50); u16(central, 20); u16(central, 20); u16(central, 0x0800);
        u16(central, method); u16(central, 0); u16(central, 0); u32(central, checksum);
        u32(central, payload.length); u32(central, file.bytes.length); u16(central, name.length);
        u16(central, 0); u16(central, 0); u16(central, 0); u16(central, 0); u32(central, 0);
        u32(central, offset); central.push(...name);
    }
    const end = [];
    u32(end, 0x06054b50); u16(end, 0); u16(end, 0); u16(end, files.length);
    u16(end, files.length); u32(end, central.length); u32(end, localLength); u16(end, 0);
    const length = localLength + central.length + end.length;
    if (length > 16 * 1024 * 1024) throw new RangeError(".riceapp exceeds 16 MiB");
    parts.push(Uint8Array.from(central), Uint8Array.from(end));
    const output = new Uint8Array(length);
    let offset = 0;
    for (const part of parts) { output.set(part, offset); offset += part.byteLength; }
    return output;
}

const [source, destination] = process.argv.slice(2);
if (!source || !destination) {
    console.error("usage: riceapp-pack <app-directory> <output.riceapp>");
    process.exitCode = 2;
} else {
    const root = path.resolve(source);
    const manifest = validateRiceAppManifest(JSON.parse(await readFile(path.join(root, "manifest.json"), "utf8")));
    const files = await collect(root);
    if (!files.some(file => file.name === manifest.entry)) throw new Error(`missing ${manifest.entry}`);
    const pinned=pinRiceAppSourceFiles(files.map(file=>[file.name,file.bytes]));
    const bytes = archive([...pinned].map(([name,bytes])=>({name,bytes})));
    if (bytes.length > 16 * 1024 * 1024) throw new RangeError(".riceapp exceeds 16 MiB");
    await writeFile(path.resolve(destination), bytes);
    console.log(`packed ${manifest.id} ${manifest.version} (${files.length} files, ${bytes.length} bytes)`);
}
