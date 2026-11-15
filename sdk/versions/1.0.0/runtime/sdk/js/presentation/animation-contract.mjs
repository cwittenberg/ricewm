import { exact, failure } from './contract-values.mjs';
import { checkedResponse, revision } from './status-contract.mjs';

export const MAX_TIMING_NODES = 2048;
const identifier = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const count = (value, maximum = MAX_TIMING_NODES) => Number.isSafeInteger(value) && value >= 0 && value <= maximum;
const codes = value => Array.isArray(value) && value.length <= 256 && value.every(code => typeof code === 'string' && code.length <= 256);
const invalid = message => { throw failure('INVALID_RESPONSE', message); };

export function animationTimelineHeader(value) {
    return value === undefined || value !== null && value.version === 1 &&
        (value.root === null || identifier(value.root)) && count(value.nodeCount) && count(value.effectCount) &&
        value.effectCount <= value.nodeCount && (value.nodeCount === 0) === (value.root === null) &&
        (value.mainSequence === undefined || value.mainSequence === null || identifier(value.mainSequence)) &&
        typeof value.playable === 'boolean' && typeof value.editable === 'boolean' && codes(value.unsupported);
}

export function animationArguments(args) {
    if (!exact(args, ['slide', 'startNode', 'limit'], ['slide']) || !identifier(args.slide) ||
        args.startNode !== undefined && !count(args.startNode) || args.limit !== undefined && (!count(args.limit, 64) || args.limit === 0)) {
        throw failure('INVALID_REQUEST', 'Invalid bounded animation query');
    }
    return { slide: args.slide, startNode: args.startNode ?? 0, limit: args.limit ?? 32 };
}

function animationBegins(conditions) {
    return Array.isArray(conditions) && conditions.length <= 64 && conditions.every(begin => {
        if (!begin || !['parent-start', 'node-start', 'node-end', 'click', 'target-click'].includes(begin.event)) return false;
        const fields = ['event', 'offsetMs', ...(['node-start', 'node-end'].includes(begin.event) ? ['nodeId'] :
            begin.event === 'target-click' ? ['objectId'] : [])];
        return exact(begin, fields) && Number.isSafeInteger(begin.offsetMs) && Math.abs(begin.offsetMs) <= 600000 &&
            (!fields.includes('nodeId') || identifier(begin.nodeId)) && (!fields.includes('objectId') || identifier(begin.objectId));
    });
}

export function animationNode(node) {
    if (!node || !identifier(node.id) || !(node.parentId === null || identifier(node.parentId)) ||
        !['parallel', 'sequence', 'effect', 'unsupported'].includes(node.kind) ||
        !Array.isArray(node.children) || node.children.length > MAX_TIMING_NODES || !node.children.every(identifier) ||
        new Set(node.children).size !== node.children.length ||
        !animationBegins(node.begin) ||
        Object.hasOwn(node, 'sourceBegin') !== Object.hasOwn(node, 'beginNormalizedFrom') ||
        Object.hasOwn(node, 'sourceBegin') && (!animationBegins(node.sourceBegin) || !identifier(node.beginNormalizedFrom)) ||
        !node.duration || !['ms', 'auto', 'indefinite'].includes(node.duration.kind) ||
        node.endPolicy !== undefined && !['children', 'duration'].includes(node.endPolicy) ||
        node.role !== undefined && !['timing-root', 'main-sequence', 'interactive-sequence', 'container', 'effect'].includes(node.role) ||
        node.authoringScope !== undefined && node.authoringScope !== null && !identifier(node.authoringScope) ||
        node.duration.kind === 'ms' && !count(node.duration.value, 600000) ||
        !(node.repeatCount === null || Number.isFinite(node.repeatCount) && node.repeatCount > 0 && node.repeatCount <= 1000) ||
        typeof node.autoReverse !== 'boolean' || !['remove', 'freeze', 'hold', 'transition'].includes(node.fill) ||
        typeof node.editable !== 'boolean' || !codes(node.unsupported)) return false;
    if (node.kind !== 'effect') return true;
    return node.children.length === 0 && identifier(node.target?.object) && identifier(node.target.subtarget?.kind) &&
        (node.target.initialVisibility === undefined || typeof node.target.initialVisibility === 'boolean') &&
        (node.visibility === undefined || node.visibility !== null && typeof node.visibility.value === 'boolean' && count(node.visibility.offsetMs, 600000)) &&
        node.target.subtarget.kind.length <= 64 && node.effect && identifier(node.effect.kind) && node.effect.kind.length <= 64 &&
        ['entrance', 'emphasis', 'exit', 'motion'].includes(node.effect.category) &&
        ['click', 'with-previous', 'after-previous'].includes(node.start) &&
        count(node.durationMs, 600000) && count(node.delayMs, 600000);
}

export function checkedAnimationTimeline(response, args, expected) {
    checkedResponse(response); revision(response.revision);
    const page = response.result;
    if (!page || !identifier(page.slide) || !(page.root === null || identifier(page.root)) ||
        !count(page.totalNodes) || (page.totalNodes === 0) !== (page.root === null) ||
        !Array.isArray(page.nodes) || page.nodes.length > 64 || !page.nodes.every(animationNode) ||
        !(page.nextNode === null || count(page.nextNode) && page.nextNode > 0 && page.nextNode < page.totalNodes) ||
        expected !== undefined && response.revision !== expected || args && (page.slide !== args.slide || page.nodes.length > args.limit ||
            (page.nextNode === null ? args.startNode + page.nodes.length !== page.totalNodes :
                page.nextNode !== args.startNode + page.nodes.length || page.nextNode <= args.startNode))) {
        invalid('Invalid animation timing page');
    }
    return response;
}
