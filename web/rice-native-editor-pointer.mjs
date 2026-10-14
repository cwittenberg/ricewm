import {isDocumentEditor, documentEditorHit, documentEditorMetadata} from './rice-document-editor-layout.mjs';
import { documentTextHit, ensureDocumentTextFonts } from './rice-document-text-metrics.mjs';
import { RiceNativeEditorRenderer } from './rice-native-editor-renderer.mjs';
import { ensureEditorContentFonts } from './rice-editor-content-fonts.mjs';

/** Measured hits are advisory until native validates generation, target and text. */
export class RiceNativeEditorPointer {
    constructor(document = globalThis.document) {
        const canvas = document?.createElement?.('canvas') ||
            (typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(1, 1) : null);
        const context = canvas?.getContext?.('2d');
        this.context = context;
        this.fontEnvironment = document || globalThis;
        this.renderer = context ? new RiceNativeEditorRenderer(context) : null;
        this.snapshots = new Map(); this.capture = new Map(); this.retainedBytes = 0;
    }
    retain(snapshot, epoch) {
        const key = `${snapshot.app}:${snapshot.window}`;
        // Retain only editor metadata; never pin slide images or vector frames.
        const commands = snapshot.commands.flatMap((command, index) => (command.role === 'editor' || command.documentEditor)
            ? [{ ...command, popupLayer: snapshot.popup != null && index >= snapshot.popup.commandStart }] : []);
        const bytes = commands.reduce((total, command) => total + command.text.length * 2 + 128, 0);
        if (bytes > 2 * 1024 * 1024) throw new RangeError('Native editor measurement metadata exceeds 2 MiB');
        this.retainedBytes -= this.snapshots.get(key)?.bytes || 0;
        this.snapshots.delete(key);
        while (this.snapshots.size >= 64 || this.retainedBytes + bytes > 2 * 1024 * 1024) {
            const oldest = this.snapshots.keys().next().value;
            this.retainedBytes -= this.snapshots.get(oldest).bytes;
            this.snapshots.delete(oldest); this.capture.delete(oldest);
        }
        this.snapshots.set(key, { snapshot: { app: snapshot.app, window: snapshot.window,
            width: snapshot.width, height: snapshot.height, popup: snapshot.popup, commands }, epoch, bytes });
        this.retainedBytes += bytes;
        // Main canvas and runtime worker have distinct FontFaceSets. Load the
        // same exact faces in both realms before interpreting measured input.
        for (const command of snapshot.commands) {
            const pending = command.documentEditor ? ensureDocumentTextFonts(command,this.fontEnvironment) : command.role === 'editor' ? ensureEditorContentFonts(command.text,this.fontEnvironment) : null;
            void pending?.catch(() => {});
        }
    }
    async prepare(app, window) {
        const retained = this.snapshots.get(`${app}:${window}`);
        const pending = [];
        for (const command of retained?.snapshot.commands || []) if (command.role === 'editor' || command.documentEditor) {
            const work = command.documentEditor ? ensureDocumentTextFonts(command,this.fontEnvironment) : ensureEditorContentFonts(command.text, this.fontEnvironment);
            if (work) pending.push(work);
        }
        if (!pending.length) return false;
        await Promise.all(pending);
        return true;
    }
    sample(bridge, app, window, kind, x, y, button) {
        if (!this.renderer) return null;
        const key = `${app}:${window}`;
        if (![0, 1, 2].includes(kind)) return null;
        const retained = this.snapshots.get(key);
        if (!retained?.epoch) return null;
        const target = (kind === 1 ? null : this.capture.get(key)) || bridge.hitTest(app, window, x, y);
        if (!target) return null;
        // Styled labels/actions use shaped text too, but never own a caret.
        if (bridge.widgetActions?.(app,window,target)?.editable === false) return null;
        const popup = retained.snapshot.popup?.rect;
        const inPopup = popup && x >= popup.x && y >= popup.y
            && x < popup.x + popup.width && y < popup.y + popup.height;
        const command = retained.snapshot.commands.find(command => {
            if (command.type !== 'text' || command.role !== 'editor' && !command.documentEditor) return false;
            // Hit testing the center of an editor hidden beneath a popup can
            // return the popup's button. Never attach that editor's measured
            // caret to a target from a different paint layer.
            if (inPopup && !command.popupLayer) return false;
            if(isDocumentEditor(command))return documentEditorMetadata(command).id===BigInt(target);
            const r = command.rect;
            return bridge.hitTest(app, window, r.x + r.width / 2, r.y + r.height / 2) === target;
        });
        if (!command) return null;
        const pending = command.documentEditor ? ensureDocumentTextFonts(command,this.fontEnvironment) : ensureEditorContentFonts(command.text, this.fontEnvironment);
        if (pending) {
            void pending.catch(() => {});
            throw new Error('Native editor fonts are not ready; prepare measured input before dispatch');
        }
        const caret = isDocumentEditor(command) ? documentEditorHit(this.context,command,x,y) : command.documentEditor ? documentTextHit(this.context,command,x) : this.renderer.hit(command, x, y);
        if (caret === null) return null;
        const text = new TextEncoder().encode(command.text);
        if (text.byteLength > 16384) return null;
        const bytes = new Uint8Array(32 + text.byteLength), view = new DataView(bytes.buffer);
        bytes.set([69, 68, 77, 49]); view.setUint32(4, bytes.byteLength, true);
        view.setBigUint64(8, BigInt(target), true); view.setBigUint64(16, retained.epoch, true);
        view.setUint32(24, caret, true); view.setUint32(28, text.byteLength, true); bytes.set(text, 32);
        return { bytes, target, multiline: command.role === 'editor' || isDocumentEditor(command) };
    }
    accepted(app, window, kind, button, target) {
        const key = `${app}:${window}`;
        if ([1, 2, 4].includes(kind)) this.capture.delete(key);
        if (kind === 1 && button === 1 && target) this.capture.set(key, target);
    }
    removeApp(app) {
        const prefix=`${BigInt(app)}:`;
        for(const [key,record] of this.snapshots) if(key.startsWith(prefix)) {
            this.retainedBytes-=record.bytes;this.snapshots.delete(key);
        }
        for(const key of this.capture.keys()) if(key.startsWith(prefix))this.capture.delete(key);
    }
}
