import {OFFICE_FONT_FACES} from './rice-office-font-manifest.mjs';
import {RiceCapabilityHydrator} from './rice-capability-hydration.mjs';

export function createOfficeFontLoader(options = {}) {
    const loader = new RiceCapabilityHydrator({baseUrl:import.meta.url, ...options});
    return async function loadOfficeFontBytes(id, onProgress = () => {}) {
        const spec = OFFICE_FONT_FACES[id];
        if (!spec || spec.id !== id) throw new Error('Unknown document font');
        const capability = {id:`font:office:${id}`,label:spec.css_family};
        onProgress({...capability,state:'loading',phase:'Loading document font',loaded:0,total:spec.bytes});
        try {
            const url = new URL(spec.web_path, import.meta.url);
            // Keep the immutable object identity explicit in the request cache key.
            // A previously cached missing-object response must not mask a newly
            // published font. Verified local capability caches still use its digest.
            url.searchParams.set('sha256', spec.sha256);
            const bytes = await loader.asset({url:url.href,
                sha256:spec.sha256,bytes:spec.bytes},loaded=>onProgress({...capability,state:'loading',
                    phase:'Loading document font',loaded,total:spec.bytes}));
            onProgress({...capability,state:'ready',loaded:spec.bytes,total:spec.bytes});
            return bytes;
        } catch (error) {
            onProgress({...capability,state:'failed',error:String(error?.message || error)});
            throw error;
        }
    };
}
export const loadOfficeFontBytes = createOfficeFontLoader();
