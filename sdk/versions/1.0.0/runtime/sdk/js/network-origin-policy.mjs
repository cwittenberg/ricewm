/** Public, exact-origin contracts. Permission receipts remain host-owned. */
export const NETWORK_ORIGIN_LIMITS = Object.freeze({review:128, grants:2048, active:4, pending:128});

export function checkedPublicHttpsUrl(value) {
    if (typeof value !== 'string' || value.length > 8192 || /[\u0000-\u0020\u007f]/u.test(value)) throw TypeError('Enter a public HTTPS address');
    const url = new URL(value), host = url.hostname.toLowerCase();
    if (url.protocol !== 'https:' || url.username || url.password || url.port ||
        !host.includes('.') || host.endsWith('.') || host.includes(':') || /^\d+(?:\.\d+)*$/u.test(host) ||
        ['localhost','local','internal','home','lan','onion'].some(suffix => host === suffix || host.endsWith('.'+suffix))) {
        throw TypeError('Only public HTTPS addresses without credentials or special ports are supported');
    }
    return url;
}

export function checkedOriginReview(parameters) {
    if (!parameters || Object.keys(parameters).some(key => key !== 'origins') ||
        !Array.isArray(parameters.origins) || !parameters.origins.length || parameters.origins.length > NETWORK_ORIGIN_LIMITS.review) {
        throw TypeError('Review between 1 and 128 exact HTTPS origins');
    }
    return Object.freeze([...new Set(parameters.origins.map(origin => {
        const url = checkedPublicHttpsUrl(origin);
        if (url.origin !== origin) throw TypeError('Network permission requires an exact HTTPS origin');
        return origin;
    }))]);
}

export function checkedNetworkRequestId(value) {
    if (typeof value !== 'string' || !/^[A-Za-z0-9_-]{1,96}$/u.test(value)) throw TypeError('Invalid network request identity');
    return value;
}
