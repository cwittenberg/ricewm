/** Source text clipping in object-local coordinates; the native surface still clips the slide. */
export function textClipRect(scene, object) {
    const { x, y, width, height } = object.bounds;
    const layout = object.text?.layout;
    // Older engines and unresolved ODF layouts retain their conservative behavior.
    const clipX = layout?.horizontalOverflow !== 'overflow';
    const clipY = layout?.verticalOverflow !== 'overflow';
    if (!clipX && !clipY) return null;
    const rect = [x, y, width, height];
    if (clipX && clipY) return rect;
    const [a, b, c, d, e, f] = object.transform;
    const determinant = a * d - b * c;
    if (!Number.isFinite(determinant) || Math.abs(determinant) < 1e-12) return rect;
    // An inverse-transformed slide bounds the open axis even for rotated/grouped shapes.
    const corners = [[0, 0], [scene.size.width, 0], [0, scene.size.height], [scene.size.width, scene.size.height]]
        .map(([px, py]) => [(d * (px - e) - c * (py - f)) / determinant,
            (-b * (px - e) + a * (py - f)) / determinant]);
    const left = clipX ? x : Math.min(...corners.map(point => point[0]));
    const top = clipY ? y : Math.min(...corners.map(point => point[1]));
    const right = clipX ? x + width : Math.max(...corners.map(point => point[0]));
    const bottom = clipY ? y + height : Math.max(...corners.map(point => point[1]));
    return [left, top, right - left, bottom - top];
}
