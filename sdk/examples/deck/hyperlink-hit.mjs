import { audienceScenePoint, hitAudienceTrigger } from './audience-trigger-hit.mjs';
import { inverseAffine, transformPoint } from './affine.mjs';
import { supportedLink } from './hyperlink-contract.mjs';
const bytes = value => new TextEncoder().encode(value).length;
/** Hit only the linked run's shaped clusters; an unrelated covering shape blocks it. */
export function hyperlinkAt({ scene, frame, point, textFrames = new Map(), visibility = new Map() }) {
    if (!scene?.objects) return null;
    const id = hitAudienceTrigger({ scene, frame, point, visibility, targets: new Set(scene.objects.map(o => o.id)) });
    let object = scene.objects.find(o => o.id === id);
    if (!object) return null;
    const position = audienceScenePoint(scene, frame, point), inverse = object.transform && inverseAffine(object.transform);
    if (position && inverse && object.text) {
        const [x,y] = transformPoint(inverse, [position.x,position.y]);
        let cursor = 0;
        for (const run of object.text.runs) {
            const start = object.text.text.indexOf(run.text, cursor);
            if (start < 0) continue;
            cursor = start + run.text.length;
            const link = object.hyperlinks?.find(link => link.run === run.id && supportedLink(link));
            if (!link) continue;
            const from = bytes(object.text.text.slice(0,start)), to = from + bytes(run.text);
            if (textFrames.get(object.id)?.glyphs?.some(g => g.cluster[0] < to && g.cluster[1] > from &&
                x >= Math.min(g.x, g.x + g.advance) && x <= Math.max(g.x, g.x + g.advance) &&
                y >= g.y - g.fontSize && y <= g.y + g.fontSize * .25)) return { ...link, object: object.id };
        }
    }
    // A group-level link applies to its visible children too.
    const seen = new Set();
    while (object && !seen.has(object.id)) {
        seen.add(object.id);
        const link = object.hyperlinks?.find(link => link.run === null && supportedLink(link));
        if (link) return { ...link, object: object.id };
        object = scene.objects.find(o => o.id === object.parentId);
    }
    return null;
}
