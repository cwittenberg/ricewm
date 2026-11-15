/** Shared discovery data for authoring and runtime; these describe capabilities,
 * never install a tool or attest that a media format works in an environment. */
export const SDK_BACKGROUND_CONTRACTS = Object.freeze({
    media: Object.freeze({
        source: '{path:appRelativePath}|{handle:fileHandle}|{asset:packagePath}|{url:exactGrantedHttpsUrl}',
        open: '{source,volume?:0..1,muted?:boolean,loop?:boolean} -> {handle,state}; playback is headless and owned by this app launch',
        control: '{handle,command:"play"|"pause"|"stop"|"seek"|"volume"|"mute",value?:number|boolean}; seek uses seconds',
        events: 'desktopEventTopics(appId).mediaChanged; declare that exact bus.subscribe topic and media.watch; close releases the session',
        lifecycle: 'Media stops on worker close/revocation. No Player window, focus change or taskbar entry. context reports filePlayback and streaming separately. A URL requires a qualified streaming provider and exact manifest network origin; do not buffer indefinite streams with network.fetch.',
    }),
    jobs: Object.freeze({
        start: '{tool:registeredToolId,input:boundedJson,destination?:{handle:writableDirectoryHandle}|{path:appRelativeDirectory}} -> job',
        operations: 'context(), start(parameters), list(), status({id}), cancel({id}), remove({id}), watch(), unwatch() via services.jobs',
        events: 'desktopEventTopics(appId).jobsChanged; protected host event containing the current job; declare that exact bus.subscribe topic',
        lifecycle: 'Host-owned jobs survive app window/worker close while RiceWM runs. Approval revocation cancels them. History is checkpointed when durable storage is available; after runtime restart unfinished jobs become interrupted and require a new explicit start. Browser close stops execution.',
        authority: 'Only host-registered tools; tool descriptors report version/input contract. No shell command, executable path, arbitrary environment or process identifier. Tool providers must enforce granted network origins and output-directory confinement, including child processes. Missing tools return TOOL_UNAVAILABLE; never simulate completion.',
    }),
});

export function validateJobPermissions(value = {}) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => key !== 'tools')) throw TypeError('Invalid job permissions');
    const tools = value.tools || [];
    if (!Array.isArray(tools) || tools.length > 16 || tools.some(id => typeof id !== 'string' || !/^[a-z0-9][a-z0-9._-]{0,95}$/.test(id)) || new Set(tools).size !== tools.length) throw TypeError('Job permissions require up to 16 exact tool IDs');
    return Object.freeze({ tools: Object.freeze([...tools]) });
}

export function boundedJobJson(value, maximumBytes = 65536) {
    if (value === undefined) value = null;
    let nodes=0, textBytes=0;
    const inspect = (item, depth = 0) => {
        if(++nodes>10000)throw RangeError('Job JSON has too many values');
        if(typeof item==='string'&&(textBytes+=new TextEncoder().encode(item).length)>maximumBytes)throw RangeError('Job JSON exceeds its byte budget');
        if (depth > 32) throw RangeError('Job JSON is too deeply nested');
        if (item === null || typeof item === 'string' || typeof item === 'boolean' || typeof item === 'number' && Number.isFinite(item)) return;
        if (!item || typeof item !== 'object' || !Array.isArray(item) && Object.getPrototypeOf(item) !== Object.prototype && Object.getPrototypeOf(item) !== null) throw TypeError('Job values must be plain JSON');
        for (const child of Object.values(item)) inspect(child, depth + 1);
    };
    inspect(value);
    const encoded = JSON.stringify(value);
    if (new TextEncoder().encode(encoded).length > maximumBytes) throw RangeError('Job JSON exceeds its byte budget');
    return JSON.parse(encoded);
}
