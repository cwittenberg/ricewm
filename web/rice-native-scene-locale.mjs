import {loadRiceLocale, ensureLocaleFonts, setRiceLocale, resolveLanguage} from './rice-i18n.mjs';

/** Scene metadata crosses worker boundaries; displayed application/user text does not. */
export class RiceNativeSceneLocale {
    constructor(document = globalThis.document, onFontError = () => {}) {
        this.document = document;
        this.onFontError = onFontError;
        this.generation = 0;
        this.applied = null;
        this.fontWork = Promise.resolve();
    }

    prepare(snapshot) {
        // Older scene producers have no locale metadata. Keep their existing
        // receiving-realm language instead of guessing from visible strings.
        if (typeof snapshot?.locale !== 'string') {
            ++this.generation;
            this.requested = null;
            return Promise.resolve(true);
        }
        const requested = resolveLanguage(snapshot.locale);
        if (requested === this.requested) return this.preparation;
        this.requested = requested;
        const generation = ++this.generation;
        this.preparation = this.prepareLanguage(requested, generation).catch(error => {
            if (generation === this.generation) this.requested = null;
            throw error;
        });
        return this.preparation;
    }

    async prepareLanguage(requested, generation) {
        const locale = await loadRiceLocale(requested);
        if (generation !== this.generation) return false;
        // Font registration order selects CJK glyph variants. Serialize it so
        // an older slow font load cannot replace a newer language's font faces.
        const work = this.fontWork.catch(() => {}).then(async () => {
            if (generation !== this.generation) return;
            try { await ensureLocaleFonts(locale, this.document); }
            catch (error) {
                // Use browser font fallback without discarding the current
                // scene or rewriting already-localized application text.
                if (generation === this.generation) this.onFontError(error);
            }
        });
        this.fontWork = work;
        await work;
        if (generation !== this.generation) return false;
        setRiceLocale(locale, this.document);
        this.applied = locale;
        return true;
    }
}
