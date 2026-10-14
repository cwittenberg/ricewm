import {spatialAssetPath} from './paths.mjs';
export function validateSpatialTheme(value) {
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['engine','api','world','package','builtin'].includes(k))||value.engine!=='rice-spatial'||value.api!==1)throw TypeError('Unsupported spatial theme');
    const result={engine:'rice-spatial',api:1,world:spatialAssetPath(value.world)};
    if(value.package!==undefined){
        const p=value.package;
        if(!p||typeof p!=='object'||Array.isArray(p)||Object.keys(p).some(k=>!['id','hash'].includes(k))||!/^[a-z][a-z0-9.-]{0,63}$/.test(p.id)||!/^[a-f0-9]{64}$/.test(p.hash))throw TypeError('Invalid installed spatial package identity');
        result.package={id:p.id,hash:p.hash};
    }
    if(value.builtin!==undefined){
        const p=value.builtin;
        if(value.package||!p||typeof p!=='object'||Array.isArray(p)||Object.keys(p).some(k=>!['asset','id','hash'].includes(k))||!/^assets\/worlds\/[a-z0-9][a-z0-9.-]{0,63}\.riceapp$/.test(p.asset)||!/^[a-z][a-z0-9.-]{0,63}$/.test(p.id)||!/^[a-f0-9]{64}$/.test(p.hash))throw TypeError('Invalid bundled spatial package identity');
        result.builtin={asset:p.asset,id:p.id,hash:p.hash};
    }
    return result;
}
