/** Turn authorized guest media rectangles into native video surfaces. */
export function embedBowlMedia(snapshot, ownsSession = () => false) {
    const commands = [];
    let changed = false;
    for (const scene of snapshot.commands) {
        const sceneIndex = commands.length;
        commands.push(scene);
        const mediaRegions = [];
        if (scene.type !== 'web-scene' || !scene.displayList?.byteLength) continue;
        const bytes = scene.displayList;
        const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
        const scale = scene.rect.width / Math.max(1, scene.viewportWidth);
        for (let offset = 0; offset + 8 <= bytes.length;) {
            const words = view.getUint16(offset + 2, true);
            const length = words ? words * 4 : view.getUint32(offset + 4, true);
            if (length < 8 || offset + length > bytes.length) break;
            if (bytes[offset] === 27 && length === 32) {
                const session = view.getBigUint64(offset + 24, true);
                if (session > 0n && ownsSession(snapshot.app, session)) {
                    const values = [0, 4, 8, 12].map(at => view.getFloat32(offset + 8 + at, true));
                    if (values.every(Number.isFinite) && values[2] > 0 && values[3] > 0) {
                        commands.push({ type: 'push-clip', rect: scene.rect }, {
                            type: 'media', request: session, embedded: true,
                            rect: { x: scene.rect.x + values[0] * scale,
                                y: scene.rect.y + values[1] * scale,
                                width: values[2] * scale, height: values[3] * scale },
                            quarterTurns: 0, scaleMode: 'fit', scale: 1, panX: 0, panY: 0,
                        }, { type: 'pop-clip' });
                        mediaRegions.push({ offset, request: String(session) });
                        changed = true;
                    }
                }
            }
            offset += length;
        }
        if (mediaRegions.length) commands[sceneIndex] = { ...scene, mediaRegions };
    }
    return changed ? Object.freeze({ ...snapshot, commands: Object.freeze(commands) }) : snapshot;
}

