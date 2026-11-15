/** Optional, session-owned processing of already decoded stereo PCM. */
export const AUDIO_EQ_BANDS = Object.freeze([60,170,310,600,1000,3000,6000,12000,14000,16000]);
export function audioEffects(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value) ||
        Object.keys(value).sort().join(',') !== 'balance,bands,enabled,preampDb' ||
        typeof value.enabled !== 'boolean' || !Array.isArray(value.bands) || value.bands.length !== AUDIO_EQ_BANDS.length ||
        value.bands.some(gain => !Number.isFinite(gain) || gain < -12 || gain > 12) ||
        !Number.isFinite(value.preampDb) || value.preampDb < -12 || value.preampDb > 12 ||
        !Number.isFinite(value.balance) || value.balance < -1 || value.balance > 1) throw TypeError('Invalid session audio effects');
    return Object.freeze({ enabled: value.enabled, bands: Object.freeze([...value.bands]), preampDb: value.preampDb, balance: value.balance });
}
audioEffects.controlFactory = Object.freeze({ topics: ['media'],
    constructor: 'audioEffects({enabled,bands:[10 gains in dB],preampDb,balance})',
    methods: 'Validate media.control({handle,command:"effects",value}). Optional processing applies only to this owned decoder session. Ten gains and preamp are bounded to -12..12 dB; balance -1..1 attenuates one stereo channel. enabled:false bypasses EQ/preamp. Unconfigured and neutral sessions retain the original PCM output path.' });
