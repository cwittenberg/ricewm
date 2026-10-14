import {resolveRiceSdkVersion, riceSdkServiceMethod} from './rice-sdk-versions.mjs';

export function validateRiceAppManifest(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw TypeError('manifest must be an object');
    if (value.schema !== 'ricewm.app/v1') throw TypeError('unsupported manifest schema');
    if (!/^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)+$/.test(value.id || '')) throw TypeError('invalid app id');
    if (typeof value.name !== 'string' || !value.name.trim()) throw TypeError('app name is required');
    if (typeof value.version !== 'string' || !/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(value.version)) throw TypeError('invalid app version');
    if (typeof value.entry !== 'string' || value.entry.startsWith('/') || value.entry.includes('\\') || value.entry.split('/').some(p => !p || p === '.' || p === '..')) throw TypeError('invalid app entry');
    const version = resolveRiceSdkVersion(value.sdk);
    const services = value.permissions?.services ?? [];
    if (!Array.isArray(services)) throw TypeError('services must be an array');
    for (const permission of services) {
        if (typeof permission !== 'string' || !riceSdkServiceMethod(permission, {version})) throw TypeError('unknown service permission');
    }
    return {...value, sdk: {version}};
}

export function pinRiceAppSourceFiles(entries) {
    const files = new Map(entries);
    const source = files.get('manifest.json');
    if (!source) throw TypeError('manifest.json is required');
    const manifest = validateRiceAppManifest(JSON.parse(new TextDecoder().decode(source)));
    files.set('manifest.json', new TextEncoder().encode(JSON.stringify(manifest, null, 2) + '\n'));
    return files;
}
