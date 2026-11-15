import test from 'node:test';
import assert from 'node:assert/strict';
import {renderRiceWebScene} from '../web/rice-native-web-scene-renderer.mjs';

test('native scene renderer rejects HTML records before painting', () => {
    const html = new TextEncoder().encode('<button>Untrusted document</button>');
    const bytes = new Uint8Array((12 + html.length + 3) & ~3);
    const view = new DataView(bytes.buffer);
    bytes[0] = 28;
    view.setUint32(4, bytes.length, true);
    view.setUint32(8, html.length, true);
    bytes.set(html, 12);
    assert.throws(() => renderRiceWebScene({}, {
        displayList: bytes, viewportWidth: 100, viewportHeight: 100,
        rect: {x: 0, y: 0, width: 100, height: 100},
    }), /HTML document records are unavailable/);
});
