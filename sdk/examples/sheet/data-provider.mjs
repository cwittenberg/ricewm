/** Query-job adapters only: criteria and cursors are control data. The workbook,
 * row masks, distinct indexes and searching stay inside the Wasm engine.
 * A provider owns at most one job and one outstanding semantic query.
 */
const error = (code, message) => Object.assign(new Error(message), { code });
const integer = value => Number.isSafeInteger(value) && value >= 0;
const empty = revision => ({ entries: [], offset: 0, next: null, total: 0, revision });

async function scope(query, sheetId) {
    if (typeof query !== 'function' || typeof sheetId !== 'string' || !sheetId.length) throw error('INVALID_QUERY', 'Query and sheet identity are required');
    const used = await query({ action: 'used-range', sheetId });
    const active = await query({ action: 'filter-state', sheetId });
    if (active.dataRevision !== used.dataRevision) throw error('STALE_QUERY', 'Workbook changed while opening the query');
    const range = used.result;
    if (!range || range.sheetId !== sheetId || !integer(used.dataRevision)) throw error('INVALID_QUERY', 'Invalid engine used range');
    const filter = active.result;
    if (filter !== null && (!Array.isArray(filter.criteria) || !integer(filter.firstRow) || !integer(filter.lastRow) ||
        !integer(filter.firstColumn) || !integer(filter.lastColumn) || filter.firstColumn > filter.lastColumn || filter.lastColumn >= 16384)) throw error('INVALID_QUERY', 'Invalid engine filter state');
    return { sheetId, filterFeatures: Array.isArray(range.filterFeatures) ? range.filterFeatures : [], dataRevision: used.dataRevision, filtered: filter !== null, empty: range.firstRow === null,
        firstRow: filter?.firstRow ?? range.firstRow, lastRow: filter?.lastRow ?? range.lastRow,
        firstColumn: filter?.firstColumn ?? range.firstColumn, lastColumn: filter?.lastColumn ?? range.lastColumn, criteria: filter?.criteria || [] };
}

function owner(query, dataRevision) {
    let closed = false, epoch = 0, tail = Promise.resolve(), job = null, queued = 0;
    const check = mine => { if (closed || mine !== epoch) throw error('QUERY_CANCELLED', 'Query was closed or superseded'); };
    async function cancel() {
        const previous = job; job = null;
        if (previous) {
            try { await query({ action: 'cancel', ...previous }); }
            catch (cause) {
                // A data edit invalidates every engine job; closing its old
                // handle then requires no further cleanup. Worker teardown also
                // owns complete cleanup after loss of the transport.
                if (!closed && !['STALE_QUERY', 'INVALID_QUERY', 'CLOSED'].includes(cause.code)) throw cause;
            }
        }
    }
    async function ask(mine, request) {
        check(mine);
        const response = await query(request);
        check(mine);
        if (response.dataRevision !== dataRevision) throw error('STALE_QUERY', 'Workbook data changed. Close and reopen this dialog.');
        return response.result;
    }
    return {
        run(callback) {
            if (closed) return Promise.reject(error('CLOSED', 'Query provider is closed'));
            if (queued >= 16) return Promise.reject(error('BUSY', 'Too many pending query requests'));
            const mine = ++epoch; queued += 1;
            const work = tail.then(async () => {
                check(mine);
                const context = {
                    check: () => check(mine), cancel, ask: request => ask(mine, request),
                    get job() { return job; },
                    async begin(request) {
                        await cancel(); check(mine);
                        // Remember a late-created handle before checking cancellation
                        // so close() can always release its engine resource.
                        const response = await query(request);
                        if (!integer(response.result?.handle) || !response.result.handle) throw error('INVALID_QUERY', 'Invalid engine query handle');
                        job = { handle: response.result.handle, dataRevision: response.dataRevision };
                        check(mine);
                        if (response.dataRevision !== dataRevision) throw error('STALE_QUERY', 'Workbook data changed. Close and reopen this dialog.');
                    },
                    async advance(search = false) {
                        const started = Date.now();
                        for (let steps = 0; steps < 16384; steps += 1) {
                            const progress = await ask(mine, { action: 'step', ...job, budget: 4096 });
                            if (progress.done || (search && progress.pageReady)) return progress;
                            if (Date.now() - started > 30000) throw error('QUERY_LIMIT', 'Query took too long. Use a smaller data range.');
                            if (steps % 8 === 7) await new Promise(resolve => setTimeout(resolve, 0));
                            check(mine);
                        }
                        throw error('QUERY_LIMIT', 'Query exceeded its work limit');
                    },
                    async activate() {
                        check(mine);
                        const result = await query({ action: 'activate-filter', ...job });
                        job = null; // Engine consumes the mask; ownership has transferred.
                        check(mine); return result;
                    },
                    async clearFilter(sheetId) {
                        // Validate the captured data epoch before the mutation.
                        // Clearing a mask can recalculate SUBTOTAL and advance it;
                        // reads must still reject that new epoch on this provider.
                        await ask(mine, { action: 'filter-state', sheetId });
                        const response = await query({ action: 'clear-filter', sheetId });
                        check(mine);
                        if (!integer(response.dataRevision) || response.dataRevision < dataRevision) throw error('STALE_QUERY', 'Filter mutation returned an invalid data revision');
                        return response.result;
                    },
                };
                try { return await callback(context); }
                catch (cause) { await cancel().catch(() => {}); throw cause; }
            });
            tail = work.catch(() => {}).finally(() => { queued -= 1; });
            return work;
        },
        close() { closed = true; epoch += 1; return tail.then(cancel); },
    };
}

export async function openFilterDataProvider({ query, sheetId, column } = {}) {
    if (!integer(column) || column >= 16384) throw error('INVALID_QUERY', 'Invalid filter column');
    const domain = await scope(query, sheetId), jobs = owner(query, domain.dataRevision);
    // Initial AutoFilter convention: the first used row is a header. Existing
    // filters carry their exact data domain and must not lose another row.
    if (!domain.filtered && !domain.empty) { domain.firstRow += 1; domain.empty = domain.firstRow > domain.lastRow; }
    const current = domain.criteria.find(item => item.column === column)?.predicate;
    const supportsExclusion = domain.filterFeatures.includes('display-excluded');
    const typedFilters = domain.filterFeatures.includes('typed-filters');
    function checked(predicate) {
        if (predicate === null) return null;
        const fail = message => { throw error('UNSUPPORTED_FILTER', message); };
        if (!predicate || typeof predicate !== 'object' || Array.isArray(predicate)) fail('Invalid filter criterion');
        if (['displayValues','displayExcluded'].includes(predicate.type)) {
            if (predicate.type === 'displayExcluded' && !supportsExclusion) fail('This engine does not support Select All exclusions');
            if (!Array.isArray(predicate.values) || predicate.values.length > 256 || predicate.values.some(value => typeof value !== 'string') || typeof predicate.blank !== 'boolean') fail('Invalid displayed-value filter state');
        } else if (typedFilters && predicate.type === 'number') {
            if (!['equal','notEqual','less','lessEqual','greater','greaterEqual'].includes(predicate.comparison) || !Number.isFinite(predicate.value)) fail('Invalid numeric filter');
        } else if (typedFilters && predicate.type === 'text') {
            if (typeof predicate.literal !== 'string' || predicate.caseSensitive !== false || typeof predicate.exact !== 'boolean') fail('Invalid text filter');
        } else if (!(typedFilters && predicate.type === 'blank' && typeof predicate.blank === 'boolean')) {
            fail('This column uses a filter that the current editor cannot change. Clear filters first.');
        }
        if (new TextEncoder().encode(JSON.stringify(predicate)).length > 32768) fail('Filter criterion exceeds its32KiB budget');
        return structuredClone(predicate);
    }
    if (current) checked(current);
    const valuesOf = predicate => predicate ? [...predicate.values.map(value => ({ kind:'text', value })), ...(predicate.blank ? [{kind:'blank'}] : [])] : null;
    let ready = false;
    const request = action => ({ action, sheetId, firstRow: domain.firstRow, lastRow: domain.lastRow, criteria: domain.criteria,
        ...(action === 'begin-filter' ? { firstColumn: domain.firstColumn, lastColumn: domain.lastColumn } : {}) });
    return Object.freeze({
        displayValues: true,
        supportsExclusion, typedFilters, criterion: current ? structuredClone(current) : null,
        selected: valuesOf(current?.type === 'displayValues' ? current : null),
        excluded: valuesOf(current?.type === 'displayExcluded' ? current : null),
        headerConvention: !domain.filtered,
        query({ offset = 0, limit = 64, literal = '' } = {}) {
            return jobs.run(async task => {
                if (domain.empty) return empty(domain.dataRevision);
                if (!task.job) { await task.begin({ ...request('begin-distinct'), column, ignoreOwnColumn: true, displayValues: true }); ready = false; }
                if (!ready) { await task.advance(); ready = true; }
                return task.ask({ action: 'distinct-page', ...task.job, offset, limit, literal, caseSensitive: false });
            });
        },
        apply(predicate) {
            try { predicate = checked(predicate); } catch (cause) { return Promise.reject(cause); }
            return jobs.run(async task => {
                const criteria = domain.criteria.filter(item => item.column !== column);
                if (predicate !== null) criteria.push({ column, predicate });
                if (!criteria.length) { await task.cancel(); return task.clearFilter(sheetId); }
                if (domain.empty) throw error('INVALID_QUERY', 'Add data before applying a filter');
                await task.begin({ ...request('begin-filter'), criteria });
                await task.advance();
                return task.activate();
            });
        },
        close: () => jobs.close(),
    });
}

/** Forward-only Find deliberately retains no result cache. Its current page is
 * bounded; the engine retains undrained hits. Restart starts a new engine job.
 */
export async function openFindDataProvider({ query, sheetId } = {}) {
    const domain = await scope(query, sheetId), jobs = owner(query, domain.dataRevision);
    let key = null, nextOffset = 0;
    return Object.freeze({
        filtered: domain.filtered,
        query({ offset = 0, limit = 64, literal, caseSensitive = false, formulas = false }) {
            return jobs.run(async task => {
                if (domain.empty) return { ...empty(domain.dataRevision), forwardOnly: true, finalTotal: true };
                const signature = JSON.stringify([literal, caseSensitive, formulas]);
                if (!task.job || offset === 0 || signature !== key) {
                    if (offset !== 0) throw error('INVALID_QUERY', 'Restart Find before changing search options');
                    await task.begin({ action: 'begin-search', sheetId, firstRow: domain.firstRow, lastRow: domain.lastRow,
                        criteria: domain.criteria, search: { literal, caseSensitive, formulas, firstColumn: domain.firstColumn, lastColumn: domain.lastColumn } });
                    key = signature; nextOffset = 0;
                }
                if (offset !== nextOffset) throw error('INVALID_QUERY', 'Find pages are forward-only. Restart to search again.');
                await task.advance(true);
                const page = await task.ask({ action: 'search-page', ...task.job, limit });
                if (page.offset !== offset || page.entries.length > limit) throw error('INVALID_QUERY', 'Engine returned an unexpected search page');
                nextOffset = page.next;
                if (page.next === null) await task.cancel();
                return { ...page, forwardOnly: true, finalTotal: page.next === null };
            });
        },
        close: () => jobs.close(),
    });
}
