import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile, readdir, mkdtemp, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {execFileSync} from 'node:child_process';
import {inflateRawSync} from 'node:zlib';
import * as sdk from '@ricewm/app';
import {CURRENT_SDK_VERSION, RICE_SDK_VERSIONS, riceSdkLibrary, resolveRiceSdkVersion} from '../sdk/rice-sdk-versions.mjs';
import {validateRiceAppManifest} from '../sdk/package-manifest.mjs';

const root = new URL('../', import.meta.url);
const examples = new URL('../sdk/examples/', import.meta.url);

test('one exact SDK implementation is selected and unknown releases are rejected', () => {
    assert.equal(CURRENT_SDK_VERSION, '1.0.0');
    assert.deepEqual(RICE_SDK_VERSIONS, ['1.0.0']);
    assert.equal(riceSdkLibrary({version: '1.0.0'}).Button, sdk.Button);
    assert.throws(() => resolveRiceSdkVersion({version: '2.0.0'}));
    assert.throws(() => resolveRiceSdkVersion({version: '^1.0.0'}));
});

test('every reflected capability routes to its exact bridge method and missing grants fail', async () => {
    const calls = [];
    const permissions = sdk.SDK_SERVICE_METHODS.map(m => m.permission);
    const api = sdk.createServices({request(service, operation, args) {
        calls.push({service, operation, args}); return {service, operation, args};
    }}, permissions);
    const actual = Object.entries(api).flatMap(([group, methods]) => Object.keys(methods).map(op => `${group}.${op}`));
    assert.deepEqual(actual, permissions);
    for (const method of sdk.SDK_SERVICE_METHODS) {
        const result = await api[method.service][method.operation]({probe: 1});
        assert.deepEqual(result, {service: method.service, operation: method.operation, args: {probe: 1}});
        assert.equal(sdk.sdkServiceMethod(method.permission), method);
    }
    assert.equal(calls.length, permissions.length);
    await assert.rejects(sdk.createServices(null).files.read({}), /not granted/);
});

test('every included example imports and packages without modifying source', async () => {
    const temporary = await mkdtemp(join(tmpdir(), 'ricewm-sdk-'));
    try {
        for (const name of (await readdir(examples)).sort()) {
            const folder = new URL(`${name}/`, examples);
            const original = await readFile(new URL('manifest.json', folder));
            const manifest = validateRiceAppManifest(JSON.parse(original));
            const app = await import(new URL(manifest.entry, folder));
            assert.equal(typeof app.default.start, 'function', name);
            if (app.default.manifest) assert.equal(app.default.manifest.id, manifest.id, name);
            const destination = join(temporary, name + '.riceapp');
            execFileSync(process.execPath, [new URL('tools/riceapp-pack.mjs', root).pathname, folder.pathname, destination]);
            assert.deepEqual(await readFile(new URL('manifest.json', folder)), original, name);
            const archive = await readFile(destination);
            let offset = 0, packaged;
            while (archive.readUInt32LE(offset) === 0x04034b50) {
                const method = archive.readUInt16LE(offset + 8), length = archive.readUInt32LE(offset + 18);
                const nameLength = archive.readUInt16LE(offset + 26), extra = archive.readUInt16LE(offset + 28);
                const fileName = archive.subarray(offset + 30, offset + 30 + nameLength).toString();
                const start = offset + 30 + nameLength + extra;
                const bytes = archive.subarray(start, start + length);
                if (fileName === 'manifest.json') packaged = JSON.parse((method === 8 ? inflateRawSync(bytes) : bytes).toString());
                offset = start + length;
            }
            assert.equal(packaged.sdk.version, '1.0.0', name);
            assert.equal(packaged.id, manifest.id, name);
        }
    } finally { await rm(temporary, {recursive: true, force: true}); }
});

test('unknown host service permissions are rejected by the public packager', () => {
    assert.throws(() => validateRiceAppManifest({schema: 'ricewm.app/v1', id: 'org.example.invalid', name: 'Invalid', version: '1.0.0', entry: 'app.mjs', permissions: {services: ['files.invented']}}), /unknown service/);
});

test('calculator example runs its controls and copies 7 × 8 = 56 through a local host bridge', async () => {
    const folder = new URL('calc-sdk/', examples);
    const manifest = validateRiceAppManifest(JSON.parse(await readFile(new URL('manifest.json', folder))));
    const {default: definition} = await import(new URL('app.mjs', folder));
    const calls = [];
    const core = new sdk.RiceJavaScriptAppCore(definition, {manifest});
    await core.initialize({callHost(service, operation, args) { calls.push({service, operation, args}); return null; }});
    try {
        await core.launch({app: 1n, arguments: [], entrypoint: 'app.mjs'});
        const app = core.instances.get(1n);
        for (const text of ['7', '×', '8', '=']) {
            const button = [...app.controls.values()].find(c => c instanceof sdk.Button && c.text === text);
            assert.ok(button, `calculator button ${text}`);
            button.sendMessage(sdk.Message.ACTIVATE);
        }
        assert.ok([...app.controls.values()].some(c => c.text === '56' || (c instanceof sdk.TextField && c.getProperty('value') === '56')));
        const copy = [...app.controls.values()].find(c => c instanceof sdk.MenuItem && c.text.startsWith('Copy Result'));
        assert.ok(copy);
        copy.sendMessage(sdk.Message.ACTIVATE);
        assert.ok(calls.some(c => c.service === 'clipboard' && c.operation === 'writeText' && c.args === '56'));
    } finally { await core.close(); }
});
