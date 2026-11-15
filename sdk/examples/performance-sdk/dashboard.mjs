import {
    Button, Chart, CheckBox, Column, Label, Message, Row, ScrollView,
    SceneBuilder, SceneOverlay, SceneSurface, Stack, State, Table, Tooltip, resolveChartTheme,
} from '@ricewm/app';
import {TelemetryHistory, formatBytes, storageCategories, usagePercent} from './telemetry.mjs';
import {encodeTable, seriesLabel, tableText} from './table-export.mjs';

const flat = State.ENABLED | State.FLAT;
const label = (text, state = 0, name = null) => new Label({text, name, state: State.ENABLED | State.START_ALIGNED | state});
const row = children => {
    const root = new Column({text: 'panel:fixed:flush', state: flat});
    let compact = null;
    root.setCompact = value => {
        if (compact === value) return;
        compact = value;
        if (root.children[0]) {
            const previous = root.children[0];
            for (const child of [...previous.children]) previous.remove(child);
            root.remove(previous, {destroy: true});
        }
        root.append(new (compact ? Column : Row)({text: compact ? 'panel:fixed:flush:gap=12' : 'panel:equal', state: flat}, children));
    };
    root.setCompact(false);
    return root;
};
const card = children => new Column({text: 'panel:fixed:content-card:padding=10:gap=6', state: flat}, children);
const clock = time => new Date(time).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false});
const pct = value => value == null ? 'Unavailable' : `${Math.round(value)}%`;

// The SDK owns the native ECharts engine, painting, input, data view and motion.
// Only monitor-specific formatting and fixed telemetry axes live here.
class MonitorChart extends Chart {
    constructor({id, height, option, describe, caption, theme}) {
        super({id, name: id, height, option, theme, animation: false});
        this.describePoint = describe;
        this.caption = caption || '';
        this.text = 'panel:fixed:flush:gap=4';
    }
    describe(point) { return this.describePoint?.(point) ?? super.describe(point); }
    dataCaption() { return this.caption || ''; }
    getDataTable() {
        return super.getDataTable().map(item => {
            const series = seriesLabel(item.series, this.caption || this.chartId);
            const x = Array.isArray(item.value) ? clock(item.value[0]) : tableText(item.x);
            return {...item, series, id: `${series}:${Array.isArray(item.value) ? item.value[0] : x}`,
                x, value: Array.isArray(item.value) ? item.value[1] : item.value};
        });
    }
    renderTable() {
        const rows = this.getDataTable();
        if (!this.dataView) {
            this.dataView = new Table({name: `${this.chartId}-data`, rowKey: 'id', data: rows,
                height: 220, selectionMode: 'row', columns: [
                    {key:'series',title:'Series',width:110}, {key:'x',title:'Time / Category',width:140},
                    {key:'value',title:'Value',width:120,align:'right'},
                ]});
            // Use the same native accessory sizing as Rice Sheet. The native
            // ScrollBar owns its themed chrome and input; no oversized track.
            this.dataView.verticalTrack.text = 'panel:flush:equal-fixed:accessory-width=16';
            this.dataView.horizontalTrack.text = 'panel:flush:equal-fixed:accessory-height=16';
            this.dataView.on(Message.SELECT, event => {
                const row = event.detail?.rows?.[0]; if (row) this.selectMark(row.index);
            });
            this.table.append(this.dataView);
        } else this.dataView.setData(rows);
    }
    setTheme(active) { super.setTheme(active); this.rebuild?.(); return this; }
}

export class PerformanceDashboard {
    constructor({app, services, log}) {
        this.app = app;
        this.services = services;
        this.log = log;
        this.history = new TelemetryHistory();
        this.snapshot = null;
        this.error = false;
        this.duration = 120_000;
        this.paused = false;
        this.pages = new Map();
        this.active = null;
        this.charts = new Set();
        this.cleanups = [];
        this.palette = resolveChartTheme(app.theme || {});
        this.stopTheme = app.on(Message.THEME_CHANGED, active => {
            this.palette = resolveChartTheme(active);
            for (const page of this.pages.values()) page.paint();
            this.update(true);
        });
    }
    record(snapshot) {
        this.snapshot = snapshot;
        this.history.record(snapshot);
        this.error = false;
        this.update();
    }
    failed() { this.error = true; this.update(); }
    get displayed() { return this.paused ? this.frozenSnapshot : this.snapshot; }
    get rows() {
        const rows = this.paused ? this.frozenRows : this.history.rows;
        return this.history.window(this.duration, rows.at(-1)?.time, rows);
    }
    get end() { return (this.paused ? this.frozenRows : this.history.rows).at(-1)?.time ?? Date.now(); }
    togglePause() {
        this.paused = !this.paused;
        if (this.paused) { this.frozenSnapshot = this.snapshot; this.frozenRows = [...this.history.rows]; }
        this.update(true);
    }
    metric(title, {compact = false} = {}) {
        const number = label('—', State.DISPLAY | (compact ? State.END_ALIGNED | State.EXPANSIVE : 0));
        const note = label('', State.MUTED);
        const heading = label(title, State.EMPHASIZED);
        return {root: card(compact ? [new Row({text:'panel:fixed:flush:gap=8',state:flat},[heading,number]),note] : [heading, number, note]), heading, number, note};
    }
    chart(id, height, makeOption, caption, describe) {
        const chart = new MonitorChart({id, height, option: makeOption(this.palette), caption, describe, theme: this.palette});
        chart.rebuild = () => {
            const option = makeOption(chart.theme), next = JSON.stringify(option);
            if (JSON.stringify(chart.rawOption) === next) return;
            chart.setOption(option);
        };
        // Keep the live readout and data action on one quiet footer line.
        const actions = chart.tableToggle.parent;
        actions.remove(chart.tableToggle);
        actions.visible = false;
        chart.remove(chart.details);
        chart.remove(chart.table);
        chart.details.setProperty('state', State.ENABLED | State.MUTED | State.EXPANSIVE);
        chart.tableToggle.text = 'Data';
        const actionsForTable = new Row({text:'panel:fixed:flush:gap=4',state:flat,visible:false},
            [['copy',23,'Copy table'],['sheet',109,'Open in Rice Sheet']].map(([action,icon,title]) => {
                const button = new Button({name:`${id}-${action}`,icon,accessibleName:title,text:'',state:flat},[new Tooltip({text:title})]);
                button.on(Message.ACTIVATE, () => void this.exportTable(chart, action, button));
                return button;
            }));
        chart.tableToggle.on(Message.ACTIVATE, () => {
            chart.tableToggle.text = chart.table.visible ? 'Hide data' : 'Data';
            actionsForTable.visible = chart.table.visible;
        });
        chart.append(new Row({text: 'panel:fixed:flush:gap=8', state: flat}, [chart.details, actionsForTable, chart.tableToggle]));
        chart.append(chart.table);
        this.charts.add(chart);
        chart.ready.catch(error => this.log.error('Monitor chart failed', {error: String(error)}));
        return chart;
    }
    async exportTable(chart, action, button) {
        button.enabled = false;
        try {
            const selected = chart.dataView?.getSelectedRows() || [];
            const rows = selected.length ? selected : chart.getDataTable();
            if (action === 'copy') {
                await this.services.clipboard.writeText(encodeTable(rows, 'tsv'));
                chart.details.text = 'Copied';
            } else {
                const name = `${chart.chartId}-${Date.now()}.csv`;
                const csv = encodeTable(rows, 'csv');
                await this.services.files.write({path:name,data:new TextEncoder().encode(csv)});
                await this.services.applications.launch({id:'riceapp-org.ricewm.sheet.desktop',
                    paths:[`/root/.local/share/ricewm/app-data/dev.ricewm.performance-sdk/${name}`]});
            }
        } catch (error) {
            chart.details.text = String(error?.message || error);
            this.log.warn('Table action failed', {error:String(error?.message || error)});
        } finally { button.enabled = true; }
    }
    gauge(id, title, value, detail, seriesIndex = 0) {
        const heading = label(title, State.EMPHASIZED);
        const note = new Tooltip({text: title});
        const chart = this.chart(id, 160, theme => {
            const measurement = value(this.displayed);
            const known = measurement != null;
            const color = measurement >= 90 ? theme.negative : theme.series[seriesIndex];
            return {series: [{id, type: 'gauge', min: 0, max: 100, startAngle: 210, endAngle: -30,
                radius: '125%', center: ['50%', '64%'], splitNumber: 4,
                axisLine: {lineStyle: {width: 12, color: [[1, theme.grid]]}},
                progress: {show: true, width: 12, roundCap: true, itemStyle: {color, opacity: known ? 1 : 0}},
                pointer: {show: true, length: '64%', width: 3, itemStyle: {color: theme.foreground, opacity: known ? 1 : 0}},
                anchor: {show: true, size: 8, itemStyle: {color: theme.foreground, opacity: known ? 1 : 0}},
                axisTick: {show: false}, splitLine: {show: true, length: 5, distance: 3, lineStyle: {color: theme.muted, width: 1}},
                axisLabel: {show: false}, title: {show: false},
                detail: {offsetCenter: [0, '38%'], fontSize: 27, fontWeight: 700, color: known ? theme.foreground : theme.muted,
                    formatter: () => known ? pct(measurement) : '—'},
                data: [{value: known ? Math.max(0, Math.min(100, measurement)) : 0, name: title}]}]};
        });
        chart.details.visible = false;
        chart.tableToggle.parent.visible = false;
        chart.surface.accessibleName = `${title} utilization gauge`;
        heading.setProperty('state', State.ENABLED | State.EMPHASIZED | State.CENTER_ALIGNED);
        const root = card([heading, chart, note]);
        return {root, update: () => {
            const snapshot = this.displayed;
            heading.text = value(snapshot) == null ? `${title} · Unavailable` : title;
            note.text = detail(snapshot);
            chart.rebuild();
        }};
    }
    trace(id, title, fields, {height = 220, bytes = false, stacked = false, paletteIndices = null, compact = false} = {}) {
        const hidden = new Set();
        const buttons = [];
        let chart;
        const format = value => value == null ? 'Unavailable' : bytes ? formatBytes(value) : pct(value);
        const legend = new Row({text: 'panel:fixed:flush:wrap-controls:gap=12', state: flat}, fields.map(([key, name], index) => {
            const checkbox = new CheckBox({name: `${id}-series-${key}`, text: name, checked: true});
            checkbox.on(Message.ACTIVATE, () => {
                if (hidden.has(key)) hidden.delete(key); else hidden.add(key);
                checkbox.checked = !hidden.has(key);
                chart.rebuild();
            });
            buttons.push([checkbox, key, name]);
            // The three gauges already identify these series by color. Keep
            // the small Overview legend wide enough for every full name.
            if (compact && !bytes) return checkbox;
            const swatch = new SceneSurface({accessibleName: `${name} color`, backgroundColor: 'transparent'});
            const paint = () => {
                const color = this.palette.series[paletteIndices?.[index] ?? index];
                const n = parseInt(color.slice(1), 16), packed = (0xff000000 | ((n & 255) << 16) | (n & 0xff00) | (n >>> 16)) >>> 0;
                swatch.submit(new SceneBuilder(12, 12).ellipse([1, 1, 10, 10], packed).finish());
            };
            paint();
            const stop = this.app.on(Message.THEME_CHANGED, paint); this.cleanups.push(stop);
            return new Row({text: 'panel:fixed:flush:wrap-controls:gap=4', state: flat}, [
                new Column({text: 'panel:fixed:width=16:flush', state: flat}, [swatch]), checkbox,
            ]);
        }));
        
        chart = this.chart(id, height, theme => {
            const rows = this.rows, end = this.end;
            const max = bytes ? Math.max(1024, ...rows.map(row => stacked ? (row.used || 0) + (row.available || 0)
                : Math.max(...fields.map(([key]) => row[key] || 0)))) : 100;
            const unit = bytes ? max >= 1024 ** 3 ? 1024 ** 3 : max >= 1024 ** 2 ? 1024 ** 2 : 1024 : 1;
            const rawStep = bytes ? max / unit / 4 : 25;
            const magnitude = 10 ** Math.floor(Math.log10(rawStep));
            const step = [1, 2, 2.5, 5, 10].find(value => value * magnitude >= rawStep) * magnitude * unit;
            const unitName = !bytes ? '%' : unit === 1024 ** 3 ? 'GiB' : unit === 1024 ** 2 ? 'MiB' : 'KiB';
            return {
                grid: {left: 58, right: 20, top: 28, bottom: 32},
                xAxis: {type: 'time', min: end - this.duration, max: end, splitNumber: compact || chart?.width < 420 ? 2 : 3, minInterval: this.duration / (compact || chart?.width < 420 ? 2 : 4),
                    axisLabel: {fontSize: 11, hideOverlap: true, formatter: value => new Date(value).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false})},
                    axisTick: {show: false}, splitLine: {show: false}},
                yAxis: {type: 'value', min: 0, max: bytes ? stacked ? max : step * 4 : 100, interval: bytes ? stacked ? max / 4 : step : 25, splitNumber: 4,
                    name: `${unitName}${id.includes('network') ? '/s' : ''}`, nameLocation: 'end', nameTextStyle: {fontSize: 11},
                    axisLabel: {fontSize: 11, formatter: value => Number((value / unit).toFixed(2)).toString()},
                    axisTick: {show: false}, axisLine: {show: false}, splitLine: {lineStyle: {type: 'dashed', color: theme.grid}}},
                series: fields.filter(([key]) => !hidden.has(key)).map(([key, name]) => {
                    const index = fields.findIndex(field => field[0] === key);
                    const color = theme.series[paletteIndices?.[index] ?? index];
                    return {id: key, name, type: 'line', ...(stacked ? {stack: 'memory'} : {}),
                        smooth: false, connectNulls: false, showSymbol: true, symbol: 'circle', symbolSize: 4,
                        lineStyle: {width: 2, color}, itemStyle: {color},
                        areaStyle: {opacity: stacked ? .24 : .08, color},
                        data: rows.map(row => ({name: clock(row.time), value: [row.time, row[key]]}))};
                }),
            };
        }, '', point => {
            const value = Array.isArray(point.value) ? point.value[1] : point.value;
            return `${point.x}  ·  ${point.name}  ${format(value)}${id.includes('network') && value != null ? '/s' : ''}`;
        });
        chart.surface.accessibleName = title;
        // An empty series list is valid ECharts, but the SDK requires at least
        // one series. Keep one hidden, empty series when all traces are off.
        const rebuild = chart.rebuild;
        chart.rebuild = () => {
            if (hidden.size === fields.length) {
                chart.setOption({xAxis: {show: false}, yAxis: {show: false}, series: [{type: 'line', data: []}]});
                chart.details.text = 'All series hidden. Select a legend item to show it.';
            } else rebuild();
        };
        return {root: card([label(title, State.EMPHASIZED), legend, chart]), chart,
            update: () => {
                const rows = this.rows;
                for (const [button, key, name] of buttons) button.text = compact && !bytes ? name : `${name}  ${format(rows.at(-1)?.[key])}${id.includes('network') ? '/s' : ''}`;
                const primary = fields[0], rateUnit = id.includes('network') ? '/s' : '', values = rows.map(row => row[primary[0]]).filter(value => value != null);
                chart.caption = values.length ? `${primary[1]}  ·  Avg ${format(values.reduce((a, b) => a + b, 0) / values.length)}${rateUnit}  ·  Peak ${format(Math.max(...values))}${id.includes('network') ? '/s' : ''}`
                    : 'Waiting for measurements';
                chart.rebuild();
            }};
    }
    toolbar(name) {
        const status = label('', State.MUTED | State.EXPANSIVE, `monitor-${name}-status`);
        const ranges = (name === 'storage' ? [] : [60_000, 120_000, 300_000]).map(duration => {
            const button = new Button({name: `monitor-${name}-range-${duration / 60_000}`, text: `${duration / 60_000}m`,
                documentFont: {family: 'sans-serif', size_pt: 10, weight: 400, italic: false, underline: false},
                accessibleName: `Show the last ${duration / 60_000} minutes`, state: flat});
            button.on(Message.ACTIVATE, () => { this.duration = duration; this.update(true); });
            return [button, duration];
        });
        const pause = new Button({name: `monitor-${name}-pause`, text: 'Pause'});
        pause.on(Message.ACTIVATE, () => this.togglePause());
        const root = new Row({text: 'panel:fixed:flush:gap=8', state: flat}, [
            status, ...ranges.map(([button]) => button), pause,
        ]);
        return {root, update: () => {
            const time = this.end;
            status.text = `${this.paused ? 'Paused' : this.error ? 'Connection interrupted' : 'Live'}  ·  ${clock(time)}`;
            status.setProperty('state', State.ENABLED | State.START_ALIGNED | State.EXPANSIVE | (this.error ? State.WARNING : State.MUTED));
            pause.text = this.paused ? 'Resume live' : 'Pause';
            for (const [button, duration] of ranges) {
                button.selected = this.duration === duration;
                button.setProperty('state', flat | (this.duration === duration ? State.SELECTED : 0));
                const paint = button.getProperty('documentPaint');
                if (paint?.foreground !== this.palette.foreground || paint?.background !== this.palette.panel)
                    button.setProperty('documentPaint', {foreground: this.palette.foreground, background: this.palette.panel});
            }
        }};
    }
    overview() {
        const cpu = this.gauge('overview-cpu', 'CPU', s => s?.cpu ?? null,
            s => s?.cpuScope || 'Waiting for CPU telemetry');
        const memory = this.gauge('overview-memory', 'Memory', s => s ? usagePercent(s.memoryUsed, s.memoryTotal) : null,
            s => s?.memoryTotal > 0 ? `${formatBytes(s.memoryUsed)} of ${formatBytes(s.memoryTotal)}` : 'Capacity unavailable', 1);
        const gpu = this.gauge('overview-gpu', 'GPU', s => s?.gpu ?? null,
            s => s?.gpu == null ? 'Timing unavailable' : 'Renderer frame budget', 2);
        const activity = this.trace('overview-activity', 'Resource activity', [['cpu', 'CPU'], ['memory', 'Memory'], ['gpu', 'GPU']], {height: 160, compact: true});
        const network = this.trace('overview-network', 'Network', [['received', '↓'], ['sent', '↑']], {height: 160, bytes: true, compact: true});
        const storage = this.gauge('overview-storage', 'Storage',
            s => s?.diskTotal > 0 ? usagePercent(s.diskTotal - s.diskFree, s.diskTotal) : null,
            s => s?.diskTotal > 0 ? `${formatBytes(s.diskTotal - s.diskFree)} used · ${formatBytes(s.diskFree)} available` : 'Capacity unavailable', 3);
        const gridRow = children => new Row({text: 'panel:equal', state: flat}, children);
        const gauges = new Column({text: 'panel:fixed:flush:gap=8', state: flat});
        let compact = null;
        gauges.compactAt = 900;
        gauges.setCompact = value => {
            if (compact === value) return;
            compact = value;
            for (const item of [cpu.root, memory.root, gpu.root, storage.root]) item.parent?.remove(item);
            for (const child of [...gauges.children]) gauges.remove(child, {destroy: true});
            if (compact) { gauges.append(gridRow([cpu.root, memory.root])); gauges.append(gridRow([gpu.root, storage.root])); }
            else gauges.append(gridRow([cpu.root, memory.root, gpu.root, storage.root]));
        };
        gauges.setCompact(false);
        return {children: [gauges, row([activity.root, network.root])],
            update: () => {
                for (const gauge of [cpu, memory, gpu, storage]) gauge.update();
                activity.update();
                network.update();
            }};
    }
    resources() {
        const cpu = this.trace('resources-cpu', 'CPU utilization', [['cpu', 'CPU']], {height: 240});
        const gpu = this.trace('resources-gpu', 'GPU renderer timing', [['gpu', 'GPU']], {height: 240, paletteIndices: [2]});
        const memory = this.trace('resources-memory', 'Memory allocation', [['used', 'Used'], ['available', 'Available']], {height: 240, bytes: true, stacked: true, paletteIndices: [1, 3]});
        const scope = new Tooltip({text: 'CPU utilization'});
        cpu.root.append(scope);
        return {children: [row([cpu.root, memory.root, gpu.root])], update: () => {
            for (const trace of [cpu, gpu, memory]) trace.update();
            const s = this.displayed;
            scope.text = `${s?.cpuScope || 'CPU'}  ·  Load ${s?.load || '—'}  ·  ${s?.processes.length || 0} processes`;
            gpu.root.children[0].text = s?.gpu == null ? 'GPU · Timing unavailable' : 'GPU renderer timing';
        }};
    }
    network() {
        const download = this.metric('↓ Download', {compact:true});
        const upload = this.metric('↑ Upload', {compact:true});
        const throughput = this.trace('network-throughput', 'Transfer rate', [['received', 'Download'], ['sent', 'Upload']], {height: 200, bytes: true});
        const connection = label('', State.MUTED);
        return {children: [row([download.root, upload.root]), throughput.root, connection], update: () => {
            throughput.update();
            const s = this.displayed;
            for (const [metric, rate, total] of [[download, 'received', 'receivedTotal'], [upload, 'sent', 'sentTotal']]) {
                metric.number.text = s ? `${formatBytes(s[rate])}/s` : '—';
                metric.note.text = s ? `Total ${formatBytes(s[total])}` : 'Waiting for telemetry';
            }
            connection.text = s ? `${s.ipv4} · ${s.networkMode} · ${s.sockets} sockets` : 'Address unavailable';
        }};
    }
    storage() {
        const capacity = this.metric('Workspace quota');
        const available = this.metric('Available');
        const state = label('', State.MUTED, 'storage-state');
        const donut = this.chart('storage-capacity', 230, theme => {
            const s = this.displayed;
            return {series: [{id: 'storage-capacity', name: 'Storage', type: 'pie', animation: false, radius: ['62%', '82%'], center: ['50%', '50%'], label: {show: false},
                itemStyle: {borderWidth: 3, borderColor: theme.panel, borderRadius: 4},
                data: s?.diskTotal > 0 ? [
                    {name: 'Used', value: Math.max(0, s.diskTotal - s.diskFree), itemStyle: {color: theme.accent}},
                    {name: 'Available', value: Math.max(0, s.diskFree), itemStyle: {color: theme.grid}},
                ] : []}]};
        }, 'Used / Available', point => `${point.x}: ${formatBytes(point.value)}`);
        const used = label('—', State.DISPLAY | State.CENTER_ALIGNED);
        const bars = this.chart('storage-categories', 230, theme => {
            const data = this.displayed ? storageCategories(this.displayed) : [];
            return {grid: {left: 120, right: 85, top: 12, bottom: 12},
                xAxis: {type: 'value', show: false, min: 0},
                yAxis: {type: 'category', inverse: true, data: data.map(item => item.name),
                    axisTick: {show: false}, axisLine: {show: false}, axisLabel: {fontSize: 12, width: 110, overflow: 'truncate'}},
                series: [{type: 'bar', name: 'Storage', barMaxWidth: 22, showBackground: true,
                    backgroundStyle: {color: theme.grid, borderRadius: 4}, itemStyle: {color: theme.accent, borderRadius: 4},
                    label: {show: true, position: 'right', color: theme.foreground, fontSize: 11, formatter: item => formatBytes(item.value)},
                    data: data.map(item => ({name: item.name, value: item.value}))}]};
        }, '', point => `${point.x}: ${formatBytes(point.value)}`);
        const charts = row([card([label('Capacity used', State.EMPHASIZED), donut, used]),
            card([label('Storage breakdown', State.EMPHASIZED), bars])]);
        capacity.note.visible = false;
        return {children: [row([capacity.root, available.root]), state, charts], update: () => {
            const s = this.displayed, known = s?.diskTotal > 0;
            capacity.number.text = known ? formatBytes(s.diskTotal) : 'Unavailable';
            capacity.note.text = 'Browser-managed capacity';
            available.number.text = known ? formatBytes(s.diskFree) : 'Unavailable';
            available.note.text = known ? `${formatBytes(Math.max(0, s.diskTotal - s.diskFree))} used` : 'Storage quota could not be read';
            state.text = !known ? 'Storage information unavailable. Waiting for a browser quota measurement.'
                : usagePercent(s.diskTotal - s.diskFree, s.diskTotal) >= 90 ? 'Workspace storage is nearly full.' : 'Workspace storage is available.';
            state.visible = !known || usagePercent(s.diskTotal - s.diskFree, s.diskTotal) >= 90;
            charts.visible = known;
            used.text = known ? `${pct(usagePercent(s.diskTotal - s.diskFree, s.diskTotal))} used` : '—';
            donut.rebuild(); bars.rebuild();
        }};
    }
    page(name) {
        this.active = name;
        if (!this.pages.has(name)) {
            const view = this[name.toLowerCase()]();
            const toolbar = this.toolbar(name.toLowerCase());
            const body = new Column({text: 'panel:fixed:flush:gap=12', state: flat}, view.children);
            const content = new Column({text: 'panel:flush:gap=8', state: flat | State.EXPANSIVE}, [
                toolbar.root,
                new ScrollView({name: `monitor-${name.toLowerCase()}-scroll`, state: flat | State.EXPANSIVE}, [body]),
            ]);
            const surface = new SceneSurface({state: flat | State.EXPANSIVE});
            const root = new Stack({text: 'panel:flush', state: flat | State.EXPANSIVE}, [surface,
                new SceneOverlay({surface, bounds: {x: 0, y: 0, width: 1, height: 1}, state: flat}, [content])]);
            let width = 1100, height = 800;
            const paint = () => {
                const n = parseInt(this.palette.background.slice(1), 16), packed = (0xff000000 | ((n & 255) << 16) | (n & 0xff00) | (n >>> 16)) >>> 0;
                surface.submit(new SceneBuilder(width, height).rect([0, 0, width, height], packed).finish());
            };
            surface.on(Message.SIZE_CHANGED, event => {
                width = event.detail?.width || width; height = event.detail?.height || height;
                // Native rows stack on compact windows; the scroll viewport
                // keeps every chart and control reachable at any window size.
                for (const group of body.children) group.setCompact?.(width < (group.compactAt || 740));
                paint();
            });
            paint();
            this.pages.set(name, {root, paint, update: () => {toolbar.update(); view.update();}});
        }
        this.update(true);
        return this.pages.get(name).root;
    }
    update(force = false) {
        const page = this.pages.get(this.active);
        if (page && (!this.paused || force || this.error)) page.update();
    }
    detach() {
        this.active = null;
        for (const {root} of this.pages.values()) root.parent?.remove(root);
    }
    destroy() {
        this.stopTheme();
        for (const cleanup of this.cleanups) cleanup();
        for (const {root} of this.pages.values()) root.destroy();
    }
}
