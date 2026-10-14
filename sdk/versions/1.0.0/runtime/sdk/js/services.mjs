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
        compute: group(bridge, grants, "compute", ["open", "invoke", "cancel", "close"], true),
        docker: group(bridge, grants, "docker", ["context", "install", "pull", "run", "start", "inspect", "logs", "stop"], true),
        clipboard: group(bridge, grants, "clipboard", ["readText", "writeText"], true),
        notifications: group(bridge, grants, 'notifications', ['create', 'update', 'delete', 'show', 'list', 'dismiss'], true),
        calendar: group(bridge, grants, 'calendar', ['sources', 'list', 'create', 'update', 'delete', 'watch'], true),
        wallpapers: group(bridge, grants, 'wallpapers', ['context', 'list', 'search', 'import', 'generate', 'set', 'assign', 'watch'], true),
        hotkeys: group(bridge, grants, 'hotkeys', ['register', 'unregister', 'list'], true),
        actions: group(bridge, grants, 'actions', ['register', 'unregister', 'list'], true),
        plugins: group(bridge, grants, 'plugins', ['register', 'unregister', 'list', 'context', 'apply'], true),
        files: group(bridge, grants, "files", [
            "read", "write", "stat", "list", "mkdir", "remove", "rename",
            "watch", "openDialog", "saveDialog", "chooseFolder",
        ], true),
        settings: group(bridge, grants, "settings", ["get", "set", "list"], true),
        themes: group(bridge, grants, "themes", ["context", "read", "save", "activate", "visual", "menu"], true),
        recipes: group(bridge, grants, "recipes", ["presentation", "list", "preview", "save", "apply", "status", "cancel"]),
        media: group(bridge, grants, "media", ["open", "metadata", "export"]),
        network: group(bridge, grants, "network", ["fetch"], true),
        applications: group(bridge, grants, "applications", [
            "list", "launch", "openWith", "editSource", "develop",
            "openDevelopmentWorkspace", "stopDevelopment", "installPackage",
        ]),
        extensions: group(bridge, grants, "extensions", [
            "list", "enable", "disable", "restart", "openManager", "call",
        ]),
        system: group(bridge, grants, "system", ["info", "performance", "processSignal", "processNice"], operation => operation === 'info'),
        terminal: group(bridge, grants, "terminal", ["spawn", "write", "resize", "close"]),
        host: group(bridge, grants, "host", ["notify", "openExternal", "print", "revealPath"], operation => operation === 'notify' || operation === 'print'),
        dialogs: group(bridge, grants, "dialogs", [
            "message", "confirm", "openFile", "saveFile", "chooseFolder",
            "chooseColor", "chooseFont", "print", "pageSetup", "find", "replace",
        ], operation => operation === 'message' || operation === 'confirm' || operation === 'openFile' || operation === 'saveFile' || operation === 'chooseFolder' || operation === 'chooseColor' || operation === 'chooseFont' || operation === 'find' || operation === 'replace'),
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
