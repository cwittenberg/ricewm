// Bounded, timestamped observations. Missing measurements stay missing.
export const SAMPLE_INTERVAL = 2_000;
export const RETENTION = 30 * 60_000;
export const usagePercent = (used, total) => total > 0
    ? Math.max(0, Math.min(100, used * 100 / total)) : null;
export const formatBytes = value => {
    if (value == null || !Number.isFinite(value)) return 'Unavailable';
    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
    let scaled = Math.max(0, value), index = 0;
    while (scaled >= 1024 && index < units.length - 1) { scaled /= 1024; index++; }
    return `${scaled.toFixed(index && scaled < 100 ? 1 : 0)} ${units[index]}`;
};
export const storageCategories = snapshot => {
    if (!(snapshot.diskTotal > 0)) return [];
    const names = {fileSystem: 'Files', caches: 'Cache storage', cacheStorage: 'Cache storage',
        indexedDB: 'IndexedDB', serviceWorkerRegistrations: 'Service workers', serviceWorkers: 'Service workers', other: 'Other'};
    const used = Math.max(0, snapshot.diskTotal - snapshot.diskFree);
    const categories = new Map();
    for (const [key, size] of snapshot.storageBreakdown) {
        if (!(size > 0) || !Number.isFinite(size)) continue;
        const name = names[key] || key;
        categories.set(name, (categories.get(name) || 0) + size);
    }
    const measured = [...categories.values()].reduce((a, b) => a + b, 0);
    if (used > measured) categories.set('Other', (categories.get('Other') || 0) + used - measured);
    const sorted = [...categories].sort((a, b) => b[1] - a[1]);
    // Keep every byte when a browser reports more categories than fit on screen.
    if (sorted.length > 8) sorted.splice(7, sorted.length - 7,
        ['Other categories', sorted.slice(7).reduce((sum, entry) => sum + entry[1], 0)]);
    return sorted.map(([name, value]) => ({name, value}));
};
export class TelemetryHistory {
    constructor() { this.rows = []; }
    record(snapshot, time = Date.now()) {
        // A wall-clock correction must not reverse the chart's time axis.
        time = Math.max(time, (this.rows.at(-1)?.time ?? time - 1) + 1);
        const memory = usagePercent(snapshot.memoryUsed, snapshot.memoryTotal);
        const row = {time, cpu: snapshot.cpu, memory, gpu: snapshot.gpu,
            used: snapshot.memoryTotal > 0 ? snapshot.memoryUsed : null,
            available: snapshot.memoryTotal > 0 ? Math.max(0, snapshot.memoryTotal - snapshot.memoryUsed) : null,
            received: snapshot.received, sent: snapshot.sent};
        const last = this.rows.at(-1);
        if (last && time - last.time > SAMPLE_INTERVAL * 1.75) {
            this.rows.push(Object.fromEntries(Object.keys(row).map(key => [key, key === 'time' ? last.time + SAMPLE_INTERVAL : null])));
        }
        this.rows.push(row);
        while (this.rows.length && (this.rows[0].time < time - RETENTION || this.rows.length > 900)) this.rows.shift();
        return row;
    }
    window(duration, end = this.rows.at(-1)?.time ?? Date.now(), rows = this.rows) {
        return rows.filter(row => row.time >= end - duration && row.time <= end);
    }
}
