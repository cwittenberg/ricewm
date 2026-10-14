/** Canonical package-relative names, identical on case-sensitive and other hosts. */
export function spatialAssetPath(value) {
    if(typeof value!=='string'||value.length>240||!value.split('/').every(part=>/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,79}$/.test(part))||value.split('/').some(part=>part==='.'||part==='..'))throw TypeError('Invalid spatial asset path');
    return value;
}
export function resolveSpatialAsset(manifest,relative) {
    spatialAssetPath(manifest);spatialAssetPath(relative);
    return spatialAssetPath([...manifest.split('/').slice(0,-1),relative].join('/'));
}
