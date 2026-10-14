/** Replay target input against charged subscriptions, independent of rendering.
 * Registration precedes dispatch; one physical event consumes one subscription.
 * A continuation is usable only after the caller verifies the history prefix. */
export function dispatchTargetEvents(history, subscriptions, continuation = null) {
    const claims = new Map(continuation?.claims || []);
    const accepted = [...(continuation?.accepted || [])];
    for (let index = continuation?.offset || 0; index < history.length; index++) {
        const event = history[index];
        if (event.target === null) continue;
        const charged = subscriptions(history.slice(0, index), claims, event)
            .filter(item => item.objectId === event.target)
            .sort((a, b) => a.registeredAt - b.registeredAt || a.registeredEvent - b.registeredEvent ||
                a.sourceOrder - b.sourceOrder || a.scheduledAt - b.scheduledAt || a.conditionIndex - b.conditionIndex);
        const first = charged[0];
        if (!first) continue;
        claims.set(first.nodeId, { event, conditionIndex: first.conditionIndex });
        accepted.push({ index: event.index, objectId: event.target, nodeId: first.nodeId });
    }
    return { claims, accepted };
}
