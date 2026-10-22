import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { webcrypto } from 'node:crypto';
import { RiceApp, Message, createServices } from '../sdk/versions/1.0.0/runtime/sdk/js/index.mjs';
import { issueForms, issueUrl } from '../sdk/examples/feedback/forms.mjs';
import { validateRiceAppManifest } from '../sdk/package-manifest.mjs';

globalThis.crypto ||= webcrypto;
const source = (await readFile(new URL('../sdk/examples/feedback/app.mjs', import.meta.url), 'utf8'))
    .replace("'@ricewm/app'", JSON.stringify(new URL('../sdk/versions/1.0.0/runtime/sdk/js/index.mjs', import.meta.url).href))
    .replace("'./forms.mjs'", JSON.stringify(new URL('../sdk/examples/feedback/forms.mjs', import.meta.url).href));
const { default: definition } = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
const valuesFor = kind => ({ summary: 'Windows & tabs 日本語', ...Object.fromEntries(issueForms[kind].fields.map(field => [field.id, field.label + '\nDetails & ? # + %'])) });

for (const kind of ['bug', 'feature']) {
    test(`${kind}: every field survives the GitHub handoff, including Unicode and URL punctuation`, () => {
        const values = valuesFor(kind), url = new URL(issueUrl(kind, values));
        assert.equal(url.origin, 'https://github.com');
        assert.equal(url.pathname, '/cwittenberg/ricewm/issues/new');
        assert.equal(url.searchParams.get('template'), issueForms[kind].template);
        assert.equal(url.searchParams.get('title'), issueForms[kind].prefix + values.summary);
        for (const field of issueForms[kind].fields) assert.equal(url.searchParams.get(field.id), values[field.id]);
        for (const field of issueForms[kind].fields.filter(field => field.required)) {
            assert.throws(() => issueUrl(kind, { ...values, [field.id]: '  ' }), /Please complete/);
        }
        assert.throws(() => issueUrl(kind, { ...values, summary: '' }), /summary/);
        assert.throws(() => issueUrl(kind, { ...values, summary: 'x'.repeat(161) }), /160/);
        assert.throws(() => issueUrl(kind, { ...values, extra: '日本語'.repeat(2000) }), /too long/);
    });

    test(`${kind}: native form validates before opening, preserves the draft on failure and allows retry`, async () => {
        const manifestFile = kind === 'bug' ? 'manifest.json' : 'feature.json';
        const manifest = validateRiceAppManifest(JSON.parse(await readFile(new URL('../sdk/examples/feedback/' + manifestFile, import.meta.url))));
        assert.deepEqual(manifest.permissions.services, ['host.openExternal']);
        const calls = [];
        let fail = true;
        const services = createServices({ async request(group, operation, parameters) {
            calls.push({ group, operation, parameters });
            if (fail) throw Error('Bowl could not be opened');
            return { requested: true, application: 'bowl' };
        } }, manifest.permissions);
        const app = new RiceApp({ manifest, services });
        definition.start({ app, services, createWindow: options => app.createWindow(options) });
        const window = [...app.windows.values()][0], root = window.root;
        assert.equal(window.title, issueForms[kind].name);
        assert.equal(calls.length, 0);
        await root.find('report-on-github').emit(Message.ACTIVATE);
        assert.equal(calls.length, 0);
        assert.match(root.find('status').text, /summary/);
        const values = valuesFor(kind);
        for (const [id, value] of Object.entries(values)) root.find(id).setProperty('value', value);
        await root.find('report-on-github').emit(Message.ACTIVATE);
        assert.match(root.find('status').text, /Bowl could not/);
        assert(root.find('report-on-github').enabled);
        assert.equal(root.find('summary').getProperty('value'), values.summary);
        fail = false;
        await root.find('report-on-github').emit(Message.ACTIVATE);
        assert.equal(calls.length, 2);
        assert.equal(calls[1].group, 'host');
        assert.equal(calls[1].operation, 'openExternal');
        assert.equal(calls[1].parameters.url, issueUrl(kind, values));
        assert.match(root.find('status').text, /submit it there/);
        assert.equal(app.windows.size, 1);
        root.find('cancel').emit(Message.ACTIVATE);
        assert(window.hostRequests.some(request => request.operation === 'close'));
        app.close();
    });
}
