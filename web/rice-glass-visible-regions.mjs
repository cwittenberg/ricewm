// Optical pixels covered by solid content or cleared for a DOM portal are
// discarded later in the same paint. Keep the original lens geometry, but shade/copy only its output.
export function glassVisibleRegions(width, height, covers) {
    let regions = [{x:0, y:0, width, height}];
    for (const cover of covers) {
        if (![cover.x, cover.y, cover.width, cover.height].every(Number.isFinite)) continue;
        const next = [];
        for (const rect of regions) {
            const x = Math.max(rect.x, Math.ceil(cover.x));
            const y = Math.max(rect.y, Math.ceil(cover.y));
            const right = Math.min(rect.x + rect.width, Math.floor(cover.x + cover.width));
            const bottom = Math.min(rect.y + rect.height, Math.floor(cover.y + cover.height));
            if (right <= x || bottom <= y) { next.push(rect); continue; }
            if (y > rect.y) next.push({x:rect.x, y:rect.y, width:rect.width, height:y-rect.y});
            if (bottom < rect.y + rect.height) next.push({x:rect.x, y:bottom, width:rect.width, height:rect.y+rect.height-bottom});
            if (x > rect.x) next.push({x:rect.x, y, width:x-rect.x, height:bottom-y});
            if (right < rect.x + rect.width) next.push({x:right, y, width:rect.x+rect.width-right, height:bottom-y});
        }
        if (next.length > 64) return null;
        regions = next;
    }
    return regions;
}

export function paintGlassImage(context, source, rect, regions, sourceY = 0) {
    if (!regions) {
        context.drawImage(source, 0, sourceY, rect.width, rect.height,
            rect.x, rect.y, rect.width, rect.height);
        return;
    }
    for (const region of regions) context.drawImage(source,
        region.x, sourceY + region.y, region.width, region.height,
        rect.x + region.x, rect.y + region.y, region.width, region.height);
}
