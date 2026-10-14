import {t} from './rice-i18n.mjs';
import {languages} from './rice-i18n-catalog.mjs';
import {planOutputIdentities,validOutputIdentitySlots} from './rice-output-identities.mjs';
// Only canonical built-ins and explicitly marked category labels are owned UI.
// The caller's descriptors remain unchanged for launching and folder operations.
export function localizedMenuApplication(application) {
    const builtin = globalThis.RiceNativeApplicationCatalog?.find(candidate =>
        candidate.path === application.path && candidate.label === application.label);
    return {
        ...application,
        label: builtin ? t(application.label) : application.label,
        menuPath: application.menuPath?.map((part, index) => {
            const owned = builtin || application.menuPathMessages?.[index] === part;
            return owned ? t(part) : part;
        }),
    };
}

function dockApplicationKey(identity) {
    if (!identity) return 0;
    let hash = 2166136261;
    for (const char of String(identity)) hash = Math.imul(hash ^ char.charCodeAt(0),16777619) >>> 0;
    return hash || 1;
}

const RESULT_ERROR = 0xffffffff;
const MAX_SURFACE_UPLOAD_BYTES = 4 * 1024 * 1024;
// Hosted synchronous consumers can use the full Wasm transfer arena. A 1080p
// frame then stays one borrowed upload instead of two independently copied ones.
const MAX_BORROWED_SURFACE_UPLOAD_BYTES = 16 * 1024 * 1024;
const MAX_DAMAGE_REGIONS_PER_DRAWABLE = 64;
const DISPLAY_CHECKPOINT_SCHEMA = "rice.display-core.checkpoint/v1";
const MAX_DISPLAY_CHECKPOINT_BYTES = 256 * 1024 * 1024;

function addDamageRegion(regions, rectangle) {
    let next = rectangle;
    for (let index = 0; index < regions.length;) {
        const previous = regions[index];
        const union = {
            left: Math.min(previous.left, next.left),
            top: Math.min(previous.top, next.top),
            right: Math.max(previous.right, next.right),
            bottom: Math.max(previous.bottom, next.bottom),
        };
        const unionArea = (union.right - union.left) * (union.bottom - union.top);
        const previousArea = (previous.right - previous.left) * (previous.bottom - previous.top);
        const nextArea = (next.right - next.left) * (next.bottom - next.top);
        if (unionArea <= previousArea + nextArea) {
            next = union;
            regions.splice(index, 1);
            index = 0;
        } else {
            index += 1;
        }
    }
    if (regions.length >= MAX_DAMAGE_REGIONS_PER_DRAWABLE) {
        for (const previous of regions) {
            next = {
                left: Math.min(previous.left, next.left),
                top: Math.min(previous.top, next.top),
                right: Math.max(previous.right, next.right),
                bottom: Math.max(previous.bottom, next.bottom),
            };
        }
        regions.length = 0;
    }
    regions.push(next);
}

export class RiceDisplayCore {
    static async instantiate(source, imports = {}) {
        const result = await WebAssembly.instantiate(source, imports);
        const instance = result instanceof WebAssembly.Instance
            ? result
            : result.instance;
        return new RiceDisplayCore(instance);
    }

    constructor(instance) {
        this.instance = instance;
        const rawExports = instance.exports;
        // X11 timestamps are elapsed milliseconds, not event sequence numbers.
        // Refresh on every ABI operation, including client requests which emit
        // property, selection and grab events without browser pointer input.
        const timestamp = rawExports.rice_timestamp_millis;
        this.exports = typeof timestamp === "function"
            ? Object.fromEntries(Object.entries(rawExports).map(([name, value]) =>
                [name, typeof value === "function" && name !== "rice_timestamp_millis"
                    ? new Proxy(value, {
                        apply(target, receiver, args) {
                            timestamp(Math.floor(performance.timeOrigin + performance.now()) >>> 0);
                            return Reflect.apply(target, receiver, args);
                        },
                    })
                    : value]))
            : rawExports;
        this.memory = this.exports.memory;
        if (!(this.memory instanceof WebAssembly.Memory)) {
            throw new TypeError("rice-display.wasm does not export memory");
        }
        if (this.exports.rice_abi_version() !== 2) {
            throw new Error("unsupported RiceWM display ABI");
        }
    }

    initialize(width, height) {
        if (this.exports.rice_init(width, height) !== 1) {
            throw new RangeError("invalid RiceWM display geometry");
        }
        this.width = Number(width);
        this.height = Number(height);
        this.outputs=[];this.outputIdentitySlots=null;
    }

    resize(width, height) {
        if (this.exports.rice_resize(width, height) !== 1) {
            throw new RangeError("invalid RiceWM display resize");
        }
        this.width = Number(width);
        this.height = Number(height);
    }

    setOutputLayout(width, height, outputs) {
        if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 ||
            width > 65535 || height > 65535 || !Array.isArray(outputs) || outputs.length > 16 ||
            outputs.some(r => ![r.x,r.y,r.width,r.height].every(Number.isInteger) ||
                r.x < 0 || r.y < 0 || r.x > 32767 || r.y > 32767 || r.width < 1 || r.height < 1 ||
                r.x+r.width > width || r.y+r.height > height)) {
            throw new RangeError("invalid RiceWM output layout");
        }
        const previous=this.outputs||[];
        const uniqueIds=values=>values.every(o=>typeof o.id==='string'&&o.id.length>0&&o.id.length<=128)&&new Set(values.map(o=>o.id)).size===values.length;
        if(this.exports.rice_output_layout_map&&outputs.length&&uniqueIds(outputs)){
            const slots=this.outputIdentitySlots||Array.from({length:16},(_,index)=>uniqueIds(previous)?previous[index]?.id??null:null);
            const plan=planOutputIdentities(slots,outputs.map(output=>output.id));
            const pointer=this.#reserve(68+outputs.length*8),view=new DataView(this.memory.buffer,pointer,68+outputs.length*8);
            plan.oldToNew.forEach((value,index)=>view.setUint32(index*4,value,true));view.setUint32(64,plan.resetMask,true);
            outputs.forEach((output,index)=>['x','y','width','height'].forEach((key,field)=>view.setUint16(68+index*8+field*2,output[key],true)));
            if(this.exports.rice_output_layout_map(width,height,outputs.length)!==1)throw new RangeError('RiceWM display rejected output identity mapping');
            this.width=width;this.height=height;this.outputs=outputs.map(output=>({...output}));this.outputIdentitySlots=plan.slots;return;
        }
        const permutation=outputs.map(output=>previous.findIndex(old=>old.id===output.id));
        const reorder=Boolean(this.exports.rice_output_layout_reorder)&&outputs.length>0&&previous.length===outputs.length&&
            width===this.width&&height===this.height&&uniqueIds(previous)&&uniqueIds(outputs)&&
            permutation.every((old,index)=>old>=0&&['x','y','width','height'].every(key=>previous[old][key]===outputs[index][key]));
        const subset=Boolean(this.exports.rice_output_layout_subset)&&outputs.length>0&&outputs.length<previous.length&&
            uniqueIds(previous)&&uniqueIds(outputs)&&permutation.every(old=>old>=0);
        const mapped=reorder||subset;
        const stride=mapped?12:8;
        const pointer = this.#reserve(Math.max(1, outputs.length * stride));
        const view = new DataView(this.memory.buffer, pointer, outputs.length * stride);
        outputs.forEach((r,i) => {
            [r.x,r.y,r.width,r.height].forEach((value,j) => view.setUint16(i*stride+j*2,value,true));
            if(mapped)view.setUint32(i*stride+8,permutation[i],true);
        });
        const accepted=reorder?this.exports.rice_output_layout_reorder(width,height,outputs.length):subset?this.exports.rice_output_layout_subset(width,height,outputs.length):this.exports.rice_output_layout?.(width,height,outputs.length);
        if (accepted !== 1) {
            throw new RangeError("RiceWM display does not support this output layout");
        }
        this.outputIdentitySlots=null;
        this.width=width;this.height=height;
        this.outputs=outputs.map(output=>({...output}));
    }

    captureCheckpoint() {
        const source = new Uint8Array(this.memory.buffer);
        if (source.byteLength === 0 ||
            source.byteLength > MAX_DISPLAY_CHECKPOINT_BYTES) {
            throw new RangeError("RiceWM display memory exceeds its checkpoint bound");
        }
        return Object.freeze({
            schema: DISPLAY_CHECKPOINT_SCHEMA,
            abi: this.exports.rice_abi_version() >>> 0,
            width: Number(this.width),
            height: Number(this.height),
            memory: source.slice(),
            outputs:(this.outputs||[]).map(output=>({...output})),
            outputIdentitySlots:this.outputIdentitySlots?[...this.outputIdentitySlots]:null,
        });
    }

    restoreCheckpoint(snapshot) {
        if (
            snapshot?.schema !== DISPLAY_CHECKPOINT_SCHEMA ||
            snapshot.abi !== (this.exports.rice_abi_version() >>> 0) ||
            !(snapshot.memory instanceof Uint8Array) ||
            snapshot.memory.byteLength === 0 ||
            snapshot.memory.byteLength > MAX_DISPLAY_CHECKPOINT_BYTES ||
            !Number.isSafeInteger(snapshot.width) || snapshot.width < 1 ||
            !Number.isSafeInteger(snapshot.height) || snapshot.height < 1
        ) throw new TypeError("RiceWM display checkpoint is invalid");
        if(snapshot.outputIdentitySlots!=null&&!validOutputIdentitySlots(snapshot.outputIdentitySlots))throw new TypeError('RiceWM checkpoint output identities are invalid');
        const outputs=snapshot.outputs||[];
        if(!Array.isArray(outputs)||outputs.length>16||outputs.some(output=>!output||!['x','y','width','height'].every(key=>Number.isSafeInteger(output[key]))||output.x<0||output.y<0||output.width<1||output.height<1||output.x+output.width>snapshot.width||output.y+output.height>snapshot.height))throw new TypeError('RiceWM checkpoint outputs are invalid');
        if(snapshot.outputIdentitySlots&&outputs.some((output,index)=>output.id!==snapshot.outputIdentitySlots[index]))throw new TypeError('RiceWM checkpoint output identities do not match its topology');
        if (snapshot.memory.byteLength > this.memory.buffer.byteLength) {
            const pages = Math.ceil(
                (snapshot.memory.byteLength - this.memory.buffer.byteLength) /
                    65_536,
            );
            this.memory.grow(pages);
        }
        const target = new Uint8Array(this.memory.buffer);
        target.fill(0);
        target.set(snapshot.memory);
        this.width = snapshot.width;
        this.height = snapshot.height;
        this.outputs=outputs.map(output=>({...output}));
        this.outputIdentitySlots=snapshot.outputIdentitySlots?[...snapshot.outputIdentitySlots]:null;
        return true;
    }

    openClient() {
        const client = this.exports.rice_client_open() >>> 0;
        if (client === 0) throw new Error("RiceWM client limit reached");
        return client;
    }

    closeClient(client) {
        return this.exports.rice_client_close(client) === 1;
    }

    pointerCursor() {
        return ['default', 'pointer', 'text', 'ew-resize', 'ns-resize',
            'nwse-resize', 'nesw-resize', 'move', 'crosshair', 'wait',
            'progress', 'not-allowed', 'grab', 'grabbing', 'help'][
            this.exports.rice_input_pointer_cursor?.() ?? 0] || 'default';
    }

    pointerTarget(x, y) {
        const target = this.exports.rice_input_pointer_target(x, y) >>> 0;
        if (target === 1) return "root";
        if (target === 2) return "window";
        if (target === 3) return "shell";
        if (target === 4) return "managed-frame";
        throw new RangeError("invalid RiceWM pointer target");
    }

    setNativeFullscreenRegions(regions) {
        if (!Array.isArray(regions) || regions.length > 16) throw new RangeError('Native fullscreen output limit exceeded');
        const records = regions.map(region => [region.x, region.y, region.width, region.height]);
        if (records.some(values => values.some(value => !Number.isSafeInteger(value) || value < 0 || value > 65535)
            || values[2] === 0 || values[3] === 0)) throw new RangeError('Invalid native fullscreen output');
        const update = this.exports.rice_wm_native_fullscreen_regions;
        if (typeof update !== 'function') {
            if (records.length) throw new Error('Native fullscreen requires an updated display runtime');
            return false;
        }
        const bytes = new Uint8Array(records.length * 8), view = new DataView(bytes.buffer);
        records.forEach((values, index) => values.forEach((value, field) => view.setUint16(index * 8 + field * 2, value, true)));
        if (bytes.length) {
            const pointer = this.#reserve(bytes.length);
            new Uint8Array(this.memory.buffer, pointer, bytes.length).set(bytes);
        }
        if (update(records.length) !== 1) throw new Error('Native fullscreen outputs were rejected');
        return true;
    }

    get supportsNativeFullscreen() { return typeof this.exports.rice_wm_native_fullscreen_regions === 'function'; }

    selectShellOutput(x, y) {
        return this.exports.rice_wm_shell_output_select?.(x, y) === 1;
    }

    pointerMotion(x, y, state = 0) {
        if (this.exports.rice_input_pointer(0, 0, x, y, state) !== 1) {
            throw new RangeError("invalid RiceWM pointer motion");
        }
    }

    relativePointerCapture() {
        return this.exports.rice_input_relative_capture?.() === 1;
    }

    pointerRelative(x, y, dx, dy, state = 0) {
        if (this.exports.rice_input_pointer_relative?.(x, y, dx, dy, state) !== 1) {
            throw new RangeError("invalid RiceWM relative pointer motion");
        }
    }

    pointerButton(pressed, button, x, y, state = 0) {
        if (
            this.exports.rice_input_pointer(
                pressed ? 1 : 2,
                button,
                x,
                y,
                state,
            ) !== 1
        ) {
            throw new RangeError("invalid RiceWM pointer button event");
        }
    }

    key(pressed, keycode, state = 0) {
        if (
            this.exports.rice_input_key(
                pressed ? 1 : 0,
                keycode,
                state,
            ) !== 1
        ) {
            throw new RangeError("invalid RiceWM key event");
        }
    }

    setClock(hours, minutes) {
        if (this.exports.rice_wm_set_clock(hours, minutes) !== 1) {
            throw new RangeError("invalid RiceWM wall clock");
        }
    }

    setConnectivityMode(mode) {
        const modes = { offline: 0, hosted: 1, local: 2, proxy: 3 };
        const value = modes[String(mode)];
        if (
            value === undefined ||
            typeof this.exports.rice_wm_connectivity_set !== "function" ||
            this.exports.rice_wm_connectivity_set(value) !== 1
        ) {
            throw new RangeError("invalid RiceWM connectivity strategy");
        }
    }

    setLocale(locale) {
        const code = languages[locale]?.code ?? 0;
        if (typeof this.exports.rice_set_locale !== 'function') return false;
        if (this.exports.rice_set_locale(code) !== 1) throw new Error('Unable to set display language');
        if (this.menuApplications) this.setMenuApplications(this.menuApplications);
        return true;
    }

    setTopbarLayout(value = {version:1,tooltips:true,items:[]}) {
        const data = typeof value === "string" ? JSON.parse(value) : value;
        const source = (data.items || []).map(item => [item.key,item.visible ? 1 : 0,item.position,item.icon || ""].join("\t")).join("\n");
        this.setTopbarWire(0, source);
        this.canvasTopbarIcons = true;
    }

    setTopbarWire(kind, source) {
        if (typeof this.exports.rice_wm_topbar_update !== "function") return false;
        const bytes = new TextEncoder().encode(source);
        const pointer = this.#reserve(Math.max(1,bytes.length));
        new Uint8Array(this.memory.buffer,pointer,bytes.length).set(bytes);
        if(this.exports.rice_wm_topbar_update(kind,pointer,bytes.length)!==1) throw new Error("Invalid topbar layout");
        return true;
    }

    setSystemControls({enabled=true,ethernet=false,wifi=false,bluetooth=false,audio=false,muted=false,batteryPercentage=null,placement="top"}={}) {
        if(!["top","taskbar"].includes(placement))throw new RangeError("invalid system controls placement");
        const battery=batteryPercentage===null||batteryPercentage===undefined?255:Number(batteryPercentage);
        if(!Number.isInteger(battery)||battery<0||battery>255||(battery>100&&battery!==255))throw new RangeError("invalid battery percentage");
        if(typeof this.exports.rice_wm_system_battery_set!=="function"||
            this.exports.rice_wm_system_battery_set(battery)!==1) {
            throw new RangeError("RiceWM rejected the native battery state");
        }
        const value=Number(enabled)|Number(ethernet)<<1|Number(wifi)<<2|
            Number(bluetooth)<<3|Number(audio)<<4|Number(muted)<<5|Number(placement==="taskbar")<<6;
        if(typeof this.exports.rice_wm_system_controls_set!=="function"||
            this.exports.rice_wm_system_controls_set(value)!==1) {
            throw new RangeError("RiceWM rejected the native system controls state");
        }
    }

    setMailIndicator({ enabled = true, unread = 0 } = {}) {
        const count = Number(unread);
        if (!Number.isSafeInteger(count) || count < 0 || count > 999) {
            throw new RangeError("invalid Rice Box unread count");
        }
        if (
            typeof this.exports.rice_wm_mail_indicator_set !== "function" ||
            this.exports.rice_wm_mail_indicator_set(enabled ? count : 0xffff_ffff) !== 1
        ) {
            throw new RangeError("RiceWM rejected the Rice Box indicator state");
        }
    }

    setSessionLogout({enabled=false}={}) {
        const configure=this.exports.rice_wm_session_logout_set;
        if(typeof configure!=="function"||configure(enabled?1:0)!==1) {
            if(enabled)throw new Error("Hosted logout requires an updated display runtime");
        }
    }

    setGnomeOverviewElements({ dashVisible = true, workspacesVisible = true } = {}) {
        const configure = this.exports.rice_wm_gnome_overview_elements_set;
        if (typeof configure !== "function" || configure(
            dashVisible ? 1 : 0,
            workspacesVisible ? 1 : 0,
        ) !== 1) {
            throw new RangeError("RiceWM rejected GNOME overview element visibility");
        }
    }

    configureTaskbar({
        style = "Standard",
        height = "Small",
        magnification = true,
        magnificationZoom = 65,
        magnificationSpread = 45,
        shellStyle = "Standard",
        launcherVisible = true,
        iconColor = null,
        topBarAllMonitors = true,
        dockAllMonitors = false,
        visible,
        position,
        autoHide,
        previews = true,
        menuStructure = "Default",
        menuIconSize = "24 px",
        menuShowThemes = true,
        menuShowWorkspaces = true,
        launcherAppearance = "Icon and text",
        launcherText = "RiceWM",
        quickLaunchVisible = true,
        quickLaunchers = [],
        trayItems = [],
    }) {
        if (this.exports.rice_wm_shell_monitors_set && this.exports.rice_wm_shell_monitors_set(
            topBarAllMonitors ? 1 : 0, dockAllMonitors ? 1 : 0,
        ) !== 1) throw new Error("Monitor panel settings rejected");
        if (this.exports.rice_wm_dock_appearance_set) {
            const monochrome = Number.isInteger(iconColor) && iconColor >= 0 && iconColor <= 0xffffff;
            if (this.exports.rice_wm_dock_appearance_set(launcherVisible ? 1 : 0, monochrome ? 1 : 0, monochrome ? iconColor : 0) !== 1) throw new Error("Dock appearance rejected");
        } else if (!launcherVisible) throw new Error("Launcher visibility requires an updated display runtime");
        if (!["Standard", "Embedded"].includes(shellStyle)) throw new RangeError("Invalid shell style");
        if (this.exports.rice_wm_shell_style_set) {
            if (this.exports.rice_wm_shell_style_set(shellStyle === "Embedded" ? 1 : 0) !== 1) throw new Error("Shell style rejected");
        } else if (shellStyle !== "Standard") throw new Error("Embedded shell requires an updated display runtime");
        if (!["Standard", "Compact dock"].includes(style)) throw new RangeError("Invalid dock style");
        if (this.exports.rice_wm_dock_style_set) {
            if (this.exports.rice_wm_dock_style_set(style === "Compact dock" ? 1 : 0) !== 1) throw new Error("Dock style rejected");
        } else if (style !== "Standard") throw new Error("Dock style requires an updated display runtime");
        const zoom = Math.max(0, Math.min(100, Math.round(Number(magnificationZoom) || 0)));
        const heights = {Minimal:32, Small:44, Comfortable:64};
        if (!Object.hasOwn(heights, height)) throw new RangeError("Invalid dock height");
        if (this.exports.rice_wm_dock_height_set) {
            if (this.exports.rice_wm_dock_height_set(heights[height]) !== 1) throw new Error("Dock height rejected");
        } else if (height !== "Small") throw new Error("Dock height requires an updated display runtime");
        const spread = Math.max(0, Math.min(100, Math.round(Number(magnificationSpread) || 0)));
        if (typeof this.exports.rice_wm_dock_magnification_set === "function") {
            if (this.exports.rice_wm_dock_magnification_set(
                magnification ? 1 : 0,
                zoom,
                spread,
            ) !== 1) throw new Error("Dock magnification rejected");
        } else if (magnification && style === "Compact dock") {
            throw new Error("Dock magnification requires an updated display runtime");
        }
        const edges = { Bottom: 0, Top: 1, Left: 2, Right: 3 };
        if (!Object.hasOwn(edges, position)) {
            throw new RangeError(
                "RiceWM taskbar position must be Top, Bottom, Left, or Right",
            );
        }
        if (this.exports.rice_wm_taskbar_configure(
            visible ? 1 : 0,
            edges[position],
            autoHide ? 1 : 0,
        ) !== 1) {
            throw new RangeError("RiceWM rejected the taskbar configuration");
        }
        if (
            typeof this.exports.rice_wm_task_previews_set === "function" &&
            this.exports.rice_wm_task_previews_set(previews ? 1 : 0) !== 1
        ) {
            throw new RangeError("RiceWM rejected the task preview configuration");
        }
        const menuStructures = { Default: 0, "Applications only": 1 };
        if (
            menuStructures[menuStructure] === undefined ||
            typeof this.exports.rice_wm_menu_layout_configure !== "function" ||
            this.exports.rice_wm_menu_layout_configure(
                menuStructures[menuStructure],
                menuShowThemes ? 1 : 0,
                menuShowWorkspaces ? 1 : 0,
            ) !== 1
        ) {
            throw new RangeError("RiceWM rejected the menu layout configuration");
        }
        const iconSize = Number.parseInt(String(menuIconSize), 10);
        if (
            ![16, 24, 32, 64].includes(iconSize) ||
            typeof this.exports.rice_wm_menu_icon_size_set !== "function" ||
            this.exports.rice_wm_menu_icon_size_set(iconSize) !== 1
        ) {
            throw new RangeError("RiceWM rejected the menu icon size");
        }
        const appearances = { "Icon only": 0, "Icon and text": 1, "Text only": 2 };
        const appearance = appearances[launcherAppearance];
        const label = new TextEncoder().encode(String(launcherText));
        const pointer = this.#reserve(label.byteLength);
        new Uint8Array(this.memory.buffer, pointer, label.byteLength).set(label);
        if (
            appearance === undefined ||
            typeof this.exports.rice_wm_launcher_appearance_set !== "function" ||
            this.exports.rice_wm_launcher_appearance_set(
                appearance, pointer, label.byteLength,
            ) !== 1
        ) {
            throw new RangeError("RiceWM rejected the launcher appearance");
        }
        this.replaceQuickLaunchers(quickLaunchVisible, style === "Compact dock"
            ? quickLaunchers.map(({ iconName, ...launcher }) => launcher)
            : quickLaunchers);
        this.replaceTrayItems(trayItems);
    }

    replaceWorkspaceNames(names) {
        const values = Array.isArray(names) ? names.map(name => String(name).trim()) : [];
        if (values.length !== 4 || values.some(name => !name || [...name].length > 16)) {
            throw new RangeError("RiceWM requires four bounded workspace names");
        }
        const encoded = new TextEncoder().encode(values.join("\n"));
        const pointer = this.#reserve(encoded.byteLength);
        new Uint8Array(this.memory.buffer, pointer, encoded.byteLength).set(encoded);
        if (
            typeof this.exports.rice_wm_workspace_names_replace !== "function" ||
            this.exports.rice_wm_workspace_names_replace(pointer, encoded.byteLength) !== 1
        ) {
            throw new RangeError("RiceWM rejected the workspace names");
        }
    }

    keyboardGrabbed() { return this.exports.rice_wm_keyboard_grabbed?.()===1; }
    pointerGrabbed() {return this.exports.rice_wm_pointer_grabbed?.()===1;}
    tileInputBlocked() {return this.exports.rice_wm_tile_input_blocked?.()===1;}
    tileOverflow() {return this.exports.rice_wm_tile_overflow?.()===1;}
    hasTileMoveRequest() {return Boolean(this.exports.rice_wm_tile_move_request_field?.(0));}
    releaseTilePointer(x,y,state=0) {this.exports.rice_wm_tile_pointer_release?.(x,y,state);}
    takeTileMoveRequest() {
        const read=this.exports.rice_wm_tile_move_request_field;
        const id=read?.(0);if(!id)return null;
        const request={id,x:read(1)|0,y:read(2)|0,cancel:read(3)===1};
        this.exports.rice_wm_tile_move_request_clear();return request;
    }
    prepareWindowNavigation(direction, frames) {
        const index=['left','right','up','down'].indexOf(direction);
        if(index<0)throw new RangeError('Invalid focus direction');
        if(!this.exports.rice_wm_focus_direction)throw new Error('Directional focus requires an updated compositor');
        this.exports.rice_wm_navigation_begin();
        for(const {id,x,y,width,height} of frames) {
            if(![id,x,y,width,height].every(Number.isFinite))continue;
            this.exports.rice_wm_navigation_native(id,x,y,width,height);
        }
        return index;
    }
    focusDirection(direction, frames) {
        return this.exports.rice_wm_focus_direction(this.prepareWindowNavigation(direction,frames));
    }
    swapDirection(direction, frames) {
        const index=this.prepareWindowNavigation(direction,frames);
        if(!this.exports.rice_wm_swap_direction)throw new Error('Window swapping requires an updated compositor');
        return this.exports.rice_wm_swap_direction(index)===1;
    }
    tileControlAvailable(kind,id,action) {
        const index=['left','right','up','down','grow','shrink'].indexOf(action);
        return index>=0 && this.exports.rice_wm_tile_control_available?.(kind,id,index)===1;
    }
    insertDirection(direction,frames) {
        const index=this.prepareWindowNavigation(direction,frames);
        if(!this.exports.rice_wm_insert_direction)throw new Error('Tile insertion requires an updated compositor');
        return this.exports.rice_wm_insert_direction(index)===1;
    }
    #tileInsertionArguments(source,target,direction) {
        const index=['left','right','up','down'].indexOf(direction);
        if(index<0)throw new RangeError('Invalid insertion direction');
        for(const value of [source,target])if(!value||![0,1].includes(value.kind)||!Number.isInteger(value.id)||value.id<1||value.id>(value.kind===1?0xffff:0xffffffff))throw new TypeError('Invalid tile identity');
        return [source.kind,source.id,target.kind,target.id,index];
    }
    previewTileInsertion(source,target,direction) {
        const args=this.#tileInsertionArguments(source,target,direction),e=this.exports;
        if(!e.rice_wm_tile_insert_preview)throw new Error('Tile insertion previews require an updated compositor');
        if(e.rice_wm_tile_insert_preview(...args)!==1)return null;
        const [x,y,width,height]=[0,1,2,3].map(field=>e.rice_wm_tile_insert_preview_field(field));
        return {x:x|0,y:y|0,width,height};
    }
    tileAt(x,y) {
        if(![x,y].every(Number.isFinite)||!this.exports.rice_wm_tile_at_field)return null;
        const [kind,id,rx,ry,width,height]=[0,1,2,3,4,5].map(field=>this.exports.rice_wm_tile_at_field(x,y,field));
        return id?{kind,id,x:rx|0,y:ry|0,width,height}:null;
    }
    tiledX11TitleAt(x,y) {return this.exports.rice_wm_tiled_x11_title_at?.(x,y)||0;}
    activateTiledX11(id) {return this.exports.rice_wm_activate_tiled_x11?.(id)===1;}
    insertTile(source,target,direction) {
        const args=this.#tileInsertionArguments(source,target,direction);
        if(!this.exports.rice_wm_tile_insert)throw new Error('Tile insertion requires an updated compositor');
        return this.exports.rice_wm_tile_insert(...args)===1;
    }
    resizeTile(operation) {
        const index=['wider','narrower','taller','shorter','cycle-width','full-width','center'].indexOf(operation);
        if(index<0)throw new RangeError('Invalid tile resize operation');
        if(!this.exports.rice_wm_resize_tile)throw new Error('Tile resizing requires an updated compositor');
        return this.exports.rice_wm_resize_tile(index)===1;
    }
    tileDividerAt(x,y) {
        if(![x,y].every(Number.isFinite))return null;
        const token=this.exports.rice_wm_tile_divider_at?.(x,y)||0;
        if(!token)return null;
        const position=this.exports.rice_wm_tile_divider_position(token)>>>0;
        return position===0xffffffff?null:{token,position,axis:token&1?'y':'x'};
    }
    moveTileDivider(token,position) {
        if(!Number.isInteger(token)||token<2||!Number.isFinite(position))return false;
        return this.exports.rice_wm_tile_divider_move?.(token,position)===1;
    }

    captureWorkspaceLayout(workspace) {
        if(!Number.isInteger(workspace)||workspace<0||workspace>=this.workspaceLimit())throw new RangeError('Invalid workspace');
        const e=this.exports;
        if(!e.rice_wm_layout_snapshot)throw new Error('Layout snapshots require an updated compositor');
        const count=e.rice_wm_layout_snapshot(workspace);
        return count?Array.from({length:count},(_,index)=>e.rice_wm_layout_word(index)):null;
    }
    x11LayoutIdentity(id) {
        const e=this.exports,count=e.rice_wm_layout_identity?.(id)||0;
        return count?'x11:'+Array.from({length:count},(_,index)=>e.rice_wm_layout_identity_byte(index).toString(16).padStart(2,'0')).join(''):null;
    }
    x11LayoutFloatingFrame(id) {
        const e=this.exports;if(!e.rice_wm_layout_floating_field)return null;
        const [x,y,width,height]=[0,1,2,3].map(field=>e.rice_wm_layout_floating_field(id,field));
        return width&&height?{x:x|0,y:y|0,width,height}:null;
    }
    restoreX11LayoutFloatingFrame(id,frame) {
        return this.exports.rice_wm_layout_floating_restore?.(id,frame.x,frame.y,frame.width,frame.height)===1;
    }
    restoreWorkspaceLayout(workspace,words) {
        if(!Number.isInteger(workspace)||workspace<0||workspace>=this.workspaceLimit())throw new RangeError('Invalid workspace');
        if(!Array.isArray(words)||words.length>4096||words.some(word=>!Number.isInteger(word)||word<0||word>0xffffffff))throw new TypeError('Invalid layout snapshot words');
        const e=this.exports;
        if(!e.rice_wm_layout_restore)throw new Error('Layout snapshots require an updated compositor');
        e.rice_wm_layout_load_begin();
        for(const word of words)if(e.rice_wm_layout_load_word(word)!==1)throw new Error('Layout snapshot is too large');
        if(e.rice_wm_layout_restore(workspace)!==1)throw new Error('Layout snapshot is invalid or its windows are no longer available');
        return true;
    }

    tileClip(kind,id) {
        if(![0,1].includes(kind)||!Number.isInteger(id)||id<1||id>(kind===1?65535:0xffffffff))return null;
        const read=this.exports.rice_wm_tile_clip_field;if(!read)return null;
        const [x,y,width,height]=Array.from({length:4},(_,field)=>read(kind,id,field));
        return width&&height?{x:x|0,y:y|0,width,height}:null;
    }
    scrollTiles(delta,point=null) {
        if(!Number.isFinite(delta)||delta===0)return false;
        if(point!==null&&![point?.x,point?.y].every(value=>Number.isFinite(value)&&value>=-0x80000000&&value<=0x7fffffff))return false;
        const accumulated=Math.max(-0x7fffffff,Math.min(0x7fffffff,(this.tileScrollRemainder||0)+delta));
        const pixels=Math.trunc(Math.max(-0x7fffffff,Math.min(0x7fffffff,accumulated)));
        this.tileScrollRemainder=accumulated-pixels;
        if(!pixels)return false;
        const changed=(point&&this.exports.rice_wm_scroll_tiles_at?this.exports.rice_wm_scroll_tiles_at(pixels,point.x,point.y):this.exports.rice_wm_scroll_tiles?.(pixels))===1;
        if(!changed)this.tileScrollRemainder=0;
        return changed;
    }
    selectDragOutput(source,point) {
        if(!Number.isInteger(source)||source<1||source>0xffffffff||![point?.x,point?.y].every(value=>Number.isInteger(value)&&value>=-32768&&value<=32767))return false;
        return this.exports.rice_wm_drag_output_select?.(source,point.x,point.y)===1;
    }
    scrollDragTiles(source,delta,point) {
        if(!Number.isInteger(source)||source<1||source>0xffffffff||!Number.isFinite(delta)||Math.abs(delta)>4096||![point?.x,point?.y].every(value=>Number.isFinite(value)&&value>=-0x80000000&&value<=0x7fffffff))return false;
        const pixels=Math.trunc(delta);if(!pixels)return false;
        return this.exports.rice_wm_scroll_drag_tiles_at?.(source,pixels,point.x,point.y)===1;
    }
    automaticTilePlan(ids, enabled, arrangement = 'Split', minimums = [], active = 0, positions = [], workspace = this.currentWorkspace()) {
        if(!Number.isInteger(workspace)||workspace<0||workspace>=this.workspaceCount())throw new RangeError('Invalid workspace');
        if(!Array.isArray(ids)||ids.length>128||ids.some(id=>!Number.isInteger(id)||id<1||id>65535)||new Set(ids).size!==ids.length)throw new TypeError('Invalid native tile identities');
        if(typeof enabled!=='boolean'||!['Split','Master stack','Scrolling'].includes(arrangement))throw new TypeError('Invalid tile arrangement');
        if(!Number.isInteger(active)||active<0||active>65535)throw new TypeError('Invalid active tile identity');
        if(!Array.isArray(minimums)||minimums.length>128||!Array.isArray(positions)||positions.length>128)throw new TypeError('Invalid native tile metadata');
        for(const position of positions) {
            if(!position||!ids.includes(position.id)||![position.x,position.y].every(value=>Number.isFinite(value)&&value>=-0x80000000&&value<=0x7fffffff))throw new TypeError('Invalid native tile position');
        }
        const e = this.exports;
        if (!e.rice_wm_auto_tile_begin) return null;
        for(const {id,width,height} of minimums) {
            if(![id,width,height].every(value=>Number.isInteger(value)&&value>=0&&value<=65535)||id===0||!ids.includes(id))throw new TypeError('Invalid native tile minimum');
        }
        if(minimums.length&&!e.rice_wm_auto_tile_minimum)throw new Error('Application tile minima require an updated compositor');
        if(arrangement==='Scrolling'&&!e.rice_wm_auto_tile_scrolling)throw new Error('Scrolling tiling requires an updated compositor');
        if(arrangement==='Master stack'&&!e.rice_wm_auto_tile_master)throw new Error('Master stack requires an updated compositor');
        e.rice_wm_auto_tile_begin(enabled ? 1 : 0);
        e.rice_wm_auto_tile_workspace?.(workspace);
        e.rice_wm_auto_tile_scrolling?.(arrangement==='Scrolling'?1:0);
        e.rice_wm_auto_tile_focus?.(active);
        e.rice_wm_auto_tile_master?.(arrangement==='Master stack'?1:0);
        for (const id of ids) e.rice_wm_auto_tile_add(id);
        for(const {id,width,height} of minimums) {
            e.rice_wm_auto_tile_minimum(id,width,height);
        }
        for(const {id,x,y,fullscreen} of positions)if(ids.includes(id)&&[x,y].every(Number.isFinite)){e.rice_wm_auto_tile_position?.(id,Math.round(x),Math.round(y));if(fullscreen)e.rice_wm_auto_tile_fullscreen?.(id);}
        const count = e.rice_wm_auto_tile_apply();
        this.scrollingClips=Array.from({length:e.rice_wm_auto_tile_clip_count?.()||0},(_,index)=>{
            const [kind,id,x,y,width,height]=Array.from({length:6},(_,field)=>e.rice_wm_auto_tile_clip_field(index,field));
            return {kind,id,x:x|0,y:y|0,width,height};
        });
        return Array.from({length:count}, (_,i)=>{
            const [id,x,y,width,height] = Array.from({length:5},(_,field)=>e.rice_wm_auto_tile_rect(i,field));
            return {id,x:x|0,y:y|0,width,height,...(arrangement==='Scrolling'?{clip:this.scrollingClips.find(c=>c.kind===1&&c.id===id)}:{})};
        });
    }

    setTopBarVisible(visible) {
        this.topBarVisible = visible !== false;
        this.setGnomePanel(this.lastGnomePanelSnapshot ?? null);
    }

    setGnomePanel(snapshot = null) {
        this.lastGnomePanelSnapshot = snapshot;
        if (
            typeof this.exports.rice_wm_gnome_panel_update_v2 !== "function" &&
            typeof this.exports.rice_wm_gnome_panel_update !== "function"
        ) {
            return false;
        }
        const profile = snapshot?.profile;
        const panelState = snapshot?.shellState?.panel;
        const styleClasses = new Set(snapshot?.shellState?.styleClasses || []);
        const panelEnabled = this.topBarVisible !== false && snapshot !== null && !styleClasses.has(
            "just-perfection-api-no-panel",
        );
        const indicators = Array.isArray(snapshot?.indicators)
            ? snapshot.indicators
            : [];
        const findValue = (actor, key) => {
            if (!actor || typeof actor !== "object") return "";
            if (actor.visible !== false && actor[key]) return String(actor[key]);
            for (const child of actor.children || []) {
                const value = findValue(child, key);
                if (value) return value;
            }
            return "";
        };
        if (snapshot !== null && profile !== 50) {
            throw new RangeError("RiceWM only accepts GNOME Shell 50 panel snapshots");
        }
        const positionOf = value =>
            new Map([["left", 0], ["center", 1], ["right", 2]])
                .get(String(value || "right")) ?? 2;
        const clean = value => String(value || "").replace(/[\t\r\n\0]/g," ").slice(0,128);
        this.setTopbarWire(1, indicators.map(actor => [clean(actor.uuid),clean(actor.title || actor.name || actor.uuid?.split("@")[0]?.replace(/[-_]/g," ")),clean(findValue(actor,"icon"))].join("\t")).join("\n"));
        const records = [];
        const encoder = new TextEncoder();
        if (snapshot !== null) {
            indicators
                .filter(actor => actor?.visible !== false)
                .slice(0, 16)
                .forEach(actor => {
                    const uuid = encoder.encode(
                        String(actor?.uuid || snapshot?.uuid || "").slice(0, 128),
                    );
                    if (uuid.byteLength === 0) return;
                    const text = findValue(actor, "text").slice(0, 128);
                    const label = encoder.encode(text);
                    const icon = findValue(actor, "icon") ||
                        findValue(actor, "iconPath") ||
                        findValue(actor, "imagePath");
                    const record = new Uint8Array(
                        4 + uuid.byteLength + 4 + label.byteLength + 4,
                    );
                    const view = new DataView(record.buffer);
                    let offset = 0;
                    view.setUint32(offset, uuid.byteLength, true);
                    offset += 4;
                    record.set(uuid, offset);
                    offset += uuid.byteLength;
                    view.setUint32(offset, label.byteLength, true);
                    offset += 4;
                    record.set(label, offset);
                    offset += label.byteLength;
                    record[offset] = 1;
                    record[offset + 1] = icon ? 1 : 0;
                    record[offset + 2] = positionOf(actor.panelPosition);
                    record[offset + 3] = Number(actor.index) || 0;
                    records.push(record);
                });
        }
        const byteLength = records.reduce(
            (total, record) => total + record.byteLength,
            4,
        );
        const stream = new Uint8Array(byteLength);
        new DataView(stream.buffer).setUint32(0, records.length, true);
        let offset = 4;
        for (const record of records) {
            stream.set(record, offset);
            offset += record.byteLength;
        }
        const pointer = this.#reserve(stream.byteLength);
        new Uint8Array(this.memory.buffer, pointer, stream.byteLength).set(stream);
        const v2 = typeof this.exports.rice_wm_gnome_panel_update_v2 === "function";
        if (panelState && typeof this.exports.rice_wm_gnome_panel_configure === "function") {
            const height = Math.max(1, Math.min(128, Number(panelState.height) || 32));
            const bottom = styleClasses.has("just-perfection-api-bottom-panel") ||
                Number(panelState.y) > 0;
            if (this.exports.rice_wm_gnome_panel_configure(height, bottom ? 1 : 0) !== 1) {
                throw new RangeError("RiceWM rejected the GNOME panel geometry");
            }
        }
        if (v2) {
            if (this.exports.rice_wm_gnome_panel_update_v2(
                panelEnabled ? 1 : 0,
                pointer,
                stream.byteLength,
            ) !== 1) {
                throw new RangeError("RiceWM rejected the GNOME Shell 50 panel snapshot");
            }
        } else if (this.exports.rice_wm_gnome_panel_update(
            panelEnabled ? 1 : 0,
            records.length ? 1 : 0,
            records.some(record => record[record.length - 3] === 1) ? 1 : 0,
            records.length ? records[0][records[0].length - 2] : 2,
            pointer,
            stream.byteLength,
        ) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME Shell 50 panel snapshot");
        }
        if (snapshot?.menu && snapshot.uuid &&
            typeof this.exports.rice_wm_gnome_menu_indicator_update === "function") {
            const uuid = encoder.encode(String(snapshot.uuid).slice(0, 128));
            const uuidPointer = this.#reserve(uuid.byteLength);
            new Uint8Array(
                this.memory.buffer,
                uuidPointer,
                uuid.byteLength,
            ).set(uuid);
            this.exports.rice_wm_gnome_menu_indicator_update(
                uuidPointer,
                uuid.byteLength,
            );
        }
        if (typeof this.exports.rice_wm_gnome_menu_replace === "function") {
            const hasImage = actor => Boolean(
                actor?.imagePath || (actor?.children || []).some(hasImage)
            );
            const records = (Array.isArray(snapshot?.menu?.items)
                ? snapshot.menu.items : []).slice(0, 64).map((item, fallbackIndex) => {
                const label = new TextEncoder().encode(findValue(item, "text").slice(0, 192));
                const index = Number.isInteger(Number(item?.index))
                    ? Math.max(0, Math.min(255, Number(item.index))) : fallbackIndex;
                const iconName = findValue(item, "icon").toLowerCase();
                const iconPath = findValue(item, "iconPath");
                const toggle = item?.kind === "switch-menu-item"
                    || findValue(item, "styleClass").split(/\s+/).includes("popup-switch-menu-item");
                const checked = toggle
                    && findValue(item, "styleClass").split(/\s+/).includes("checked");
                const iconKind = iconPath ? 7
                    : /alarm|timer/.test(iconName) ? 15
                        : /pause/.test(iconName) ? 9
                            : /stop/.test(iconName) ? 10
                                : /playback-start|play|start/.test(iconName) ? 8
                                    : /folder|directory|drive|home/.test(iconName) ? 11
                                        : /location|map|pin/.test(iconName) ? 12
                                            : /copy|clipboard/.test(iconName) ? 13
                                                : /eject|unmount/.test(iconName) ? 14
                                                    : /application|executable|apps/.test(iconName) ? 16
                    : /network|server|web|globe/.test(iconName) ? 1
                        : /share|emblem-shared/.test(iconName) ? 2
                            : /time|clock/.test(iconName) ? 3
                                : /settings|preferences|gear/.test(iconName) ? 4
                                    : /history|recent|refresh/.test(iconName) ? 5
                                        : /link|uri|open/.test(iconName) ? 6
                                            : iconName ? 7 : 0;
                const flags = (item?.separator ? 1 : 0)
                    | (item?.reactive === false ? 0 : 2)
                    | (toggle ? 4 : 0)
                    | (hasImage(item) ? 8 : 0);
                const stateFlags = flags | (checked ? 16 : 0);
                const record = new Uint8Array(6 + label.byteLength);
                const view = new DataView(record.buffer);
                view.setUint8(0, index);
                view.setUint8(1, stateFlags);
                view.setUint16(2, label.byteLength, true);
                view.setUint8(4, iconKind);
                record.set(label, 6);
                return record;
            });
            const byteLength = records.reduce((total, record) => total + record.byteLength, 0);
            let menuPointer = 0;
            if (byteLength) {
                menuPointer = this.#reserve(byteLength);
                const destination = new Uint8Array(this.memory.buffer, menuPointer, byteLength);
                let offset = 0;
                for (const record of records) {
                    destination.set(record, offset);
                    offset += record.byteLength;
                }
            }
            if (this.exports.rice_wm_gnome_menu_replace(menuPointer, byteLength) !== 1) {
                throw new RangeError("RiceWM rejected the GNOME Shell 50 popup menu");
            }
        }
        return true;
    }

    setGnomeMenuImage(width, height, pixels) {
        if (typeof this.exports.rice_wm_gnome_menu_image_update !== "function") return false;
        const source = pixels instanceof Uint32Array ? pixels : new Uint32Array(pixels || []);
        const expected = Number(width) * Number(height);
        if (!Number.isInteger(expected) || expected <= 0 || expected > 512 * 512 || source.length !== expected) {
            throw new RangeError("invalid GNOME extension popup image");
        }
        const pointer = this.#reserve(source.byteLength);
        new Uint8Array(this.memory.buffer, pointer, source.byteLength)
            .set(new Uint8Array(source.buffer, source.byteOffset, source.byteLength));
        if (this.exports.rice_wm_gnome_menu_image_update(
            Number(width), Number(height), pointer, source.length,
        ) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME extension popup image");
        }
        return true;
    }

    setGnomeMenuIcon(index, width, height, pixels) {
        if (typeof this.exports.rice_wm_gnome_menu_icon_update !== "function") return false;
        return this.#uploadGnomePixels(
            "rice_wm_gnome_menu_icon_update",
            [Number(index), Number(width), Number(height)],
            pixels,
        );
    }

    setGnomeDialog(uuid, title, body, buttons) {
        if (typeof this.exports.rice_wm_gnome_dialog_replace !== "function") return false;
        if (buttons === undefined) {
            buttons = body;
            body = [];
        }
        const encoder = new TextEncoder();
        const uuidBytes = encoder.encode(String(uuid || "").slice(0, 128));
        const titleBytes = encoder.encode(String(title || "").slice(0, 256));
        const encodeList = (values, count, length) => {
            const records = (Array.isArray(values) ? values : []).slice(0, count)
                .map(label => encoder.encode(String(label).slice(0, length)))
                .filter(record => record.byteLength > 0);
            const bytes = new Uint8Array(4 + records.reduce(
                (total, record) => total + 4 + record.byteLength, 0,
            ));
            const view = new DataView(bytes.buffer);
            view.setUint32(0, records.length, true);
            let offset = 4;
            for (const record of records) {
                view.setUint32(offset, record.byteLength, true);
                offset += 4;
                bytes.set(record, offset);
                offset += record.byteLength;
            }
            return bytes;
        };
        const bodyBytes = encodeList(body, 24, 256);
        const buttonsBytes = encodeList(buttons, 16, 64);
        // All arguments share RiceWM's one staging allocation. Lay them out in
        // a single frame so later writes cannot corrupt earlier strings.
        const titleOffset = uuidBytes.byteLength;
        const bodyOffset = titleOffset + titleBytes.byteLength;
        const buttonsOffset = bodyOffset + bodyBytes.byteLength;
        const pointer = this.#reserve(buttonsOffset + buttonsBytes.byteLength);
        const frame = new Uint8Array(
            this.memory.buffer,
            pointer,
            buttonsOffset + buttonsBytes.byteLength,
        );
        frame.set(uuidBytes, 0);
        frame.set(titleBytes, titleOffset);
        frame.set(bodyBytes, bodyOffset);
        frame.set(buttonsBytes, buttonsOffset);
        if (this.exports.rice_wm_gnome_dialog_replace(
            pointer,
            uuidBytes.byteLength,
            pointer + titleOffset,
            titleBytes.byteLength,
            pointer + bodyOffset,
            bodyBytes.byteLength,
            pointer + buttonsOffset,
            buttonsBytes.byteLength,
        ) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME extension dialog");
        }
        return true;
    }

    setGnomeOverlay(uuid, title, body, buttons, rectangles = []) {
        if (!this.setGnomeDialog(uuid, title, body, buttons)) return false;
        if (typeof this.exports.rice_wm_gnome_dialog_fullscreen !== "function") return false;
        if (this.exports.rice_wm_gnome_dialog_fullscreen(1) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME extension stage overlay");
        }
        if (typeof this.exports.rice_wm_gnome_overlay_rects_update === "function") {
            const records = (Array.isArray(rectangles) ? rectangles : []).slice(0, 512)
                .map(rectangle => {
                    const sourceWidth = Math.max(1, Number(rectangle.sourceWidth) || this.width || 1);
                    const sourceHeight = Math.max(1, Number(rectangle.sourceHeight) || this.height || 1);
                    return {
                        x: Math.max(0, Math.round((Number(rectangle.x) || 0) * this.width / sourceWidth)),
                        y: Math.max(0, Math.round((Number(rectangle.y) || 0) * this.height / sourceHeight)),
                        width: Math.max(1, Math.round((Number(rectangle.width) || 0) * this.width / sourceWidth)),
                        height: Math.max(1, Math.round((Number(rectangle.height) || 0) * this.height / sourceHeight)),
                        color: Number(rectangle.color) >>> 0,
                    };
                })
                .filter(rectangle => rectangle.width > 0 && rectangle.height > 0);
            const bytes = new Uint8Array(4 + records.length * 12);
            const view = new DataView(bytes.buffer);
            view.setUint32(0, records.length, true);
            records.forEach((rectangle, index) => {
                const offset = 4 + index * 12;
                view.setUint16(offset, Math.min(65535, rectangle.x), true);
                view.setUint16(offset + 2, Math.min(65535, rectangle.y), true);
                view.setUint16(offset + 4, Math.min(65535, rectangle.width), true);
                view.setUint16(offset + 6, Math.min(65535, rectangle.height), true);
                view.setUint32(offset + 8, rectangle.color, true);
            });
            const pointer = this.#reserve(bytes.byteLength);
            new Uint8Array(this.memory.buffer, pointer, bytes.byteLength).set(bytes);
            if (this.exports.rice_wm_gnome_overlay_rects_update(pointer, bytes.byteLength) !== 1) {
                throw new RangeError("RiceWM rejected GNOME overlay actor geometry");
            }
        }
        return true;
    }

    setGnomeBackground(rectangles = [], texts = []) {
        if (typeof this.exports.rice_wm_gnome_background_update !== "function") return false;
        const scaleRect = item => {
            const sourceWidth = Math.max(1, Number(item.sourceWidth) || this.width || 1);
            const sourceHeight = Math.max(1, Number(item.sourceHeight) || this.height || 1);
            const x = Math.round((Number(item.x) || 0) * this.width / sourceWidth);
            const y = Math.round((Number(item.y) || 0) * this.height / sourceHeight);
            const width = Math.max(1, Math.round(
                (Number(item.width) || 0) * this.width / sourceWidth,
            ));
            const height = Math.max(1, Math.round(
                (Number(item.height) || 0) * this.height / sourceHeight,
            ));
            return {
                x: Math.max(0, x),
                y: Math.max(0, y),
                width: Math.max(1, width + Math.min(0, x)),
                height: Math.max(1, height + Math.min(0, y)),
            };
        };
        const boundedRectangles = (Array.isArray(rectangles) ? rectangles : [])
            .slice(0, 512).map(item => ({ ...scaleRect(item), color: Number(item.color) >>> 0 }));
        const encoder = new TextEncoder();
        const boundedTexts = (Array.isArray(texts) ? texts : []).slice(0, 4096)
            .flatMap(item => {
                const text = encoder.encode(String(item.text || ""));
                if (text.byteLength === 0 || text.byteLength > 255) return [];
                return [{
                    ...scaleRect(item),
                    fontPx: Math.max(1, Math.min(255, Math.round(
                        (Number(item.fontPx) || 12) * this.height /
                        Math.max(1, Number(item.sourceHeight) || this.height || 1),
                    ))),
                    weight: Math.max(1, Math.min(10, Math.round((Number(item.weight) || 400) / 100))),
                    align: Math.max(0, Math.min(2, Number(item.align) || 0)),
                    foreground: Number(item.foreground) >>> 0,
                    text,
                }];
            });
        const textBytes = boundedTexts.reduce(
            (total, item) => total + ((16 + item.text.byteLength + 3) & ~3), 0,
        );
        const bytes = new Uint8Array(4 + boundedRectangles.length * 12 + textBytes);
        const view = new DataView(bytes.buffer);
        view.setUint16(0, boundedRectangles.length, true);
        view.setUint16(2, boundedTexts.length, true);
        let offset = 4;
        for (const item of boundedRectangles) {
            view.setUint16(offset, Math.min(65535, item.x), true);
            view.setUint16(offset + 2, Math.min(65535, item.y), true);
            view.setUint16(offset + 4, Math.min(65535, item.width), true);
            view.setUint16(offset + 6, Math.min(65535, item.height), true);
            view.setUint32(offset + 8, item.color, true);
            offset += 12;
        }
        for (const item of boundedTexts) {
            view.setUint16(offset, Math.min(65535, item.x), true);
            view.setUint16(offset + 2, Math.min(65535, item.y), true);
            view.setUint16(offset + 4, Math.min(65535, item.width), true);
            view.setUint16(offset + 6, Math.min(65535, item.height), true);
            view.setUint8(offset + 8, item.fontPx);
            view.setUint8(offset + 9, item.weight);
            view.setUint8(offset + 10, item.align);
            view.setUint8(offset + 11, item.text.byteLength);
            view.setUint32(offset + 12, item.foreground, true);
            bytes.set(item.text, offset + 16);
            offset += (16 + item.text.byteLength + 3) & ~3;
        }
        const pointer = this.#reserve(bytes.byteLength);
        new Uint8Array(this.memory.buffer, pointer, bytes.byteLength).set(bytes);
        if (this.exports.rice_wm_gnome_background_update(pointer, bytes.byteLength) !== 1) {
            throw new RangeError("RiceWM rejected GNOME extension background actors");
        }
        return true;
    }

    clearGnomeDialog() {
        if (typeof this.exports.rice_wm_gnome_dialog_clear !== "function") return false;
        return this.exports.rice_wm_gnome_dialog_clear() === 1;
    }

    getTaskbarGeometry() {
        const read = this.exports.rice_wm_taskbar_geometry;
        if (typeof read !== "function") return null;
        const pointer = this.#reserve(32);
        if (read(pointer) !== 1) return null;
        const view = new DataView(this.memory.buffer, pointer, 32);
        return {edge:view.getInt32(0,true),left:view.getInt32(4,true),top:view.getInt32(8,true),
            right:view.getInt32(12,true),bottom:view.getInt32(16,true),
            alwaysOnTop:view.getInt32(20,true)!==0,autoHide:view.getInt32(24,true)!==0,
            visible:view.getInt32(28,true)!==0};
    }

    updateNotificationIcon(slot, icon) {
        const fn = this.exports.rice_wm_notification_icon_update;
        if (typeof fn !== "function") return false;
        const tip = new TextEncoder().encode(icon.tip || "");
        const pixels = icon.pixels;
        if (!(pixels instanceof Uint32Array) || tip.length > 512) return false;
        const offset = (tip.length + 3) & ~3;
        const pointer = this.#reserve(offset + pixels.byteLength);
        const bytes = new Uint8Array(this.memory.buffer, pointer, offset + pixels.byteLength);
        bytes.set(tip);
        bytes.set(new Uint8Array(pixels.buffer, pixels.byteOffset, pixels.byteLength), offset);
        return fn(slot, pointer, tip.length, icon.iconWidth, icon.iconHeight, pointer+offset, pixels.length, icon.hidden ? 1 : 0) === 1;
    }
    removeNotificationIcon(slot) { return this.exports.rice_wm_notification_icon_remove?.(slot) === 1; }
    focusNotificationIcon(slot) { return this.exports.rice_wm_notification_icon_focus?.(slot) === 1; }
    drainNotificationEvents() {
        const fn = this.exports.rice_wm_notification_event_drain;
        if (typeof fn !== "function") return [];
        const pointer = this.#reserve(16), events = [];
        while (events.length < 1024 && fn(pointer) === 1) {
            const view = new DataView(this.memory.buffer, pointer, 16);
            events.push({slot:view.getUint32(0,true), event:view.getUint32(4,true), x:view.getInt32(8,true), y:view.getInt32(12,true)});
        }
        return events;
    }

    setGnomeIndicatorImage(uuid, width, height, pixels) {
        if (typeof this.exports.rice_wm_gnome_indicator_image_update !== "function") return false;
        const encoded = new TextEncoder().encode(String(uuid || "").slice(0, 128));
        if (encoded.byteLength === 0) return false;
        const source = pixels instanceof Uint32Array
            ? pixels
            : new Uint32Array(pixels || []);
        if (source.length === 0 || source.length > 128 * 128) {
            throw new RangeError("invalid GNOME extension icon pixels");
        }
        // rice_io_reserve exposes one reusable staging allocation. Reserving
        // the UUID and pixels independently aliases both pointers and lets the
        // pixel upload overwrite the UUID before the Wasm call. Keep the two
        // arguments in one aligned frame, as a native multi-buffer call would.
        const pixelOffset = (encoded.byteLength + 3) & ~3;
        const pointer = this.#reserve(pixelOffset + source.byteLength);
        const destination = new Uint8Array(
            this.memory.buffer,
            pointer,
            pixelOffset + source.byteLength,
        );
        destination.fill(0);
        destination.set(encoded, 0);
        destination.set(
            new Uint8Array(source.buffer, source.byteOffset, source.byteLength),
            pixelOffset,
        );
        if (this.exports.rice_wm_gnome_indicator_image_update(
            pointer,
            encoded.byteLength,
            Number(width),
            Number(height),
            pointer + pixelOffset,
            source.length,
        ) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME extension icon");
        }
        return true;
    }

    replaceQuickLaunchers(visible, launchers) {
        if (!Array.isArray(launchers) || launchers.length > 8) {
            throw new RangeError("invalid RiceWM quick-launch layout");
        }
        this.exports.rice_wm_dock_identity_set?.(1,0,0);
        for (const launcher of launchers) {
            const identity = launcher.applicationId || ({files:"native:rice-files",terminal:"native:rice-term","image-viewer":"native:rice-view"})[launcher.type];
            this.exports.rice_wm_dock_identity_set?.(1,launcher.action,dockApplicationKey(identity));
        }
        const recordBytes = 72;
        const maximumTitleBytes = 64;
        const encoder = new TextEncoder();
        const records = new Uint8Array(launchers.length * recordBytes);
        const view = new DataView(records.buffer);
        launchers.forEach((launcher, index) => {
            const action = Number(launcher?.action) >>> 0;
            const icon = Number(launcher?.icon);
            if (!action || !Number.isInteger(icon) || icon < 0 || icon > 0xff) {
                throw new RangeError("invalid RiceWM quick shortcut");
            }
            let title = String(launcher?.title || "Application").trim();
            while (title && encoder.encode(title).byteLength > maximumTitleBytes) {
                title = title.slice(0, -1);
            }
            const encodedTitle = encoder.encode(title);
            if (!encodedTitle.byteLength) {
                throw new RangeError("invalid RiceWM quick shortcut title");
            }
            const offset = index * recordBytes;
            view.setUint32(offset, action, true);
            // Named artwork is composited by the browser, as in the menu.
            view.setUint8(offset + 4, launcher.iconName ? 255 : icon);
            view.setUint8(offset + 5, encodedTitle.byteLength);
            records.set(encodedTitle, offset + 8);
        });
        let pointer = 0;
        if (records.byteLength) {
            pointer = this.#reserve(records.byteLength);
            new Uint8Array(this.memory.buffer, pointer, records.byteLength).set(records);
        }
        if (
            typeof this.exports.rice_wm_quick_launch_replace === "function" &&
            this.exports.rice_wm_quick_launch_replace(
                visible ? 1 : 0,
                pointer,
                records.byteLength,
            ) !== 1
        ) {
            throw new RangeError("RiceWM rejected the quick shortcuts");
        }
    }

    replaceTrayItems(items) {
        if (!Array.isArray(items) || items.length > 12) {
            throw new RangeError("invalid RiceWM tray layout");
        }
        const tags = {
            clock: 1,
            workspaces: 2,
            sound: 3,
            launcher: 4,
            performance: 5,
            connectivity: 6,
        };
        const iconSlugs = [
            "calculator", "chevron-down", "chevron-left", "chevron-right", "chevron-up",
            "desktop", "dialog-information", "dialog-warning", "document-new", "document-open",
            "document-save-as", "document-save", "drive", "edit-copy", "edit-cut", "edit-delete",
            "edit-find", "edit-paste", "edit-redo", "edit-rename", "edit-undo", "file", "files",
            "folder", "home", "image-viewer", "image", "lock", "menu", "navigate-up", "network",
            "object-add", "object-remove", "search", "selection-check", "settings", "terminal",
            "text-editor", "trash-empty", "trash-full", "view-detail", "view-grid", "view-refresh",
            "window-close", "window-maximize", "window-minimize", "window-restore",
        ];
        const records = new Uint8Array(items.length * 12);
        const view = new DataView(records.buffer);
        const seen = new Set();
        items.forEach((item, index) => {
            const tag = tags[item?.type];
            const action = item?.type === "launcher" ? Number(item.action) >>> 0 : 0;
            const icon = tag === 4 ? iconSlugs.indexOf(String(item.icon)) : 0;
            const validColor = tag === 4 && /^#[0-9a-f]{6}$/i.test(item.color);
            const color = validColor
                ? Number.parseInt(item.color.slice(1), 16)
                : 0;
            if (
                !tag || seen.has(tag) ||
                (tag === 4 && (action === 0 || icon < 0 || !validColor))
            ) {
                throw new RangeError("invalid or duplicate RiceWM tray item");
            }
            seen.add(tag);
            const offset = index * 12;
            view.setUint8(offset, tag);
            view.setUint8(offset + 1, icon);
            view.setUint32(offset + 4, action, true);
            view.setUint32(offset + 8, color, true);
        });
        let pointer = 0;
        if (records.byteLength) {
            pointer = this.#reserve(records.byteLength);
            new Uint8Array(this.memory.buffer, pointer, records.byteLength).set(records);
        }
        if (
            typeof this.exports.rice_wm_tray_replace === "function" &&
            this.exports.rice_wm_tray_replace(pointer, records.byteLength) !== 1
        ) {
            throw new RangeError("RiceWM rejected the tray layout");
        }
    }

    setPerformanceSample(cpu, receivedPerSecond = 0, sentPerSecond = 0) {
        if (typeof this.exports.rice_wm_performance_sample !== "function") return false;
        const cpuValue = Math.max(0, Math.min(100, Math.round(Number(cpu) || 0)));
        const networkValue = Math.max(0, Math.min(
            100,
            Math.round((Number(receivedPerSecond) + Number(sentPerSecond)) / 1024),
        ));
        return this.exports.rice_wm_performance_sample(cpuValue, networkValue) === 1;
    }

    taskbarPointer(x, y) {
        if (this.exports.rice_wm_taskbar_pointer(x, y) !== 1) {
            throw new RangeError("invalid RiceWM taskbar pointer position");
        }
    }

    supportsScrollingWorkspaces() {
        return typeof this.exports.rice_wm_scrolling_workspaces === 'function';
    }
    setScrollingWorkspaces(enabled) {
        if(enabled&&!this.exports.rice_wm_scrolling_workspaces)throw Error('Dynamic workspaces require an updated compositor');
        this.exports.rice_wm_scrolling_workspaces?.(enabled?1:0);
        this.scrollingWorkspaces=Boolean(enabled);
    }
    workspaceCount() {return this.exports.rice_wm_workspace_count?.()||4;}
    workspaceLimit() {return this.scrollingWorkspaces?128:this.workspaceCount();}
    activeOutput() {return this.exports.rice_wm_active_output?.()||0;}
    outputWorkspace(output=this.activeOutput()) {return this.exports.rice_wm_output_workspace?.(output)??this.currentWorkspace();}
    workspaceIds(output=this.activeOutput()) {
        const read=this.exports.rice_wm_workspace_list;
        if(!read)return Array.from({length:4},(_,i)=>i);
        const count=read(output,0xffffffff);
        return Array.from({length:Math.min(128,count)},(_,i)=>read(output,i));
    }
    setNativeOutputPosition(id,x,y) {
        if(!Number.isInteger(id)||id<1||id>65535||![x,y].every(value=>Number.isFinite(value)&&value>=-0x80000000&&value<=0x7fffffff))throw new RangeError('Invalid native output position');
        this.exports.rice_wm_native_output_position?.(id,Math.round(x),Math.round(y));
    }
    workspaceWindowVisible(kind,id,workspace) {
        if(![0,1].includes(kind)||!Number.isInteger(id)||id<1||id>(kind===1?65535:0xffffffff)||!Number.isInteger(workspace)||workspace<0||workspace>=this.workspaceLimit())return false;
        return this.exports.rice_wm_workspace_visible?this.exports.rice_wm_workspace_visible(kind,id,workspace)===1:workspace===this.currentWorkspace();
    }
    currentWorkspace() {
        if (typeof this.exports.rice_wm_current_workspace !== "function") {
            return 0;
        }
        const workspace = this.exports.rice_wm_current_workspace() >>> 0;
        if (workspace === RESULT_ERROR) {
            throw new Error("RiceWM workspace state is not initialized");
        }
        return workspace;
    }

    moveOverviewWindow(id, workspace, x, y) {
        if (![id, workspace, x, y].every(Number.isInteger) || id < 1 || id > 0xffffffff || workspace < 0 || workspace >= this.workspaceLimit() || x < -32768 || x > 32767 || y < -32768 || y > 32767) return false;
        if (!this.exports.rice_wm_overview_move) throw Error('Moving Activities windows requires an updated compositor');
        return this.exports.rice_wm_overview_move(id, workspace, x, y) === 1;
    }

    switchWorkspace(workspace) {
        const next = Number(workspace);
        if (!Number.isInteger(next) || next < 0 || next >= this.workspaceLimit()) {
            throw new RangeError("invalid RiceWM workspace");
        }
        if (typeof this.exports.rice_wm_workspace_switch !== "function") {
            throw new Error("RiceWM workspace switching is unavailable");
        }
        return this.exports.rice_wm_workspace_switch(next) === 1;
    }

    replaceNativeTasks(tasks) {
        if (typeof this.exports.rice_wm_native_tasks_replace !== "function") {
            return false;
        }
        if (!Array.isArray(tasks) || tasks.length > 48) {
            throw new RangeError("invalid RiceWM native task list");
        }
        const encoder = new TextEncoder();
        this.exports.rice_wm_dock_identity_set?.(0,0,0);
        const records = tasks.map(task => {
            const id = Number(task.id);
            const workspace = Number(task.workspace);
            const icon = Number(task.icon) || 0;
            let titleText = '';
            for (const character of String(task.title || "Native Task").replace(/[\x00-\x1f\x7f]/gu, ' ')) {
                if (encoder.encode(titleText + character).byteLength > 48) break;
                titleText += character;
            }
            const title = encoder.encode(titleText);
            if (
                !Number.isInteger(id) || id < 1 || id > 0xffff ||
                !Number.isInteger(workspace) || workspace < 0 || workspace >= this.workspaceLimit() ||
                !Number.isInteger(icon) || icon < 0 || icon > 0xff ||
                title.byteLength < 1 || title.byteLength > 48
            ) {
                throw new RangeError("invalid RiceWM native task");
            }
            this.exports.rice_wm_dock_identity_set?.(0,id,dockApplicationKey(task.applicationId));
            return { task, id, workspace, icon, title };
        });
        const byteLength = records.reduce(
            (total, record) => total + 8 + record.title.byteLength,
            0,
        );
        if (byteLength === 0) {
            if (this.exports.rice_wm_native_tasks_replace(0, 0) !== 1) {
                throw new Error("RiceWM rejected the native task list");
            }
            return true;
        }
        const pointer = this.#reserve(byteLength);
        const bytes = new Uint8Array(this.memory.buffer, pointer, byteLength);
        let offset = 0;
        for (const { task, id, workspace, icon, title } of records) {
            const view = new DataView(bytes.buffer, bytes.byteOffset + offset, 8);
            view.setUint16(0, id, true);
            view.setUint8(2, workspace);
            view.setUint8(
                3,
                (task.minimized ? 0x01 : 0) |
                    (task.active ? 0x02 : 0) |
                    (task.maximized ? 0x04 : 0),
            );
            view.setUint8(4, icon);
            view.setUint8(5, title.byteLength);
            view.setUint16(6, 0, true);
            bytes.set(title, offset + 8);
            offset += 8 + title.byteLength;
        }
        if (this.exports.rice_wm_native_tasks_replace(pointer, byteLength) !== 1) {
            throw new Error("RiceWM rejected the native task list");
        }
        return true;
    }

    showNativeTaskMenu(taskId, x, y) {
        if (typeof this.exports.rice_wm_native_task_menu_show !== "function") {
            return false;
        }
        const id = Number(taskId);
        const rootX = Math.floor(Number(x));
        const rootY = Math.floor(Number(y));
        if (
            !Number.isInteger(id) || id < 1 || id > 0xffff ||
            !Number.isFinite(rootX) || !Number.isFinite(rootY)
        ) {
            throw new RangeError("invalid RiceWM native task menu anchor");
        }
        return this.exports.rice_wm_native_task_menu_show(id, rootX, rootY) === 1;
    }

    showNotification(kind, title, message = "") {
        const kinds = {
            info: 0,
            storage: 1,
            warning: 2,
            error: 3,
        };
        const notificationKind =
            typeof kind === "string" ? kinds[kind] : Number(kind);
        const printableAscii = (value, maximum) => {
            let output = "";
            for (const character of String(value || "")) {
                const code = character.codePointAt(0);
                output += code >= 0x20 && code <= 0x7e
                    ? character
                    : character === "\n" || character === "\r" ||
                        character === "\t"
                        ? " "
                        : "?";
                if (output.length >= maximum) break;
            }
            return output;
        };
        const encodedTitle = new TextEncoder().encode(
            printableAscii(title, 40),
        );
        const encodedMessage = new TextEncoder().encode(
            printableAscii(message, 64),
        );
        if (
            !Number.isInteger(notificationKind) ||
            notificationKind < 0 ||
            notificationKind > 3 ||
            encodedTitle.byteLength === 0
        ) {
            throw new RangeError("invalid RiceWM notification");
        }
        const byteLength =
            encodedTitle.byteLength + encodedMessage.byteLength;
        const pointer = this.#reserve(byteLength);
        const bytes = new Uint8Array(this.memory.buffer, pointer, byteLength);
        bytes.set(encodedTitle);
        bytes.set(encodedMessage, encodedTitle.byteLength);
        if (
            this.exports.rice_wm_notification_show(
                notificationKind,
                pointer,
                encodedTitle.byteLength,
                pointer + encodedTitle.byteLength,
                encodedMessage.byteLength,
            ) !== 1
        ) {
            throw new RangeError("RiceWM rejected the notification");
        }
    }

    clearNotification() {
        if (this.exports.rice_wm_notification_clear() !== 1) {
            throw new Error("RiceWM failed to clear the notification");
        }
    }

    refreshViewableWindows() {
        const refreshed = this.exports.rice_refresh_viewable_windows() >>> 0;
        if (refreshed === RESULT_ERROR) {
            throw new Error("RiceWM failed to refresh mapped windows");
        }
        return refreshed;
    }

    parkNextManagedWindow() {
        return typeof this.exports.rice_wm_park_next_managed_window === "function" &&
            this.exports.rice_wm_park_next_managed_window() === 1;
    }

    cancelParkNextManagedWindow() {
        return typeof this.exports.rice_wm_park_cancel === "function" &&
            this.exports.rice_wm_park_cancel() === 1;
    }

    setManagedWindowResizable(window, resizable) {
        const id = Number(window) >>> 0;
        return id !== 0 &&
            typeof this.exports.rice_wm_managed_window_set_resizable === "function" &&
            this.exports.rice_wm_managed_window_set_resizable(
                id,
                resizable ? 1 : 0,
            ) === 1;
    }

    parkedManagedWindow() {
        if (typeof this.exports.rice_wm_parked_window !== "function") return 0;
        return this.exports.rice_wm_parked_window() >>> 0;
    }

    parkedManagedWindowOwner() {
        if (typeof this.exports.rice_wm_parked_window_owner !== "function") return 0;
        return this.exports.rice_wm_parked_window_owner() >>> 0;
    }

    activateParkedManagedWindow(window) {
        const id = Number(window) >>> 0;
        return id !== 0 &&
            typeof this.exports.rice_wm_parked_window_activate === "function" &&
            this.exports.rice_wm_parked_window_activate(id) === 1;
    }

    setDesktopHidden(hidden) {
        return this.exports.rice_wm_desktop_hidden?.(hidden ? 1 : 0) === 1;
    }

    restoreWorkspaceWindows() {
        return this.exports.rice_wm_restore_workspace_windows?.() === 1;
    }

    windowTaskCapabilities() {
        return {
            snapshot: typeof this.exports.rice_wm_search_windows === 'function',
            activate: typeof this.exports.rice_wm_overview_activate === 'function',
            fields: ['id', 'title', 'workspace', 'minimized'],
        };
    }

    windowTaskSnapshot() {
        const capabilities = this.windowTaskCapabilities();
        if (!capabilities.snapshot) throw new Error('X11 window snapshot is unavailable');
        return {capabilities, windows: this.searchWindows()};
    }

    searchWindows() {
        const snapshot = this.exports.rice_wm_search_windows;
        if (typeof snapshot !== 'function') return [];
        const length = snapshot(0, 0) >>> 0;
        if (length === RESULT_ERROR) throw Error('Window search is unavailable');
        if (!length) return [];
        const pointer = this.#reserve(length);
        if ((snapshot(pointer, length) >>> 0) !== length) throw Error('Window catalog changed');
        const view = new DataView(this.memory.buffer, pointer, length), result = [];
        const decoder = new TextDecoder();
        for (let offset = 0; offset < length;) {
            if (offset + 16 > length) throw Error('Invalid window catalog');
            const id = view.getUint32(offset, true), workspace = view.getUint32(offset + 4, true);
            const minimized = view.getUint32(offset + 8, true) !== 0, size = view.getUint32(offset + 12, true);
            offset += 16;
            if (size > length - offset) throw Error('Invalid window title');
            const title = decoder.decode(new Uint8Array(this.memory.buffer, pointer + offset, size));
            offset += size;
            result.push({id, workspace, minimized, title});
        }
        return result;
    }

    spatialWindowSnapshot() {
        const snapshot=this.exports.rice_wm_spatial_windows;
        if(typeof snapshot!=='function')return null;
        const length=snapshot(0,0)>>>0;
        if(length===RESULT_ERROR||length<8||length>8+1024*24)throw Error('X11 spatial metadata unavailable');
        const pointer=this.#reserve(length);
        if((snapshot(pointer,length)>>>0)!==length)throw Error('X11 spatial metadata changed');
        const view=new DataView(this.memory.buffer,pointer,length),count=view.getUint32(4,true);
        if(view.getUint32(0,true)!==1||count>1024||length!==8+count*24)throw Error('Invalid X11 spatial metadata');
        const windows=[];
        for(let offset=8;offset<length;offset+=24){
            const client=view.getUint32(offset,true),frame=view.getUint32(offset+4,true),owner=view.getUint32(offset+8,true),rootClient=view.getUint32(offset+12,true),workspace=view.getUint32(offset+16,true),flags=view.getUint32(offset+20,true);
            if(!client||!frame||!owner||flags>15)throw Error('Invalid X11 spatial owner');
            windows.push({client,frame,owner,rootClient,workspace,overrideRedirect:!!(flags&1),viewable:!!(flags&2),minimized:!!(flags&4),parked:!!(flags&8)});
        }
        return {schema:'rice-x11-spatial/v1',windows};
    }

    overviewClient(window) {
        if(!Number.isInteger(window)||window<1||window>0xffffffff)return 0;
        return this.exports.rice_wm_overview_client?.(window)>>>0;
    }

    activateOverviewWindow(window) {
        const id = Number(window) >>> 0;
        return id !== 0 &&
            typeof this.exports.rice_wm_overview_activate === "function" &&
            this.exports.rice_wm_overview_activate(id) === 1;
    }

    closeOverviewWindow(window) {
        const id = Number(window) >>> 0;
        return id !== 0 &&
            typeof this.exports.rice_wm_overview_close === "function" &&
            this.exports.rice_wm_overview_close(id) === 1;
    }

    discardParkedManagedWindow(window) {
        const id = Number(window) >>> 0;
        return id !== 0 &&
            typeof this.exports.rice_wm_parked_window_discard === "function" &&
            this.exports.rice_wm_parked_window_discard(id) === 1;
    }

    setThemeMenuRowHeight(height=0){
        if(!Number.isInteger(height)||(height!==0&&(height<16||height>72)))throw RangeError('Invalid native menu row height');
        const setter=this.exports.rice_wm_theme_menu_row_height_set;
        if(!setter){if(height)throw Error('Updated shell engine is required for menu metrics');return;}
        if(setter(height)!==1)throw Error('Native shell rejected menu row height');
    }

    setMenuApplications(applications) {
        const encoder = new TextEncoder();
        const structured = typeof this.exports.rice_wm_menu_replace_v2 === "function";
        this.menuApplications = applications.map(application => ({...application,
            menuPath: application.menuPath ? [...application.menuPath] : undefined}));
        const records = applications.map(source => {
            const application = localizedMenuApplication(source);
            const action = application.action >>> 0;
            const icon = application.icon >>> 0;
            const category = application.category >>> 0;
            const label = encoder.encode(String(application.label || ""));
            const menuPath = Array.isArray(application.menuPath)
                ? application.menuPath.map(part => String(part || "").trim())
                    .filter(Boolean).slice(0, 8)
                : [];
            const path = encoder.encode(menuPath.join("\0"));
            const flags = (application.uninstallable === true ? 1 : 0) |
                (application.provisionable === true ? 2 : 0) |
                (application.hydrating === true ? 4 : 0);
            if (
                action === 0 ||
                icon > 0xff ||
                category > 9 ||
                category > 9 ||
                label.byteLength === 0 ||
                label.byteLength > 48 ||
                path.byteLength > 384 ||
                menuPath.some(part => /[\0/]/.test(part))
            ) {
                throw new RangeError("invalid RiceWM application-menu entry");
            }
            if (!structured) {
                const record = new Uint8Array(8 + label.byteLength);
                const view = new DataView(record.buffer);
                view.setUint32(0, action, true);
                view.setUint8(4, icon);
                view.setUint8(5, label.byteLength);
                view.setUint8(6, category);
                record.set(label, 8);
                return record;
            }
            const record = new Uint8Array(10 + label.byteLength + path.byteLength);
            const view = new DataView(record.buffer);
            view.setUint32(0, action, true);
            view.setUint8(4, icon);
            view.setUint8(5, label.byteLength);
            view.setUint16(6, path.byteLength, true);
            view.setUint16(8, flags, true);
            record.set(label, 10);
            record.set(path, 10 + label.byteLength);
            return record;
        });
        const byteLength = records.reduce(
            (total, record) => total + record.byteLength,
            0,
        );
        let pointer = 0;
        if (byteLength > 0) {
            pointer = this.#reserve(byteLength);
            const output = new Uint8Array(
                this.memory.buffer,
                pointer,
                byteLength,
            );
            let offset = 0;
            for (const record of records) {
                output.set(record, offset);
                offset += record.byteLength;
            }
        }
        const replace = structured
            ? this.exports.rice_wm_menu_replace_v2
            : this.exports.rice_wm_menu_replace;
        if (replace(pointer, byteLength) !== 1) {
            throw new RangeError("RiceWM rejected the application menu");
        }
    }

    clearDockHover() {
        this.exports.rice_wm_dock_hover_clear?.();
    }

    dismissMenu() {
        if (this.exports.rice_wm_menu_dismiss() !== 1) {
            throw new Error("RiceWM could not dismiss the shell menu");
        }
    }

    setThemes(catalog) {
        if (
            catalog?.schema !== "ricewm-theme-catalog-v1" ||
            !Array.isArray(catalog.themes) ||
            catalog.themes.length < 1 ||
            catalog.themes.length > 32
        ) {
            throw new RangeError("invalid RiceWM theme catalog");
        }
        this.themeCornerRadii = catalog.themes.map(theme => theme.cornerRadius ?? 0);
        const encoder = new TextEncoder();
        const names = new Set();
        const records = catalog.themes.map(theme => {
            const name = String(theme?.name || "");
            if (
                !/^[A-Za-z0-9][A-Za-z0-9 ._()+&'-]{0,47}$/.test(name) ||
                names.has(name.toLocaleLowerCase("en-US"))
            ) {
                throw new RangeError("invalid or duplicate RiceWM theme name");
            }
            names.add(name.toLocaleLowerCase("en-US"));
            const groups = [
                [theme.taskbar, 7],
                [theme.menu, 6],
                [theme.frame, 8],
            ];
            for (const [colors, expected] of groups) {
                if (
                    !Array.isArray(colors) ||
                    colors.length !== expected ||
                    colors.some(color =>
                        !Number.isInteger(color) ||
                        color < 0 ||
                        color > 0x00ffffff
                    )
                ) {
                    throw new RangeError("invalid RiceWM theme RGB palette");
                }
            }
            if (
                !Number.isInteger(theme.icon) ||
                theme.icon < 0 ||
                theme.icon > 0x00ffffff
            ) {
                throw new RangeError("invalid RiceWM theme icon color");
            }
            if (
                !Number.isInteger(theme.iconHover) ||
                theme.iconHover < 0 ||
                theme.iconHover > 0x00ffffff
            ) {
                throw new RangeError("invalid RiceWM theme icon hover color");
            }
            const encodedName = encoder.encode(name);
            const colors = groups.flatMap(([values]) => values);
            const record = new Uint8Array(4 + colors.length * 4 + encodedName.byteLength);
            const view = new DataView(record.buffer);
            const backdropMaterial = [
                "backdrop-blur", "frosted-ice", "liquid-glass",
            ].includes(
                theme?.material,
            ) || [
                "glass.theme",
                "frosted-ice.theme",
                "emerald.theme",
                "orchard.theme",
            ].includes(theme?.file);
            view.setUint8(
                0,
                encodedName.byteLength | (backdropMaterial ? 0x80 : 0),
            );
            const profileCode = theme?.controlLayout === "split" ? 6 : new Map([
                ["standard", 0],
                // XP uses standard control geometry; the browser paints its Luna frame.
                ["redmond-xp", 0],
                ["familiar", 1],
                ["redmond-98", 7],
                ["yellow-jacket", 2],
                ["command-deck", 3],
                ["orchard", 4],
                ["neon-grid", 5],
            ]).get(theme?.chromeProfile);
            if (!Number.isInteger(profileCode)) {
                throw new RangeError("invalid RiceWM theme chrome profile");
            }
            view.setUint8(
                1,
                7 | (theme?.menuMaterial === "glass" ? 0x08 : 0) |
                    (profileCode << 4),
            );
            view.setUint8(2, 6);
            view.setUint8(3, 8);
            colors.forEach((color, index) => {
                view.setUint32(4 + index * 4, color, true);
            });
            record.set(encodedName, 4 + colors.length * 4);
            return record;
        });
        const byteLength = records.reduce(
            (total, record) => total + record.byteLength,
            0,
        );
        const pointer = this.#reserve(byteLength);
        const output = new Uint8Array(this.memory.buffer, pointer, byteLength);
        let offset = 0;
        for (const record of records) {
            output.set(record, offset);
            offset += record.byteLength;
        }
        if (this.exports.rice_wm_themes_replace(pointer, byteLength) !== 1) {
            throw new RangeError("RiceWM rejected the theme catalog");
        }
    }

    setDockCornerRadius(radius) {
        const value = Math.max(0, Math.min(32, Math.round(Number(radius) || 0)));
        if (this.exports.rice_wm_dock_corner_radius_set && this.exports.rice_wm_dock_corner_radius_set(value) !== 1) throw new Error('Dock corner radius rejected');
    }

    selectTheme(index) {
        const selected = Number(index);
        if (!Number.isInteger(selected) || selected < 0 || selected > 31) {
            throw new RangeError("invalid RiceWM theme index");
        }
        if (this.exports.rice_wm_theme_select(selected) !== 1) {
            throw new RangeError("RiceWM rejected the selected theme");
        }
        this.setDockCornerRadius(this.themeCornerRadii?.[selected] ?? 0);
    }

    drainWmActions() {
        const pending = this.exports.rice_wm_action_pending() >>> 0;
        if (pending === RESULT_ERROR) {
            throw new Error("RiceWM action queue is not initialized");
        }
        const actions = [];
        for (let index = 0; index < pending; index += 1) {
            const action = this.exports.rice_wm_action_drain() >>> 0;
            if (action === 0) break;
            if (action === 0xffff_ff12 && this.exports.rice_wm_relaunch_command_drain) {
                const pointer=this.#reserve(4096);
                const length=this.exports.rice_wm_relaunch_command_drain(pointer,4096) >>> 0;
                const command=new TextDecoder().decode(new Uint8Array(this.memory.buffer,pointer,Math.min(length,4096)));
                actions.push({kind:"new-instance",argv:command.split("\0").filter(Boolean)});
            } else actions.push(action);
        }
        return actions;
    }

    clientStatus(client) {
        return this.exports.rice_client_status(client) >>> 0;
    }

    feed(client, bytes) {
        const source = bytes instanceof Uint8Array
            ? bytes
            : new Uint8Array(bytes);
        if (source.byteLength === 0) return 0;
        const pointer = this.#reserve(source.byteLength);
        new Uint8Array(
            this.memory.buffer,
            pointer,
            source.byteLength,
        ).set(source);
        const consumed = this.exports.rice_client_feed(
            client,
            pointer,
            source.byteLength,
        ) >>> 0;
        if (consumed === RESULT_ERROR) {
            throw new Error("RiceWM rejected the client byte stream");
        }
        return consumed;
    }

    drain(client) {
        const pending = this.exports.rice_client_pending(client) >>> 0;
        if (pending === RESULT_ERROR) {
            throw new Error("unknown RiceWM client");
        }
        if (pending === 0) return new Uint8Array();
        const pointer = this.#reserve(pending);
        const written = this.exports.rice_client_drain(
            client,
            pointer,
            pending,
        ) >>> 0;
        if (written === RESULT_ERROR || written > pending) {
            throw new Error("RiceWM failed to drain client output");
        }
        return new Uint8Array(
            this.memory.buffer,
            pointer,
            written,
        ).slice();
    }

    drainScene() {
        const pending = this.exports.rice_scene_pending() >>> 0;
        if (pending === RESULT_ERROR) {
            throw new Error("RiceWM scene is not initialized");
        }
        if (pending === 0) return new Uint8Array();
        const pointer = this.#reserve(pending);
        const written = this.exports.rice_scene_drain(
            pointer,
            pending,
        ) >>> 0;
        if (written === RESULT_ERROR || written > pending) {
            throw new Error("RiceWM failed to drain renderer commands");
        }
        return new Uint8Array(
            this.memory.buffer,
            pointer,
            written,
        ).slice();
    }

    shellTexts() {
        const pending = this.exports.rice_shell_text_snapshot_len() >>> 0;
        if (pending === RESULT_ERROR) {
            throw new Error("RiceWM shell-text snapshot is unavailable");
        }
        if (pending === 0) return [];
        const pointer = this.#reserve(pending);
        const written = this.exports.rice_shell_text_snapshot(
            pointer,
            pending,
        ) >>> 0;
        if (written === RESULT_ERROR || written !== pending) {
            throw new Error("RiceWM failed to serialize shell text");
        }
        const bytes = new Uint8Array(
            this.memory.buffer,
            pointer,
            written,
        );
        const view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
        );
        const decoder = new TextDecoder();
        const overlays = [];
        let offset = 0;
        while (offset < bytes.byteLength) {
            if (offset + 24 > bytes.byteLength) {
                throw new RangeError("truncated RiceWM shell-text header");
            }
            const textLength = view.getUint8(offset + 15);
            const recordLength = (24 + textLength + 3) & ~3;
            if (offset + recordLength > bytes.byteLength) {
                throw new RangeError("truncated RiceWM shell-text record");
            }
            const rawAlign = view.getUint8(offset + 14);
            const hasMenuIconMetadata = (rawAlign & 0x40) !== 0;
            const menuIconSizes = [16, 24, 32, 64];
            const menuIconKinds = ["label", "image", "folder", "label"];
            overlays.push({
                id: view.getUint32(offset, true),
                x: view.getUint16(offset + 4, true),
                y: view.getUint16(offset + 6, true),
                width: view.getUint16(offset + 8, true),
                height: view.getUint16(offset + 10, true),
                fontPx: view.getUint8(offset + 12),
                weight: view.getUint8(offset + 13) * 100,
                align: ["left", "center", "right"][rawAlign & 0x03] || "left",
                ...(rawAlign === 3 ? {role: "window-title"} : {}),
                rotateCcw: (rawAlign & 0x80) !== 0,
                menuIconSize: hasMenuIconMetadata
                    ? menuIconSizes[(rawAlign >>> 2) & 0x03]
                    : 0,
                menuIconKind: hasMenuIconMetadata
                    ? menuIconKinds[(rawAlign >>> 4) & 0x03]
                    : null,
                iconFallbackOmitted: hasMenuIconMetadata,
                foreground: view.getUint32(offset + 16, true),
                background: view.getUint32(offset + 20, true),
                text: decoder.decode(
                    bytes.subarray(offset + 24, offset + 24 + textLength),
                ),
            });
            offset += recordLength;
        }
        return overlays;
    }

    surfaceGeometry(drawable) {
        const packed = this.exports.rice_surface_geometry(drawable) >>> 0;
        if (packed === 0) return null;
        return {
            width: packed & 0xffff,
            height: packed >>> 16,
        };
    }

    readSurface(drawable, x, y, width, height, borrow = false, arenaOffset = 0, arenaBytes = width * height * 4) {
        const byteLength = width * height * 4;
        if (!Number.isSafeInteger(byteLength) || byteLength <= 0) {
            throw new RangeError("invalid RiceWM surface rectangle");
        }
        if (!Number.isSafeInteger(arenaOffset) || arenaOffset < 0 ||
            !Number.isSafeInteger(arenaBytes) || arenaBytes < arenaOffset + byteLength) {
            throw new RangeError("invalid RiceWM surface arena range");
        }
        const pointer = this.#reserve(arenaBytes) + arenaOffset;
        const written = this.exports.rice_surface_read(
            drawable,
            x,
            y,
            width,
            height,
            pointer,
            byteLength,
        ) >>> 0;
        if (written === RESULT_ERROR || written !== byteLength) {
            throw new Error("RiceWM failed to read drawable pixels");
        }
        const bytes = new Uint8Array(
            this.memory.buffer,
            pointer,
            written,
        );
        // Borrowed output is valid only until the next display Wasm call that
        // uses the transfer arena or grows memory. The desktop service enables
        // it solely for a single upload consumed synchronously by the renderer.
        return borrow ? bytes : bytes.slice();
    }

    writeSurfaceBgra(drawable, x, y, width, height, stride, pixels) {
        const source = pixels instanceof Uint8Array
            ? pixels
            : new Uint8Array(pixels || 0);
        let rowBytes = width * 4;
        const required = (height - 1) * stride + rowBytes;
        if (
            !Number.isSafeInteger(x) || x < 0 || x > 0xffff ||
            !Number.isSafeInteger(y) || y < 0 || y > 0xffff ||
            !Number.isSafeInteger(width) || width <= 0 || width > 0xffff ||
            !Number.isSafeInteger(height) || height <= 0 || height > 0xffff ||
            !Number.isSafeInteger(stride) || stride < rowBytes ||
            !Number.isSafeInteger(required) || required <= 0 ||
            source.byteLength < required
        ) {
            throw new RangeError("invalid RiceWM BGRA surface upload");
        }

        // Match write_surface_bgra's drawable clipping BEFORE splitting the
        // transfer. After a resize, the first band can overlap the drawable
        // while a later band starts below it and is rejected by the core.
        // Validate the full original source above; clipping must never make a
        // truncated buffer valid. Keep its original stride for every row.
        const geometry = this.surfaceGeometry(drawable);
        if (!geometry || x >= geometry.width || y >= geometry.height) {
            throw new RangeError("RiceWM BGRA upload has no drawable intersection");
        }
        width = Math.min(width, geometry.width - x);
        height = Math.min(height, geometry.height - y);
        rowBytes = width * 4;

        // Keep the reusable Wasm transfer arena bounded while allowing a
        // drawable to grow to the compositor's real work area. A maximized
        // 1262x834 BGRA client is already slightly larger than 4 MiB; rejecting
        // that complete application paint made ordinary window resizing an
        // accidental process-memory limit. Preserve source stride semantics
        // and submit the same authoritative pixels in bounded row bands.
        const rowsPerTransfer = Math.max(
            1,
            Math.floor((MAX_SURFACE_UPLOAD_BYTES - rowBytes) / stride) + 1,
        );
        for (let row = 0; row < height; row += rowsPerTransfer) {
            const transferHeight = Math.min(rowsPerTransfer, height - row);
            const transferBytes =
                (transferHeight - 1) * stride + rowBytes;
            const sourceOffset = row * stride;
            const pointer = this.#reserve(transferBytes);
            new Uint8Array(
                this.memory.buffer,
                pointer,
                transferBytes,
            ).set(source.subarray(sourceOffset, sourceOffset + transferBytes));
            if (
                this.exports.rice_surface_write_bgra(
                    drawable,
                    x,
                    y + row,
                    width,
                    transferHeight,
                    stride,
                    pointer,
                    transferBytes,
                ) !== 1
            ) {
                const geometry = this.surfaceGeometry(drawable);
                throw new Error(
                    "RiceWM rejected the BGRA surface upload " +
                    `drawable=0x${drawable.toString(16)} x=${x} y=${y + row} ` +
                    `width=${width} height=${transferHeight} stride=${stride} ` +
                    `surface=${geometry ? `${geometry.width}x${geometry.height}` : "missing"}`,
                );
            }
        }
    }

    surfaceUploads(commands, borrowSingle = false) {
        const bytes = commands instanceof Uint8Array
            ? commands
            : new Uint8Array(commands);
        const damage = new Map();
        const view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
        );
        for (let offset = 0; offset + 32 <= bytes.byteLength; offset += 32) {
            if (view.getUint8(offset) !== 3) continue;
            const id = view.getUint32(offset + 4, true);
            const x = view.getInt32(offset + 12, true);
            const y = view.getInt32(offset + 16, true);
            const width = view.getUint16(offset + 20, true);
            const height = view.getUint16(offset + 22, true);
            if (width === 0 || height === 0) continue;
            let entry = damage.get(id);
            if (!entry) {
                const geometry = this.surfaceGeometry(id);
                if (!geometry) continue;
                entry = { geometry, regions: [] };
                damage.set(id, entry);
            }
            const left = Math.max(0, Math.min(entry.geometry.width, x));
            const top = Math.max(0, Math.min(entry.geometry.height, y));
            const right = Math.max(left, Math.min(entry.geometry.width, x + width));
            const bottom = Math.max(top, Math.min(entry.geometry.height, y + height));
            if (left === right || top === bottom) continue;
            addDamageRegion(entry.regions, { left, top, right, bottom });
        }
        const uploads = [];
        for (const [id, { regions }] of damage) {
            for (const rectangle of regions) {
                const { left: x, top: y, right, bottom } = rectangle;
                const width = right - x;
                const rowsPerUpload = Math.max(
                    1,
                    Math.floor((borrowSingle ? MAX_BORROWED_SURFACE_UPLOAD_BYTES : MAX_SURFACE_UPLOAD_BYTES) / (width * 4)),
                );
                for (let uploadY = y; uploadY < bottom; uploadY += rowsPerUpload) {
                    const uploadHeight = Math.min(rowsPerUpload, bottom - uploadY);
                    uploads.push({
                        id,
                        x,
                        y: uploadY,
                        width,
                        height: uploadHeight,
                    });
                }
            }
        }
        // Read all synchronous uploads into disjoint regions of one bounded
        // arena. A small second damage rectangle must not force a full video
        // frame to be copied out of Wasm. Larger batches keep owned snapshots.
        const totalBytes = uploads.reduce((sum, upload) =>
            sum + upload.width * upload.height * 4, 0);
        const borrow = borrowSingle && totalBytes <= MAX_BORROWED_SURFACE_UPLOAD_BYTES;
        let arenaOffset = 0;
        for (const upload of uploads) {
            const bytes = upload.width * upload.height * 4;
            upload.pixels = this.readSurface(
                upload.id, upload.x, upload.y, upload.width, upload.height,
                borrow, borrow ? arenaOffset : 0, borrow ? totalBytes : bytes,
            );
            arenaOffset += bytes;
        }
        return uploads;
    }

    fullSurfaceUploads(commands) {
        const bytes = commands instanceof Uint8Array
            ? commands
            : new Uint8Array(commands);
        const view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
        );
        const drawables = new Set();
        for (let offset = 0; offset + 32 <= bytes.byteLength; offset += 32) {
            const opcode = view.getUint8(offset);
            if (opcode === 1) {
                const flags = view.getUint8(offset + 1);
                if ((flags & 4) !== 0) continue;
                drawables.add(view.getUint32(offset + 4, true));
            } else if (opcode === 3) {
                // A retained follow-up batch can consist only of damage
                // records after the full window graph was posted. Snapshot
                // the complete affected drawable, not just its latest rect.
                drawables.add(view.getUint32(offset + 4, true));
            }
        }
        const uploads = [];
        for (const id of drawables) {
            const geometry = this.surfaceGeometry(id);
            if (!geometry || geometry.width < 1 || geometry.height < 1) {
                continue;
            }
            const rowsPerUpload = Math.max(
                1,
                Math.floor(
                    MAX_SURFACE_UPLOAD_BYTES / (geometry.width * 4),
                ),
            );
            for (
                let y = 0;
                y < geometry.height;
                y += rowsPerUpload
            ) {
                const height = Math.min(
                    rowsPerUpload,
                    geometry.height - y,
                );
                uploads.push({
                    id,
                    x: 0,
                    y,
                    width: geometry.width,
                    height,
                    pixels: this.readSurface(
                        id,
                        0,
                        y,
                        geometry.width,
                        height,
                    ),
                });
            }
        }
        return uploads;
    }

    drainUnsupported() {
        const requests = [];
        for (;;) {
            const packed = this.exports.rice_unsupported_next() >>> 0;
            if (packed === RESULT_ERROR) {
                throw new Error("RiceWM diagnostics are not initialized");
            }
            if (packed === 0) return requests;
            requests.push({
                opcode: packed & 0xff,
                count: packed >>> 8,
            });
        }
    }

    drainRequestCounts() {
        const requests = [];
        for (;;) {
            const packed = this.exports.rice_request_count_next() >>> 0;
            if (packed === RESULT_ERROR) {
                throw new Error("RiceWM request diagnostics are unavailable");
            }
            if (packed === 0) return requests;
            requests.push({
                opcode: packed & 0xff,
                count: packed >>> 8,
            });
        }
    }

    drainPresentedWindows() {
        const windows = [];
        for (;;) {
            const window = this.exports.rice_presented_window_next() >>> 0;
            if (window === RESULT_ERROR) {
                throw new Error(
                    "RiceWM presentation diagnostics are unavailable",
                );
            }
            if (window === 0) return windows;
            windows.push(window);
        }
    }

    #uploadGnomePixels(exportName, arguments_, pixels) {
        const source = pixels instanceof Uint32Array ? pixels : new Uint32Array(pixels || []);
        if (source.length === 0 || source.length > 128 * 128) {
            throw new RangeError("invalid GNOME extension icon pixels");
        }
        const pointer = this.#reserve(source.byteLength);
        new Uint8Array(this.memory.buffer, pointer, source.byteLength)
            .set(new Uint8Array(source.buffer, source.byteOffset, source.byteLength));
        if (this.exports[exportName](...arguments_, pointer, source.length) !== 1) {
            throw new RangeError("RiceWM rejected the GNOME extension icon");
        }
        return true;
    }

    #reserve(capacity) {
        const pointer = this.exports.rice_io_reserve(capacity) >>> 0;
        if (pointer === 0 || this.exports.rice_io_capacity() < capacity) {
            throw new RangeError("RiceWM staging-buffer limit exceeded");
        }
        return pointer;
    }
}
