import * as sdk100 from './versions/1.0.0/runtime/sdk/js/index.mjs';


export const CURRENT_SDK_VERSION='1.0.0';
export function createRiceSdkVersionRegistry(entries){
    const versions=new Set();
    for(const [version,namespace] of entries){
        if(versions.has(version))throw Error('Duplicate RiceWM SDK version '+version);
        if(!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)||!namespace||typeof namespace!=='object')throw Error('Invalid RiceWM SDK release');
        versions.add(version);
    }
    return new Map(entries);
}
const releases=createRiceSdkVersionRegistry([['1.0.0',sdk100]]);
export const RICE_SDK_VERSIONS=Object.freeze([...releases.keys()]);
const semantic=/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
export function resolveRiceSdkVersion(sdk){
    if(sdk!==undefined&&(!sdk||typeof sdk!=='object'||Array.isArray(sdk)))throw TypeError('manifest sdk must be an object');
    if(sdk?.version===undefined)return CURRENT_SDK_VERSION;
    if(typeof sdk.version!=='string'||!semantic.test(sdk.version))throw TypeError('manifest sdk.version must be an exact semantic version');
    const version=sdk.version;
    if(!releases.has(version))throw RangeError('RiceWM SDK version '+sdk.version+' is unavailable');return version;
}
export function riceSdkLibrary(sdk){return releases.get(resolveRiceSdkVersion(sdk));}
export function riceSdkServiceMethod(permission,sdk){return riceSdkLibrary(sdk).sdkServiceMethod(permission);}
export function riceSdkModuleUrls(sdk){
    const version=resolveRiceSdkVersion(sdk),base=new URL('./versions/'+version+'/runtime/sdk/js/',import.meta.url);
    return Object.freeze({version,index:new URL('index.mjs',base).href,runtime:new URL('runtime.mjs',base).href,bus:new URL('bus.mjs',base).href});
}
