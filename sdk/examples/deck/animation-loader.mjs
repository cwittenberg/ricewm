import { MAX_TIMING_NODES, animationTimelineHeader, checkedAnimationTimeline } from './animation-contract.mjs';

function freezeProjection(value, seen = new Set()) {
    if (!value || typeof value !== 'object' || seen.has(value)) return value;
    seen.add(value);
    for (const child of Object.values(value)) freezeProjection(child, seen);
    return Object.freeze(value);
}

const sameBegins = (a, b) => a.length === b.length && a.every((begin, i) =>
    ['event', 'offsetMs', 'nodeId', 'objectId'].every(key => begin[key] === b[i][key]));
function validateNormalization(node, byId) {
    if (node.beginNormalizedFrom === undefined) return;
    const owner = byId.get(node.beginNormalizedFrom), trigger = owner?.sourceBegin;
    if (!owner || owner.kind !== 'sequence' || owner.role !== 'interactive-sequence' ||
        owner.beginNormalizedFrom !== owner.id || owner.begin.length !== 0 || owner.editable ||
        owner.repeatCount !== 1 || owner.autoReverse || owner.fill === 'transition' ||
        trigger?.length !== 1 || trigger[0].event !== 'target-click' || trigger[0].offsetMs < 0 ||
        !owner.children.length || owner.children.some(id => byId.get(id)?.beginNormalizedFrom !== owner.id)) {
        throw Error('Invalid animation normalization owner');
    }
    if (node === owner) return;
    if (node.kind !== 'parallel' || node.role !== 'container' || node.parentId !== owner.id || node.editable ||
        node.repeatCount !== 1 || node.autoReverse || node.fill === 'transition' ||
        !owner.children.includes(node.id) || !sameBegins(node.begin, trigger) ||
        !(node.sourceBegin.length === 0 || sameBegins(node.sourceBegin, trigger) ||
            sameBegins(node.sourceBegin, [{ event: 'parent-start', offsetMs: 0 }]))) {
        throw Error('Invalid animation normalization condition');
    }
}

/** Read one immutable timing projection. Revision/identity/pagination checks
 * prevent joining old graph fragments after an edit or slide switch. */
export async function loadAnimationTimeline(port, { scene, revision, cancelled = () => false }) {
    const header = structuredClone(scene.animationTimeline), slideId = scene.slideId;
    if (header === undefined) return null;
    if (!animationTimelineHeader(header)) throw Error('Invalid slide animation summary');
    if (typeof port.animationTimeline !== 'function') throw Error('Animation projection is unavailable');
    const alive = () => { if (cancelled()) throw Object.assign(new Error('Animation request cancelled'), { name: 'AbortError' }); };
    const nodes = [], ids = new Set();
    let startNode = 0, limit = 64;
    do {
        alive();
        let reply;
        try { reply = await port.animationTimeline({ slide: slideId, startNode, limit }, revision); }
        catch (error) {
            alive();
            if (error?.code !== 'RESPONSE_TOO_LARGE' || limit === 1) throw error;
            limit = Math.max(1, Math.floor(limit / 2)); continue;
        }
        alive(); checkedAnimationTimeline(reply);
        const page = reply.result, next = startNode + page.nodes.length;
        if (reply.revision !== revision || page.slide !== slideId || page.root !== header.root ||
            page.totalNodes !== header.nodeCount || page.totalNodes > MAX_TIMING_NODES ||
            page.nodes.length > limit || (page.nextNode === null ? next !== page.totalNodes :
                page.nextNode !== next || page.nextNode <= startNode || page.nextNode >= page.totalNodes)) {
            throw Error('Animation revision or pagination changed');
        }
        for (const node of page.nodes) {
            if (ids.has(node.id)) throw Error('Repeated animation identity');
            ids.add(node.id); nodes.push(structuredClone(node));
        }
        startNode = page.nextNode;
    } while (startNode !== null);
    const byId = new Map(nodes.map(node => [node.id, node])), seen = new Set(), preorder = [];
    function visit(id, parent, depth) {
        const node = byId.get(id);
        if (!node || seen.has(id) || node.parentId !== parent || depth > 64) throw Error('Invalid animation tree ownership');
        seen.add(id); preorder.push(id);
        for (const child of node.children) visit(child, id, depth + 1);
    }
    if (header.root !== null) visit(header.root, null, 0);
    if (nodes.some((node, index) => node.id !== preorder[index])) throw Error('Invalid animation source preorder');
    if (seen.size !== nodes.length || nodes.filter(node => node.kind === 'effect').length !== header.effectCount ||
        header.mainSequence && !byId.has(header.mainSequence)) throw Error('Animation summary changed');
    for (const node of nodes) {
        if (node.authoringScope && (node.authoringScope !== header.mainSequence || byId.get(node.authoringScope)?.kind !== 'sequence')) throw Error('Invalid animation authoring scope');
        validateNormalization(node, byId);
        for (const begin of [...node.begin, ...(node.sourceBegin ?? [])]) {
            if (['node-start', 'node-end'].includes(begin.event) && !byId.has(begin.nodeId)) throw Error('Missing animation timing reference');
        }
    }
    return freezeProjection({ ...header, slide: slideId, totalNodes: nodes.length, nodes, nextNode: null });
}
