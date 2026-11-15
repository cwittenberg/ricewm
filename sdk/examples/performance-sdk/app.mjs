import {
    Button, CheckBox, Column, Dialog, Label, Menu, MenuBar, MenuItem,
    Message, Row, ScrollView, State, Table, Tabs, TextField, defineApp,
} from "@ricewm/app";
import { PerformanceDashboard } from "./dashboard.mjs";
import {recordingView} from "./profiler-report.mjs";

const enabled = State.ENABLED;
const bytes = value => {
    const units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let unit = 0;
    let scaled = Number(value) || 0;
    while (scaled >= 1024 && unit + 1 < units.length) {
        scaled /= 1024;
        unit += 1;
    }
    return unit === 0 ? `${Math.round(scaled)} B` : `${scaled.toFixed(1)} ${units[unit]}`;
};

export function parseSnapshot(source) {
    const lines = String(source || "").split("\n").filter(Boolean);
    const header = (lines.shift() || "").split("\t");
    if (header[0] !== "S") throw new Error("RiceWM returned an invalid performance snapshot");
    const offset = header[1] === "1" ? 2 : 3;
    const fields = header.slice(offset);
    const snapshot = {
        cpu: Math.round(Number(fields[0]) || 0),
        cpuScope: 'Guest process CPU',
        memoryUsed: Number(fields[1]) || 0,
        memoryTotal: Number(fields[2]) || 0,
        diskFree: header[1] === "7" ? Number(fields[3]) || 0 : 0,
        diskTotal: header[1] === "7" ? Number(fields[4]) || 0 : 0,
        received: Number(fields[5]) || 0,
        sent: Number(fields[6]) || 0,
        receivedTotal: Number(fields[7]) || 0,
        sentTotal: Number(fields[8]) || 0,
        ipv4: fields[9] || "Unavailable",
        networkMode: fields[10] || "offline",
        load: fields[11] || "0.00",
        sockets: Number(fields[12]) || 0,
        gpu: null,
        gpuBackend: "unavailable",
        gpuReason: "unavailable",
        storageBreakdown: [],
        processes: [],
    };
    const executionByPid = new Map();
    for (const line of lines) {
        const field = line.split("\t");
        if (field[0] === 'V' && field[1] === '1' && field.length === 3) { snapshot.cpuScope = field[2]; continue; }
        if (field[0] === "U" && field.length === 4) {
            snapshot.gpu = field[1] === "" ? null : Math.round(Number(field[1]) || 0);
            snapshot.gpuBackend = field[2] || "unavailable";
            snapshot.gpuReason = field[3] || "unavailable";
            continue;
        }
        if (field[0] === "B" && field.length === 3) {
            snapshot.storageBreakdown.push([field[1], Number(field[2]) || 0]);
            continue;
        }
        if (field[0] === "N") {
            const versioned = field.length >= 30 &&
                ["2", "3", "4", "5", "6", "7"].includes(field[1]);
            const pid = Number(field[versioned ? 2 : 1]) || 0;
            const event = field[versioned ? 4 : 2] || "";
            const ordinal = versioned ? Number(field[3]) || 0 : 0;
            const previous = executionByPid.get(pid);
            if (pid && (!previous || ordinal >= previous.ordinal)) {
                executionByPid.set(pid, {
                    ordinal,
                    accelerated: !field[versioned ? 28 : 14] && [
                        "native-wasm-exec", "native-wasm-first-output",
                        "native-wasm-profile-mark", "native-wasm-detached-process-start",
                        "native-wasm-detached-painted", "native-wasm-detached-scene",
                    ].includes(event),
                });
            }
            continue;
        }
        // The first eleven fields are stable. Newer hosts append the live
        // execution backend; extensions must not make the process disappear.
        if (field[0] !== "P" || field.length < 11) continue;
        snapshot.processes.push({
            pid: Number(field[1]) || 0,
            parent: Number(field[2]) || 0,
            state: field[3],
            nice: Number(field[4]) || 0,
            cpuMs: Number(field[5]) || 0,
            cpu: field[6] === '' ? null : Math.round(Number(field[6]) || 0),
            memory: Number(field[7]) || 0,
            threads: Number(field[8]) || 0,
            user: field[9],
            command: field[10],
            executionBackend: field[11],
        });
    }
    for (const process of snapshot.processes) {
        process.accelerated = !["Z", "X"].includes(process.state) && (
            process.executionBackend !== undefined
                ? process.executionBackend === "native-wasm"
                : executionByPid.get(process.pid)?.accelerated === true
        );
    }
    return snapshot;
}

export default defineApp({
    manifest: { id: "dev.ricewm.performance-sdk", name: "System Monitor", version: "3.0.3" },
    async start({ app, services, log, createWindow }) {
        let snapshot = {
            cpu: 0, memoryUsed: 0, memoryTotal: 1, diskFree: 0, diskTotal: 0,
            received: 0, sent: 0, receivedTotal: 0, sentTotal: 0,
            ipv4: "Unavailable", networkMode: "offline", load: "0.00", sockets: 0,
            gpu: null, gpuBackend: "unavailable", gpuReason: "unavailable",
            storageBreakdown: [], processes: [],
        };
        let tab = "Overview";
        let renderedTab = null;
        const dashboard = new PerformanceDashboard({app, services, log});
        let filter = "";
        let acceleratedOnly = false;
        let searchFocused = false;
        let deferredSearchRender = null;
        let selectedPid = null;
        let confirmationPid = null;
        let processSort = "cpu";
        let processSortAscending = false;
        // One shared table model keeps every header and body cell aligned.
        // Widths are relative, so resizing the window preserves the layout;
        // dragging a header divider updates the matching body column too.
        let processColumnWidths = [380, 103, 103, 103, 103, 103, 105];
        // Each process row delegates its entire hit area to one primary button.
        // The binding is installed after attachment, when RiceUI has assigned
        // stable control IDs. This makes whitespace and every column select the
        // same row, matching native Win32-style report-list behaviour.
        let processRowBindings = [];
        let processActionBindings = [];
        let tabBindings = [];
        let pointerInteractionActive = false;
        let stopped = false;
        let timer;
        let window;
        let recordingPanel = null;
        let recordingLoad = 0;
        const activate = (control, action) => {
            // A telemetry refresh must never replace an armed control between
            // its physical press and release. The retained host quite rightly
            // requires both halves of an activation to belong to the same
            // widget, so keep that widget alive for the whole gesture.
            control.on(Message.POINTER, event => {
                if (event.detail?.kind === "down") pointerInteractionActive = true;
                else if (["up", "leave"].includes(event.detail?.kind)) {
                    pointerInteractionActive = false;
                }
            });
            control.on(Message.ACTIVATE, event => {
                pointerInteractionActive = false;
                action(event);
            });
            return control;
        };
        let refreshing = false;
        const refresh = async () => {
            if (stopped || refreshing || recordingPanel) return;
            refreshing = true;
            try {
                snapshot = parseSnapshot(await services.system.performance());
                if (stopped || recordingPanel) return;
                dashboard.record(snapshot);
                if (!snapshot.processes.some(process => process.pid === selectedPid)) selectedPid = null;
                if (!searchFocused && !pointerInteractionActive) render();
            } catch (error) {
                dashboard.failed();
                log.warn("refresh failed", { error: String(error?.message || error) });
            } finally { refreshing = false; }
        };
        const tabButton = (name, icon) => {
            const selected = tab === name;
            const button = activate(new Button({
                name: `monitor-tab-${name.toLowerCase()}`, text: name, icon,
                documentFont: {family: 'sans-serif', size_pt: 11, weight: 400, italic: false, underline: false},
                documentPaint: {foreground: dashboard.palette.foreground, background: dashboard.palette.panel},
                state: enabled | State.FLAT | (selected ? State.SELECTED : 0),
                selected,
            }), () => {
                tab = name;
                render();
            });
            const composite = new Row({
                state: enabled | (selected ? State.SELECTED : 0),
                selected,
            }, [button]);
            button.monitorIcon = icon;
            tabBindings.push([composite, button]);
            return composite;
        };
        const selectProcess = (process, rebuild = true) => {
            selectedPid = process.pid;
            if (rebuild) {
                render();
                return;
            }
            // Context selection happens on secondary-button down. Patch the
            // existing controls in place so the same retained tree receives
            // secondary-button up and can open its contextual MenuBar.
            for (const binding of processRowBindings) {
                const selected = binding.process.pid === process.pid;
                binding.row.setProperty("selected", selected);
                binding.selector.setProperty("selected", selected);
            }
            const actionsEnabled = true;
            for (const control of processActionBindings) {
                control.setProperty("enabled", actionsEnabled);
            }
        };
        const processContent = () => {
            const processes = snapshot.processes
                .filter(process => (!acceleratedOnly || process.accelerated) &&
                    (!filter || `${process.command} ${process.pid} ${process.user}`.toLowerCase().includes(filter.toLowerCase())))
                .sort((left, right) => {
                    const compare = {
                        command: () => left.command.localeCompare(right.command),
                        pid: () => left.pid - right.pid,
                        user: () => left.user.localeCompare(right.user),
                        cpu: () => left.cpu - right.cpu,
                        memory: () => left.memory - right.memory,
                        nice: () => left.nice - right.nice,
                        state: () => left.state.localeCompare(right.state),
                    }[processSort];
                    const order = compare() || left.pid - right.pid;
                    return processSortAscending ? order : -order;
                })
                .slice(0, 200);
            const search = new TextField({
                value: filter,
                accessibleName: "Search processes",
                tooltip: "Search by process name, PID, or user",
                icon: 30,
                state: enabled | State.EXPANSIVE,
            });
            search.on(Message.VALUE_CHANGED, event => {
                filter = String(event.detail?.value ?? search.getProperty("value"));
                clearTimeout(deferredSearchRender);
                deferredSearchRender = setTimeout(() => {
                    searchFocused = false;
                    render();
                }, 350);
            });
            search.on(Message.FOCUS, () => {
                searchFocused = true;
            });
            search.on(Message.BLUR, () => {
                searchFocused = false;
                clearTimeout(deferredSearchRender);
                deferredSearchRender = setTimeout(render, 0);
            });
            search.on(Message.KEY, event => {
                const keyCode = Number(event.detail?.keyCode);
                if (event.detail?.pressed &&
                    (event.detail?.key === "Enter" || [13, 36, 104].includes(keyCode))) {
                    searchFocused = false;
                    render();
                }
            });
            const accelerated = activate(new CheckBox({
                text: "Show Accelerated only",
                state: enabled | (acceleratedOnly ? State.CHECKED : 0),
                checked: acceleratedOnly,
            }), () => {
                acceleratedOnly = !acceleratedOnly;
                render();
            });
            const header = (key, label, start = false) => activate(new Button({
                text: processSort === key ? `${label} ${processSortAscending ? "↑" : "↓"}` : label,
                state: enabled | State.EMPHASIZED | (start ? State.START_ALIGNED : 0),
            }), () => {
                if (processSort === key) processSortAscending = !processSortAscending;
                else {
                    processSort = key;
                    processSortAscending = ["command", "pid", "user", "state"].includes(key);
                }
                render();
            });
            const headerRow = new Row({ text: "split:380:file-detail-header" }, [
                header("command", "Process", true),
                header("pid", "PID"), header("user", "User"), header("cpu", "CPU"),
                header("memory", "Memory"), header("nice", "Nice"), header("state", "State"),
            ]);
            const rows = processes.map(process => {
                const selected = selectedPid === process.pid;
                const state = enabled | State.FLAT | (selected ? State.SELECTED : 0);
                const select = () => selectProcess(process);
                const selector = activate(new Button({
                    text: `${process.accelerated ? "⚡ " : ""}${process.command}`.slice(0, 48),
                    state: state | State.START_ALIGNED,
                    selected,
                }), select);
                selector.on(Message.POINTER, event => {
                    if (event.detail?.button === 3 && event.detail?.kind === "down") {
                        selectProcess(process, false);
                    }
                });
                selector.on(Message.KEY, event => {
                    const keyCode = Number(event.detail?.keyCode);
                    if (event.detail?.pressed &&
                        (keyCode === 46 || keyCode === 119 || event.detail?.key === "Delete")) {
                        void endProcess(process.pid);
                    }
                });
                const cell = (text, extra = 0) => new Label({ text, state: enabled | extra });
                const row = new Row({ state, selected }, [
                    selector,
                    cell(String(process.pid)), cell(process.user.slice(0, 14)), cell(process.cpu === null ? '—' : `${process.cpu}%`),
                    cell(bytes(process.memory)), cell(String(process.nice)), cell(process.state),
                ]);
                processRowBindings.push({ process, row, selector });
                return row;
            });
            const bodyTable = new Table({ columnWidths: processColumnWidths }, rows);
            const headerTable = new Table({
                columnWidths: processColumnWidths,
                resizable: true,
            }, [headerRow]);
            headerTable.on(Message.POINTER, () => {
                processColumnWidths = [...headerTable.getProperty("columnWidths")];
                bodyTable.setProperty("columnWidths", processColumnWidths);
            });
            const selected = snapshot.processes.find(process => process.pid === selectedPid);
            const actionsEnabled = Boolean(selected);
            const content = [
                new Row({ text: "split:480" }, [
                    new Label({
                        text: `${processes.length} of ${snapshot.processes.length} processes  ·  CPU ${snapshot.cpu}%  ·  Memory ${bytes(snapshot.memoryUsed)} of ${bytes(snapshot.memoryTotal)}`,
                        state: enabled | State.START_ALIGNED,
                    }),
                    new Row({ text: "split:500" }, [
                        accelerated,
                        new Row({ text: "split:260" }, [
                            new Label({ text: "Search", state: enabled | State.EMPHASIZED }),
                            search,
                        ]),
                    ]),
                ]),
                new Row({}, [headerTable]),
                new ScrollView({ text: `0/${processes.length * 40}` }, [bodyTable]),
            ];
            if (selected) content.push(new Label({
                text: `PID ${selected.pid}  ·  parent ${selected.parent}  ·  ${selected.threads} thread${selected.threads === 1 ? "" : "s"}  ·  user ${selected.user}  ·  nice ${selected.nice}`,
                state: enabled | State.START_ALIGNED,
            }));
            const improveButton = activate(new Button({
                text: "Improve", icon: 73, enabled: actionsEnabled,
                state: actionsEnabled ? enabled : 0,
            }), () => void changeNice(-1));
            const decreaseButton = activate(new Button({
                text: "Decrease", icon: 74, enabled: actionsEnabled,
                state: actionsEnabled ? enabled : 0,
            }), () => void changeNice(1));
            const endButton = activate(new Button({
                    text: "End process…", icon: 25, enabled: actionsEnabled, destructive: true,
                    state: (actionsEnabled ? enabled : 0) | State.DESTRUCTIVE,
                }), () => {
                    if (!selectedPid) return;
                    confirmationPid = selectedPid;
                    render();
                });
            processActionBindings.push(improveButton, decreaseButton, endButton);
            content.push(new Row({ text: "panel:actions" }, [
                activate(new Button({ text: "Refresh", icon: 34 }), () => void refresh()),
                improveButton,
                decreaseButton,
                endButton,
            ]));
            const endMenuItem = activate(new MenuItem({
                text: "End process", icon: 25,
                enabled: actionsEnabled,
                state: (actionsEnabled ? enabled : 0) | State.DESTRUCTIVE,
            }), () => selectedPid && void endProcess(selectedPid));
            const improveMenuItem = activate(new MenuItem({
                text: "Improve priority", icon: 73,
                enabled: actionsEnabled,
                state: actionsEnabled ? enabled : 0,
            }), () => void changeNice(-1));
            const decreaseMenuItem = activate(new MenuItem({
                text: "Decrease priority", icon: 74,
                enabled: actionsEnabled,
                state: actionsEnabled ? enabled : 0,
            }), () => void changeNice(1));
            processActionBindings.push(endMenuItem, improveMenuItem, decreaseMenuItem);
            content.push(new MenuBar({ text: "context" }, [
                new Menu({ text: "Process" }, [
                    endMenuItem,
                    improveMenuItem,
                    decreaseMenuItem,
                ]),
            ]));
            return content;
        };
        const build = () => {
            tabBindings = [];
            const tabs = [
                tabButton("Overview", 40),
                tabButton("Processes", 79),
                tabButton("Resources", 40),
                tabButton("Network", 65),
                tabButton("Storage", 64),
                new Column({
                    text: "panel:tab-page",
                    state: enabled | State.EXPANSIVE,
                }, tab === "Processes" ? processContent() : [dashboard.page(tab)]),
            ];
            const children = [new Tabs({ text: "panel:tabs:integrated" }, tabs)];
            const process = snapshot.processes.find(candidate => candidate.pid === confirmationPid);
            if (process) children.push(new Dialog({ text: "End process?" }, [
                new Label({ text: `Send SIGTERM to ${process.command} (PID ${process.pid})? Unsaved work in this process may be lost.` }),
                new Row({ text: "panel:actions" }, [
                    activate(new Button({ text: "Cancel" }), () => {
                        confirmationPid = null;
                        render();
                    }),
                    activate(new Button({ text: "End process", state: enabled | State.DESTRUCTIVE | State.ACCENT }), () => void endProcess(process.pid)),
                ]),
            ]));
            return new Column({}, children);
        };
        async function changeNice(delta) {
            const process = snapshot.processes.find(candidate => candidate.pid === selectedPid);
            if (!process) return;
            const nice = Math.max(-20, Math.min(19, process.nice + delta));
            try {
                await services.system.processNice({ pid: process.pid, nice });
                await refresh();
            } catch (error) {
                log.error("renice failed", { pid: process.pid, error: String(error?.message || error) });
            }
        }
        async function endProcess(pid) {
            confirmationPid = null;
            try {
                await services.system.processSignal({ pid, signal: 15 });
                selectedPid = null;
                await refresh();
            } catch (error) {
                log.error("SIGTERM failed", { pid, error: String(error?.message || error) });
                render();
            }
        }
        function syncTabIcons() {
            const compact = (window?.state?.contentWidth || 1100) < 900;
            for (const [, button] of tabBindings) button.icon = compact ? null : button.monitorIcon;
        }
        function render() {
            if (tab !== "Processes" && renderedTab === tab) return;
            dashboard.detach();
            const previous = window.root;
            processRowBindings = [];
            processActionBindings = [];
            window.setRoot(build());
            renderedTab = tab;
            syncTabIcons();
            previous.destroy();
            // `control-for` is a renderer-level composite-control contract. It
            // turns the Row into a full-width proxy for the primary selector.
            for (const { row, selector } of processRowBindings) {
                row.text = `panel:file-detail-row:control-for=${selector.id}`;
            }
            for (const [composite, button] of tabBindings) {
                composite.text = `panel:tab:control-for=${button.id}`;
            }
        }
        processRowBindings = [];
        processActionBindings = [];
        window = createWindow({ title: "System Monitor", root: build() });
        renderedTab = tab;
        window.onState(syncTabIcons);
        window.onState(state => recordingPanel?.resize?.(state.contentWidth || 1100));
        app.on(Message.APP_OPEN, async ({paths = []} = {}) => {
            const path=paths.find(value=>String(value).endsWith('.riceprofile'));
            if(!path)return;
            const load=++recordingLoad;
            recordingPanel?.destroy?.();
            recordingPanel={destroy(){}};
            dashboard.detach();
            const old=window.root;
            window.title='Recording overview';
            window.setRoot(new Column({text:'panel:padding=24:gap=12'},[
                new Label({text:'Opening recording…',state:State.EMPHASIZED})]));
            old.destroy();
            const returnToLive=()=>{
                ++recordingLoad;recordingPanel?.destroy?.();recordingPanel=null;
                window.title='System Monitor';renderedTab=null;render();void refresh();
            };
            try{
                const bytes=await services.files.read({path});
                if(bytes.byteLength>4*1024*1024)throw Error('Recording exceeds the supported size.');
                const report=JSON.parse(new TextDecoder().decode(bytes));
                const panel=await recordingView({report,app,services,onLive:returnToLive});
                if(stopped||load!==recordingLoad){panel.destroy();return;}
                recordingPanel=panel;
                const loading=window.root;window.setRoot(panel.root);loading.destroy();
                panel.resize(window.state.contentWidth||1100);
            }catch(error){
                if(stopped||load!==recordingLoad)return;
                const back=new Button({text:'Live monitor'});back.on(Message.ACTIVATE,returnToLive);
                const loading=window.root;
                window.setRoot(new Column({text:'panel:padding=24:gap=12'},[
                    new Label({text:'Could not open recording',state:State.EMPHASIZED}),
                    new Label({text:String(error?.message||error)}),back]));loading.destroy();
                log.error('Recording could not open',{error:String(error)});
            }
        });
        app.on(Message.THEME_CHANGED, () => {
            for (const [, button] of tabBindings) button.setProperty('documentPaint', {
                foreground: dashboard.palette.foreground, background: dashboard.palette.panel,
            });
        });
        for (const { row, selector } of processRowBindings) {
            row.text = `panel:file-detail-row:control-for=${selector.id}`;
        }
        for (const [composite, button] of tabBindings) {
            composite.text = `panel:tab:control-for=${button.id}`;
        }
        app.on(Message.APP_CLOSE, () => {
            stopped = true;
            clearTimeout(deferredSearchRender);
            clearInterval(timer);
            recordingPanel?.destroy?.();
            dashboard.destroy();
        });
        await Promise.all([...dashboard.charts].map(chart => chart.ready));
        await refresh();
        if (!stopped) timer = setInterval(refresh, 2_000);
    },
});
