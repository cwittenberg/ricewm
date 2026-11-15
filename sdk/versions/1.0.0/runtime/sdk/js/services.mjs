function grantedSet(permissions) {
    const values = Array.isArray(permissions) ? permissions : permissions?.services || [];
    return new Set([...values.map(String), "debug.log"]);
}

function capabilityMethod(bridge, grants, service, operation, generated) {
    const permission = `${service}.${operation}`;
    const method = async parameters => {
        if (!grants.has(permission)) throw new Error(`app was not granted ${permission}`);
        if (!bridge || typeof bridge.request !== "function") throw new Error("RiceWM service bridge is unavailable");
        return bridge.request(service, operation, parameters ?? {});
    };
    Object.defineProperties(method, {
        permission: { value: permission },
        generatedAllowed: { value: generated },
        granted: { value: grants.has(permission) },
    });
    return method;
}

function group(bridge, grants, service, operations, generated = false) {
    return Object.freeze(Object.fromEntries(operations.map(operation => [
        operation,
        capabilityMethod(bridge, grants, service, operation, typeof generated === "function" ? generated(operation) : generated),
    ])));
}

/** Capability-checked access to host-owned RiceWM services. */
export function createServices(bridge, permissions = []) {
    const grants = grantedSet(permissions);
    return Object.freeze({
        assets: group(bridge, grants, 'assets', ['read', 'stat'], true),
        resources: group(bridge, grants, 'resources', ['read', 'stat'], true),
        graphics: group(bridge, grants, 'graphics', ['context','open','upload','render','close'], true),
        editors: group(bridge, grants, 'editors', ['open', 'invoke', 'events', 'respond', 'close'], true),
        compute: group(bridge, grants, "compute", ["open", "invoke", "cancel", "close"], true),
        docker: group(bridge, grants, "docker", ["context", "install", "pull", "run", "start", "inspect", "logs", "stop"], true),
        clipboard: group(bridge, grants, "clipboard", ["readText", "writeText", "readFormats", "writeFormats"], true),
        documents: group(bridge, grants, "documents", ["extractText"], true),
        notifications: group(bridge, grants, 'notifications', ['create', 'update', 'delete', 'show', 'list', 'dismiss'], true),
        calendar: group(bridge, grants, 'calendar', ['sources', 'list', 'create', 'update', 'delete', 'watch'], true),
        accounts: group(bridge, grants, 'accounts', ['list'], true),
        contacts: group(bridge, grants, 'contacts', ['list', 'upsert', 'delete'], true),
        mail: group(bridge, grants, 'mail', ['list', 'search', 'read', 'attachment', 'labels', 'send'], true),
        wallpapers: group(bridge, grants, 'wallpapers', ['context', 'list', 'search', 'import', 'generate', 'set', 'assign', 'watch'], true),
        hotkeys: group(bridge, grants, 'hotkeys', ['register', 'unregister', 'list'], true),
        actions: group(bridge, grants, 'actions', ['register', 'unregister', 'list'], true),
        capabilities: group(bridge, grants, 'capabilities', ['register', 'unregister', 'respond'], true),
        windows: group(bridge, grants, 'windows', ['register', 'unregister', 'focus', 'state'], true),
        drops: group(bridge, grants, 'drops', ['register', 'unregister', 'respond'], true),
        plugins: group(bridge, grants, 'plugins', ['register', 'unregister', 'list', 'context', 'apply'], true),
        files: group(bridge, grants, "files", [
            "read", "write", "stat", "list", "mkdir", "remove", "rename",
            "watch", "unwatch", "child", "release", "requestWrite", "version", "releaseVersion", "beginWrite", "writeChunk", "commitWrite", "abortWrite", "openDialog", "saveDialog", "chooseFolder",
        ], true),
        settings: group(bridge, grants, "settings", ["get", "set", "list"], true),
        themes: group(bridge, grants, "themes", ["context", "read", "save", "activate", "visual", "menu", "applications", "applicationVisual"], true),
        recipes: group(bridge, grants, "recipes", ["presentation", "list", "preview", "save", "apply", "status", "cancel"]),
        media: group(bridge, grants, "media", ["open", "metadata", "export", "context", "control", "state", "close", "watch", "unwatch", "playSound"], operation => !["metadata", "export"].includes(operation)),
        jobs: group(bridge, grants, "jobs", ["context", "start", "list", "status", "cancel", "remove", "watch", "unwatch"], true),
        display: group(bridge, grants, 'display', ['context', 'get', 'set', 'reset'], true),
        network: group(bridge, grants, "network", ["fetch"], true),
        applications: group(bridge, grants, "applications", [
            "list", "launch", "assist", "formatSource", "openWith", "handlers", "open", "status", "editSource", "develop",
            "openDevelopmentWorkspace", "stopDevelopment", "installPackage",
        ], operation => ['handlers', 'open', 'status'].includes(operation)),
        extensions: group(bridge, grants, "extensions", [
            "list", "enable", "disable", "restart", "openManager", "call", "develop", "logs",
        ]),
        system: group(bridge, grants, "system", ["info", "capabilities", "performance", "processSignal", "processNice"], operation => operation === 'info' || operation === 'capabilities'),
        terminal: group(bridge, grants, "terminal", ["spawn", "read", "write", "resize", "scroll", "close"]),
        host: group(bridge, grants, "host", ["notify", "openExternal", "print", "revealPath"], operation => operation === 'notify' || operation === 'print'),
        dialogs: group(bridge, grants, "dialogs", [
            "message", "confirm", "openFile", "saveFile", "chooseFolder",
            "chooseColor", "chooseFont", "print", "pageSetup", "find", "replace",
        ], true),
        debug: group(bridge, grants, "debug", ["log"], true),
    });
}

/** Win32-common-dialog equivalents, rendered and owned by RiceWM. */
export class SystemDialogs {
    constructor(services) { this.services = services; }
    message(options) { return this.services.dialogs.message(options); }
    confirm(options) { return this.services.dialogs.confirm(options); }
    warning(text, title = "Warning") { return this.message({ kind: "warning", title, text }); }
    error(text, title = "Error") { return this.message({ kind: "error", title, text }); }
    openFile(options = {}) { return this.services.dialogs.openFile(options); }
    saveFile(options = {}) { return this.services.dialogs.saveFile(options); }
    chooseFolder(options = {}) { return this.services.dialogs.chooseFolder(options); }
    chooseColor(options = {}) { return this.services.dialogs.chooseColor(options); }
    chooseFont(options = {}) { return this.services.dialogs.chooseFont(options); }
    print(options = {}) { return this.services.dialogs.print(options); }
    pageSetup(options = {}) { return this.services.dialogs.pageSetup(options); }
    find(options = {}) { return this.services.dialogs.find(options); }
    replace(options = {}) { return this.services.dialogs.replace(options); }
}

/** Inspect the actual SDK API without invoking a service or host bridge.
 * Manifest parsing, generated-app policy and host dispatch use these same
 * methods; adding a method cannot leave a second name list behind. */
export const SDK_SERVICE_METHODS = Object.freeze(Object.entries(createServices(null)).flatMap(([service, methods]) =>
    Object.entries(methods).map(([operation, method]) => Object.freeze({
        service, operation, permission: method.permission, generatedAllowed: method.generatedAllowed,
    })),
));
const sdkMethods = new Map(SDK_SERVICE_METHODS.map(method => [method.permission, method]));
export function sdkServiceMethod(permission) { return sdkMethods.get(permission) || null; }
