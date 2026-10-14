export const failure = (code, message) => Object.assign(new Error(message), { code });
export const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
export function exact(value, allowed, required = allowed) {
    return object(value) && Object.keys(value).every(key => allowed.includes(key)) && required.every(key => Object.hasOwn(value, key));
}
