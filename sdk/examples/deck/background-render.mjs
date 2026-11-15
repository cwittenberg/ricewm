import { BACKGROUND_IMAGE_KEY } from './background-contract.mjs';
import { paintBackgroundFill,pictureFillTiles,paintPictureFill } from '@ricewm/app';

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
    return pictureFillTiles(scene.size,scene.backgroundPicture.image,scene.backgroundPicture).map(tile=>tile.rect);
}

export function paintBackgroundPicture(builder, scene, frames, textures) {
    const picture = scene.backgroundPicture, resource = frames.get(BACKGROUND_IMAGE_KEY);
    if (!picture || !resource || picture.opacity === 0) return;
    const { left, top, right, bottom } = picture.image.crop;
    paintPictureFill(builder,textures.get(resource.key),scene.size,picture.image,picture,{uv:[left,top,1-right,1-bottom]});
}

export function paintBackgroundGradient(builder, scene, packed) {
    if(['gradient','pattern'].includes(scene.backgroundFill?.kind)) {
        paintBackgroundFill(builder,scene.backgroundFill,[0,0,scene.size.width,scene.size.height]);return;
    }
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
