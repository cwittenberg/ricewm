import { BACKGROUND_IMAGE_KEY } from './background-contract.mjs';

/** Resource planning only: backgrounds never enter the editable object list. */
export function backgroundImageTarget(scene) {
    const picture = scene.backgroundPicture;
    if (!picture || !scene.size) return null;
    const { image } = picture;
    return { id: BACKGROUND_IMAGE_KEY, image, transform: [1, 0, 0, 1, 0, 0], bounds: {
        x: 0, y: 0,
        width: picture.mode === 'tile' ? image.width * picture.scaleX : scene.size.width,
        height: picture.mode === 'tile' ? image.height * picture.scaleY : scene.size.height,
    } };
}

/** Visible tiles only, with a source-validated command budget. */
export function backgroundRectangles(scene) {
    const target = backgroundImageTarget(scene);
    if (!target) return [];
    const { width, height } = scene.size, picture = scene.backgroundPicture;
    if (picture.mode === 'stretch') return [[0, 0, width, height]];
    const w = target.bounds.width, h = target.bounds.height;
    const start = (offset, span) => { const value = ((offset % span) + span) % span; return value === 0 ? 0 : value - span; };
    const x = start(picture.offsetX, w), y = start(picture.offsetY, h);
    const columns = Math.ceil((width - x) / w), rows = Math.ceil((height - y) / h);
    if (columns * rows > 4096) throw new RangeError('Background texture needs a larger tile scale');
    return Array.from({ length: columns * rows }, (_, i) => [x + i % columns * w, y + Math.floor(i / columns) * h, w, h]);
}

export function paintBackgroundPicture(builder, scene, frames, textures) {
    const picture = scene.backgroundPicture, resource = frames.get(BACKGROUND_IMAGE_KEY);
    if (!picture || !resource || picture.opacity === 0) return;
    const { left, top, right, bottom } = picture.image.crop;
    builder.pushClip([0, 0, scene.size.width, scene.size.height]);
    for (const rect of backgroundRectangles(scene)) {
        builder.image(textures.get(resource.key), rect, { uv: [left, top, 1 - right, 1 - bottom], opacity: picture.opacity });
    }
    builder.popClip();
}

export function paintBackgroundGradient(builder, scene, packed) {
    const gradient = scene.backgroundGradient;
    if (!gradient) return;
    const stops = gradient.stops.map(stop => ({ offset: stop.offset, color: packed(stop) }));
    const { width, height } = scene.size;
    if (gradient.kind === 'linear') {
        builder.linearGradient([0, 0, width, height], gradient.start, gradient.end, stops);
    } else {
        const [x, y] = gradient.center, [rx, ry] = gradient.radius;
        builder.pushTransform([rx, 0, 0, ry, x, y]);
        builder.radialGradient([-x / rx, -y / ry, width / rx, height / ry], [0, 0, 0], [0, 0, 1], stops);
        builder.popTransform();
    }
}
