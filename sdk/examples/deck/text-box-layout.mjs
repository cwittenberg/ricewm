// Source-resolved text geometry shared by editor, thumbnails and presentations.
const bounded = value => Number.isFinite(value) && Math.abs(value) <= 1_000_000;

export function textBox(object, notices) {
    const bounds = object.bounds, layout = object.text.layout;
    if (!bounds || !bounded(bounds.width) || !bounded(bounds.height) || bounds.width <= 0 || bounds.height < 0) {
        throw new TypeError('Text geometry unavailable');
    }
    const source = layout ?? { left: 0, top: 0, right: 0, bottom: 0, vertical: 'top', wrap: true };
    if (!['left', 'top', 'right', 'bottom'].every(key => bounded(source[key])) ||
        !['top', 'middle', 'bottom'].includes(source.vertical) || typeof source.wrap !== 'boolean') {
        throw new TypeError('Unsupported bounded text box layout');
    }
    if (!layout) notices.push('Some text-box margins or anchoring are unresolved and use preview placement.');
    const width = bounds.width - source.left - source.right;
    const height = bounds.height - source.top - source.bottom;
    if (!bounded(width) || width <= 0 || !bounded(height) || height < 0) {
        throw new TypeError('Text-box margins leave no supported text area');
    }
    const x = (bounds.x ?? 0) + source.left, y = (bounds.y ?? 0) + source.top;
    if (!bounded(x) || !bounded(y)) throw new TypeError('Unsupported text-box origin');
    return { x, y, width, height, vertical: source.vertical, wrap: source.wrap };
}

export function horizontalOffset(box, frame, alignment) {
    if (box.wrap) return 0;
    const remaining = box.width - frame.width;
    if (alignment === 'center') return remaining / 2;
    if (alignment === 'right') return remaining;
    if (alignment !== 'left') throw new TypeError('Unresolved unwrapped paragraph direction');
    return 0;
}

export function verticalOffset(box, frame) {
    const remaining = box.height - frame.height;
    return box.y + (box.vertical === 'middle' ? remaining / 2 : box.vertical === 'bottom' ? remaining : 0);
}
