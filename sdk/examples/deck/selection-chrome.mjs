import {rectangleHandles} from '@ricewm/app';
import {objectCorners} from './canvas-geometry.mjs';

export function objectResizeHandles(object) {
    const points=objectCorners(object);
    return points.length===4?rectangleHandles(points).map(item=>item.point):points;
}

// The pinned reference separates neutral object selection from the active
// cell accent. These are application paint tokens, not document properties.
export function deckSelectionColors(theme=null) {
    const panel=theme?.panel||theme?.background;
    const dark=typeof panel==='string'&&/^#[0-9a-f]{6}$/i.test(panel)&&
        .2126*parseInt(panel.slice(1,3),16)+.7152*parseInt(panel.slice(3,5),16)+.0722*parseInt(panel.slice(5,7),16)<128;
    return {selection:dark?0xffaaaaaa:0xff7a7a7a,handleFill:0xffffffff,handleStroke:0xff6e6e6e,
        accent:dark?0xff1d6bf2:0xff1a5ad9};
}
