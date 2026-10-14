/** One revision-pinned, bounded scene reader for editing and presenting. */
export async function loadDeckScene(port, { index, revision, maximumObjects = 4096, cancelled = () => false }) {
    let scene = null, startObject = 0, limit = 64;
    const ids = new Set();
    const alive = () => { if (cancelled()) throw Object.assign(new Error('Slide request cancelled'), { name: 'AbortError' }); };
    do {
        alive();
        let reply;
        try { reply = await port.scene({ slide: index, startObject, limit }, revision); }
        catch (error) {
            alive();
            if (error?.code !== 'RESPONSE_TOO_LARGE' || limit === 1) throw error;
            limit = Math.max(1, Math.floor(limit / 2));
            continue;
        }
        alive();
        const page = reply?.result?.scene;
        if (reply.revision !== revision || page?.index !== index || !Number.isSafeInteger(page.totalObjects) ||
            page.totalObjects < 0 || page.totalObjects > maximumObjects || !Array.isArray(page.objects) || page.objects.length > 64) {
            throw Error('Slide geometry revision or object budget changed');
        }
        if (!scene) scene = { ...page, objects: [] };
        if (page.slideId !== scene.slideId || page.totalObjects !== scene.totalObjects ||
            page.size?.width !== scene.size?.width || page.size?.height !== scene.size?.height ||
            page.background?.color !== scene.background?.color || page.background?.opacity !== scene.background?.opacity ||
            JSON.stringify(page.backgroundPicture) !== JSON.stringify(scene.backgroundPicture) ||
            JSON.stringify(page.backgroundGradient) !== JSON.stringify(scene.backgroundGradient) ||
            JSON.stringify(page.transition) !== JSON.stringify(scene.transition) ||
            page.canSetTransition !== scene.canSetTransition || page.canDisableTransitionClick !== scene.canDisableTransitionClick ||
            page.hasAnimationTiming !== scene.hasAnimationTiming ||
            JSON.stringify(page.animationTimeline) !== JSON.stringify(scene.animationTimeline) ||
            page.canMorphTransition !== scene.canMorphTransition ||
            page.canSetSize !== scene.canSetSize || page.canSetBackground !== scene.canSetBackground ||
            page.layoutId !== scene.layoutId ||
            (page.nextObject === null ? startObject + page.objects.length !== page.totalObjects
                : page.nextObject !== startObject + page.objects.length || page.nextObject <= startObject || page.nextObject >= page.totalObjects)) {
            throw Error('Slide geometry pagination changed');
        }
        for (const object of page.objects) {
            if (!object.id || ids.has(object.id)) throw Error('Repeated slide object identity');
            ids.add(object.id); scene.objects.push(object);
        }
        startObject = page.nextObject;
    } while (startObject !== null);
    return scene;
}

export async function renderDeckScene(port, scene, { cancelled = () => false } = {}) {
    const alive = () => { if (cancelled()) throw Object.assign(new Error('Slide request cancelled'), { name: 'AbortError' }); };
    alive();
    const text = port.renderText ? await port.renderText(scene) : { frames: new Map(), notices: [] };
    alive();
    const pictures = port.renderImages ? await port.renderImages(scene) : { frames: new Map(), notices: [] };
    alive();
    const unsupported = (scene.unsupported?.length || scene.objects.some(object => object.unsupported?.length))
        ? ['Some slide features are not supported yet; check the slide before presenting.'] : [];
    return { scene, textFrames: text.frames, imageFrames: pictures.frames, notices: [...pictures.notices, ...text.notices, ...unsupported] };
}
