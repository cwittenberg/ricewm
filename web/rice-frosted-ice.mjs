import { HYPRGLASS_GLSL } from "./rice-hyprglass-shaders.mjs";
import { RiceWebGpuGlass } from "./rice-frosted-ice-webgpu.mjs";
import { resizeCanvasBackingStore } from "./rice-native-canvas-primitives.mjs";
// Reusable compositor material: live backdrop capture, optical refraction,
// spectral rim lighting and a deterministic Canvas2D fallback.

import {
    createRiceVisualEffectWebGl2Context,
} from "./rice-visual-effect-capability.mjs";

const VERTEX_SHADER = `#version 300 es
in vec2 a_position;
out vec2 v_uv;
void main() {
    v_uv = vec2(a_position.x * .5 + .5, 1. - (a_position.y * .5 + .5));
    gl_Position = vec4(a_position, 0., 1.);
}`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;
uniform sampler2D u_backdrop;
uniform vec2 u_root;
uniform vec2 u_origin;
uniform vec2 u_size;
uniform float u_radius;
uniform float u_thickness;
uniform float u_opacity;
uniform float u_blur;
uniform float u_refraction;
uniform float u_chroma;
uniform vec3 u_tint;
uniform float u_time;
uniform vec2 u_light;
uniform float u_pointer_light;
uniform float u_hyprglass;
in vec2 v_uv;
out vec4 out_color;

float roundedDistance(vec2 p) {
    vec2 halfSize = u_size * .5;
    vec2 q = abs(p) - max(halfSize - vec2(u_radius), vec2(0.));
    return length(max(q, 0.)) + min(max(q.x, q.y), 0.) - u_radius;
}
float surfaceHeight(vec2 p) {
    float depth = clamp(-roundedDistance(p) / max(u_thickness, 1.), 0., 1.);
    float n = 3.4;
    return pow(max(1. - pow(1. - depth, n), 0.), 1. / n);
}
vec3 transmitted(vec2 uv, float diffusion) {
    vec2 texel = 1. / max(u_root, vec2(1.));
    // Match the Canvas fallback's default diffusion radius. The old GPU path
    // sampled only ~3.7 px away while Canvas used ~10.7 px, which left
    // displaced one-pixel window silhouettes sharply legible on hardware.
    float radius = 2. + diffusion * 24.;
    float inner = radius * .45;
    vec3 c = texture(u_backdrop, uv).rgb * .04;
    // Rotate the taps half a step so horizontal and vertical one-pixel edges
    // are not repeatedly sampled along their own axis. Two circular rings
    // retain form while diffusing the sharp duplicate underneath the glass.
    for (int index = 0; index < 8; index++) {
        float angle = (float(index) + .5) * .78539816339;
        vec2 direction = vec2(cos(angle), sin(angle));
        c += texture(u_backdrop, clamp(uv + direction * texel * inner, .001, .999)).rgb * .04;
        c += texture(u_backdrop, clamp(uv + direction * texel * radius, .001, .999)).rgb * .08;
    }
    return c;
}
${HYPRGLASS_GLSL}
void main() {
    vec2 local = v_uv * u_size;
    vec2 p = local - u_size * .5;
    float distance = roundedDistance(p);
    float mask = 1. - smoothstep(-1., 1., distance);
    if (mask <= 0.) { out_color = vec4(0.); return; }
    if (u_hyprglass > .5) { out_color = hyprGlass(local, distance, mask); return; }

    float e = 1.25;
    vec2 gradient = vec2(
        surfaceHeight(p + vec2(e, 0.)) - surfaceHeight(p - vec2(e, 0.)),
        surfaceHeight(p + vec2(0., e)) - surfaceHeight(p - vec2(0., e))
    ) / (2. * e);
    float depth = clamp(-distance / max(u_thickness, 1.), 0., 1.);
    float edge = 1. - smoothstep(.18, .96, depth);
    vec3 normal = normalize(vec3(-gradient * u_thickness * .32, 1.));
    vec3 ray = refract(vec3(0., 0., -1.), normal, 1. / 1.46);
    vec2 displacement = ray.xy / max(-ray.z, .18) * edge *
        (8. + u_refraction * 34.) / max(u_root, vec2(1.));
    vec2 baseUv = (u_origin + local) / u_root;
    vec2 uv = clamp(baseUv + displacement, .002, .998);

    // Preserve recognizable detail through the center and diffuse only the
    // curved thickness. A second, counter-displaced sample gives the rim the
    // compression visible in a real lens instead of a uniform frosted blur.
    float diffusion = .08 + u_blur * .46 + edge * .20;
    vec3 glass = transmitted(uv, diffusion);
    if (edge > 0.) glass = mix(glass, transmitted(clamp(baseUv - displacement * .38, .002, .998), diffusion * .72), edge * .20);
    if (edge > 0. && u_chroma > 0.) {
    vec2 dispersion = normalize(
        displacement + normal.xy / max(u_root, vec2(1.)) + vec2(.000001)
    ) *
        edge * (1. + u_chroma * 7.) / max(u_root, vec2(1.));
    vec3 dispersed = glass;
    dispersed.r = texture(u_backdrop, clamp(uv + dispersion, .002, .998)).r;
    dispersed.b = texture(u_backdrop, clamp(uv - dispersion, .002, .998)).b;
    glass = mix(glass, dispersed, edge * u_chroma * .28);
    }

    float luma = dot(glass, vec3(.2126, .7152, .0722));
    glass = mix(vec3(luma), glass, 1.10 + u_blur * .22);
    float tint = (.035 + u_opacity * .16) * (.72 + luma * .28);
    glass = mix(glass, u_tint, tint);

    // One desktop-space light ties separate panels into the same scene. Its
    // slow drift is deliberately tiny: the material should feel alive, not
    // animated. Moving windows and backdrops produce the larger response.
    vec2 ambientLightPoint = vec2(
        .16 + sin(u_time * .11) * .025,
        .08 + cos(u_time * .09) * .018
    ) * u_root;
    vec2 lightPoint = mix(ambientLightPoint, u_light, u_pointer_light);
    vec2 toLight = (lightPoint - (u_origin + local)) /
        max(max(u_root.x, u_root.y), 1.);
    vec3 light = normalize(vec3(toLight * 1.9, .72));
    vec3 view = vec3(0., 0., 1.);
    float facing = max(dot(normal, view), 0.);
    float fresnel = pow(1. - facing, 2.2);
    float specular = pow(max(dot(normalize(light + view), normal), 0.), 88.);
    float broadReflection = pow(max(dot(normalize(light + view), normal), 0.), 9.);
    float litRim = pow(max(dot(normal.xy, normalize(toLight + vec2(.0001))), 0.), 2.4);
    float outerLine = exp(-abs(distance) * 1.15);
    float innerLine = exp(-abs(distance + u_thickness * .56) * .31);
    float caustic = innerLine * litRim * (.35 + u_refraction * .65);
    float orientation = dot(normal.xy, normalize(vec2(-.72, -.48)));
    vec3 spectrum = vec3(
        max(-orientation, 0.),
        .30 + (1. - abs(orientation)) * .38,
        max(orientation, 0.)
    ) * edge * u_chroma * (.06 + fresnel * .24);
    vec3 reflection = vec3(.72, .88, 1.) *
        (specular * .34 + broadReflection * edge * .055);
    reflection += vec3(.42, .78, 1.) * caustic * .18;
    reflection += vec3(1.) * outerLine * (.055 + litRim * .12);
    reflection += vec3(.62, .83, .92) * innerLine * .035;
    float oppositeShade = edge * pow(max(dot(-normal.xy, normalize(toLight + vec2(.0001))), 0.), 2.) * .09;
    vec3 color = glass * (1. - oppositeShade) + reflection + spectrum;
    // The backdrop is already transmitted into the color; interior alpha must
    // replace the sharp source pixels beneath the effect. Leaving 6-18% of
    // the original canvas visible caused a crisp duplicate of every covered
    // window edge. Only the antialiased silhouette mask remains translucent.
    float alpha = mask;
    // This WebGL canvas is composited as premultiplied alpha. Premultiplying
    // the antialiased SDF edge prevents bright RGB from leaking through its
    // nearly transparent corner pixels as a white fringe.
    out_color = vec4(color * alpha, alpha);
}`;

let pointerLightPoint = null;
const NO_BACKDROP_EPOCH = Symbol("no-frosted-backdrop-epoch");
const MAX_SOFTWARE_BACKDROPS = 8;

export function setFrostedIcePointerLight(x, y) {
    const next = Number.isFinite(x) && Number.isFinite(y)
        ? Object.freeze({ x: Number(x), y: Number(y) })
        : null;
    if (
        next?.x === pointerLightPoint?.x &&
        next?.y === pointerLightPoint?.y &&
        Boolean(next) === Boolean(pointerLightPoint)
    ) return false;
    pointerLightPoint = next;
    return true;
}

function defaultCanvas(width, height) {
    if (typeof globalThis.OffscreenCanvas === "function") {
        return new globalThis.OffscreenCanvas(width, height);
    }
    const canvas = globalThis.document?.createElement?.("canvas");
    if (!canvas) return null;
    canvas.width = width;
    canvas.height = height;
    return canvas;
}

function percent(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) ? Math.max(0, Math.min(100, number)) : fallback;
}

function channel(value, fallback) {
    const number = Number(value);
    return Number.isFinite(number) ? Math.max(0, Math.min(255, number)) : fallback;
}

export function isFrostedIceTheme(theme) {
    return ["frosted-ice", "backdrop-blur", "liquid-glass"].includes(theme?.material) ||
        [
            "frosted-ice.theme",
            "glass.theme",
            "emerald.theme",
            "orchard.theme",
        ].includes(theme?.file);
}

// Only theme-owned reading planes transmit the backdrop; text, images,
// selection colors and small controls keep their original coverage.
export function hyprglassSurfaceCommand(command, chrome) {
    if (chrome?.frostedIce?.model !== "hyprglass" || command.type !== "fill" ||
        !(command.rect?.width * command.rect?.height > 12000) ||
        !Array.isArray(command.color) ||
        ![chrome.body, chrome.control, ...(chrome.frostedIce.surfaceColors || [])].some(color =>
            Array.isArray(color) && command.color.slice(0, 3).every((value, index) => value === color[index]))) return command;
    return { ...command, color: [...command.color.slice(0, 3), Math.min(command.color[3] ?? 255, 20)] };
}

export function frostedIceOptics(profile = {}) {
    const tint = Array.isArray(profile.tint) && profile.tint.length >= 3
        ? profile.tint
        : [9, 19, 23];
    return Object.freeze({
        hyprglass: profile.model === "hyprglass",
        thickness: 7 + percent(profile.thickness, 58) * .24,
        opacity: .16 + percent(profile.opacity, 42) * .005,
        blur: percent(profile.blur, 62) / 100,
        blurPixels: profile.model === "hyprglass" ? 4 * Math.SQRT2 * percent(profile.blur, 62) / 62 : 2 + percent(profile.blur, 62) * .14,
        refraction: profile.model === "hyprglass" ? 8 * percent(profile.refraction, 60) / 60 : percent(profile.refraction, 52) / 100,
        chroma: percent(profile.chroma, 18) / 100,
        rimWidth: percent(profile.rimWidth, 100) / 100,
        tintR: channel(tint[0], 9) / 255,
        tintG: channel(tint[1], 19) / 255,
        tintB: channel(tint[2], 23) / 255,
        neutral: profile.neutral === true,
        brightness: profile.model === "hyprglass" ? .68 : 1,
        saturation: profile.model === "hyprglass" ? .8 : profile.neutral === true ? 1 : 1.12 + percent(profile.blur, 62) * .0032,
        contrast: profile.model === "hyprglass" ? .9 : profile.neutral === true ? 1 : 1.04 + percent(profile.thickness, 58) * .0012,
        pointerLight: profile.model !== "hyprglass" && profile.pointerLight === true && pointerLightPoint !== null,
        lightX: profile.model === "hyprglass" ? 0 : pointerLightPoint?.x || 0,
        lightY: profile.model === "hyprglass" ? 0 : pointerLightPoint?.y || 0,
    });
}

function compile(gl, type, source) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        gl.deleteShader(shader);
        return null;
    }
    return shader;
}

export class FrostedIceMaterial {
    constructor({
        canvasFactory = defaultCanvas,
        accelerationMode = "auto",
        onStatus = () => {},
        surface = "glass",
    } = {}) {
        this.canvasFactory = canvasFactory;
        this.accelerationMode = ["auto", "hardware", "software"].includes(
            String(accelerationMode),
        ) ? String(accelerationMode) : "auto";
        this.onStatus = typeof onStatus === "function" ? onStatus : () => {};
        this.surface = String(surface || "glass").slice(0, 64);
        this.capture = null;
        this.captureSource = null;
        this.captureValid = false;
        this.captureRevision = 0;
        this.backdropEpoch = NO_BACKDROP_EPOCH;
        this.softwareBackdrops = new Map();
        this.performance = {
            captures: 0,
            captureReuses: 0,
            softwareBackdropBuilds: 0,
            softwareBackdropReuses: 0,
            gpuBackdropUploads: 0,
            gpuDraws: 0,
            gpuDrawReuses: 0,
        };
        this.effect = null;
        this.webgpu = null;
        this.gpu = null;
        this.gpuUnavailable = false;
        this.failed = "";
    }

    setWebGpuDevice(device, format = "bgra8unorm") {
        if (this.webgpu?.device === device && !this.webgpu.closed) return;
        this.webgpu?.dispose();
        this.webgpu = null;
        if (!device || this.accelerationMode === "software") return;
        this.webgpu = new RiceWebGpuGlass({
            device, format, canvasFactory: this.canvasFactory,
            onReady: () => {
                this.#disposeGpu(true);
                this.onStatus({
                    state: "ready", surface: this.surface, backend: "webgpu",
                    mode: this.accelerationMode, softwareFallback: false,
                });
            },
            onFailure: error => this.onStatus({
                state: "fallback", surface: this.surface, backend: "webgl2",
                reason: String(error.message || error),
            }),
        });
    }

    setAccelerationMode(mode) {
        const next = ["auto", "hardware", "software"].includes(String(mode))
            ? String(mode) : "software";
        if (next === this.accelerationMode) return;
        this.#disposeGpu(true);
        if (next === "software") {
            this.webgpu?.dispose();
            this.webgpu = null;
        }
        this.accelerationMode = next;
        this.gpuUnavailable = false;
        this.failed = "";
        this.onStatus({
            state: "configured",
            surface: this.surface,
            backend: next === "software" ? "fallback" : "webgl2",
            mode: next,
            softwareFallback: next === "software",
        });
    }

    // The compositor mutates one canvas while walking the window stack, so
    // source object identity cannot tell us whether its pixels are unchanged.
    // Callers opt in to retained backdrop work with an explicit epoch and
    // advance/invalidate it at real z-order or damage boundaries.
    beginBackdropEpoch(epoch) {
        if (Object.is(epoch, this.backdropEpoch)) return false;
        this.backdropEpoch = epoch;
        this.invalidateBackdrop();
        return true;
    }

    invalidateBackdrop() {
        this.captureValid = false;
        return true;
    }

    backdropPerformance() {
        return Object.freeze({ ...this.performance });
    }

    memorySnapshot() {
        // RGBA storage estimate; excludes driver buffers, MSAA and browser pools.
        const pixels = canvas => (canvas?.width || 0) * (canvas?.height || 0);
        const opticalBufferBytes = (this.softwareLens?.map?.byteLength || 0) + (this.softwareLens?.samples?.byteLength || 0) + (this.softwareLens?.indices?.byteLength || 0) + (this.softwareLens?.output?.data?.byteLength || 0);
        const canvasBytes = 4 * (pixels(this.capture) + pixels(this.effect) + pixels(this.softwareLens?.canvas) +
            [...(this.retainedLenses?.values() || [])].reduce((sum, canvas) => sum + pixels(canvas), 0) +
            [...this.softwareBackdrops.values()].reduce((sum, item) => sum + pixels(item.canvas), 0));
        const textureBytes = 4 * (this.gpu?.textureWidth || 0) * (this.gpu?.textureHeight || 0);
        const webgpu = this.webgpu?.memorySnapshot() || null;
        return {
            canvasBytes, textureBytes, webgpu,
            estimatedBytes: canvasBytes + textureBytes + opticalBufferBytes + (webgpu?.estimatedBytes || 0),
            opticalBufferBytes,
            softwareBackdrops: this.softwareBackdrops.size,
            gpuContexts: (this.gpu ? 1 : 0) + (this.webgpu?.context ? 1 : 0),
            backend: this.webgpu?.context ? "webgpu" : this.gpu?.executionMode || "canvas2d",
            adapter: this.gpu?.renderer || null,
        };
    }

    draw(context, source, rect, radius, profile = {}, now = 0, sampleRect = rect) {
        if (
            !context || !source || !rect || !sampleRect ||
            rect.width < 1 || rect.height < 1 ||
            sampleRect.width < 1 || sampleRect.height < 1
        ) return false;
        let optics = frostedIceOptics(profile);
        // Probe hardware once. Emulating the optical shader on the CPU costs
        // more than the bounded Canvas fallback, so auto uses that fallback.
        if (optics.hyprglass && !this.webgpu?.pipeline) this.#ensureGpu(rect.width, rect.height, true);
        // A tiled desktop can explicitly identify an unchanged underlay. Keep
        // its finished optical pixels across cursor/content updates, without
        // retaining text or skipping backdrop invalidation for floating panes.
        let retainedKey = null;
        if (optics.hyprglass && this.gpuUnavailable && !this.webgpu?.pipeline && profile.backdropCacheKey != null) {
            if (this.retainedLensSource !== source || this.retainedLensEpoch !== profile.backdropCacheKey) {
                this.retainedLenses = new Map();
                this.retainedLensSource = source;
                this.retainedLensEpoch = profile.backdropCacheKey;
            }
            retainedKey = JSON.stringify([rect.width,rect.height,radius,sampleRect,optics]);
            const retained = this.retainedLenses.get(retainedKey);
            if (retained) {
                this.#paintLens(context,retained,rect,radius);
                if (profile.rim === true && optics.rimWidth > 0) this.#rim(context,rect,radius,optics);
                return true;
            }
        }
        let captureRegion = null;
        if (optics.hyprglass && this.gpuUnavailable && !this.webgpu?.pipeline) {
            const sourceScale = profile.sourceScale === .25 ? .25 : 1;
            if (sourceScale !== 1) sampleRect = {x:sampleRect.x*sourceScale,y:sampleRect.y*sourceScale,width:sampleRect.width*sourceScale,height:sampleRect.height*sourceScale};
            const scale = .25 / sourceScale;
            const pad = Math.ceil((optics.blurPixels * 3 + 4) * sourceScale);
            const x = Math.max(0, Math.floor(sampleRect.x - pad));
            const y = Math.max(0, Math.floor(sampleRect.y - pad));
            captureRegion = { x, y, scale,
                width: Math.max(1, Math.min(source.width, Math.ceil(sampleRect.x + sampleRect.width + pad)) - x),
                height: Math.max(1, Math.min(source.height, Math.ceil(sampleRect.y + sampleRect.height + pad)) - y) };
            const sx = Math.ceil(captureRegion.width * scale) / captureRegion.width;
            const sy = Math.ceil(captureRegion.height * scale) / captureRegion.height;
            sampleRect = { x: (sampleRect.x - x) * sx, y: (sampleRect.y - y) * sy,
                width: sampleRect.width * sx, height: sampleRect.height * sy };
            optics = { ...optics, downsampled: true, blurPixels: optics.blurPixels * .25 };
        }
        let capture = this.#capture(source, captureRegion);
        if (!capture) return false;
        if (optics.hyprglass) {
            now = 0;
            // Upstream glass: two Gaussian iterations, sigma=radius/3.
            // Combine their variance once per captured backdrop. The optical
            // pass then needs only one sample, or three along the spectral rim.
            capture = this.#softwareBackdrop(capture, {...optics, hyprglass: false,
                saturation: 1, contrast: 1, brightness: 1}) || capture;
        }
        // Menus need a flat vibrancy sheet, not the refractive edge lens used
        // by decorative glass cards. The Canvas path retains registered blur
        // and tint while avoiding spectral corner pixels by construction.
        if (profile.sheet === true && !optics.hyprglass) {
            this.#canvasDraw(context, capture, rect, radius, optics, sampleRect);
        } else if (!this.webgpu?.draw(context, capture, rect, radius, optics, now, sampleRect, `${this.captureRevision}:${optics.hyprglass ? optics.blurPixels : ''}`) &&
            !this.#gpuDraw(context, capture, rect, radius, optics, now, sampleRect)) {
            this.#canvasDraw(context, capture, rect, radius, optics, sampleRect);
        }
        if (profile.rim === true && optics.rimWidth > 0) {
            this.#rim(context, rect, radius, optics);
        }
        if (retainedKey && this.softwareLens?.canvas) {
            const lens = this.softwareLens.canvas;
            const copy = this.canvasFactory(lens.width,lens.height);
            copy.getContext('2d').drawImage(lens,0,0);
            this.retainedLenses.set(retainedKey,copy);
            let bytes = [...this.retainedLenses.values()].reduce((sum,item)=>sum+item.width*item.height*4,0);
            while (this.retainedLenses.size > 16 || bytes > 4*1024*1024) {
                const key = this.retainedLenses.keys().next().value, oldest=this.retainedLenses.get(key);
                bytes -= oldest.width*oldest.height*4;
                this.retainedLenses.delete(key);
            }
        }
        return true;
    }

    drawRim(context, rect, radius, profile = {}, tracePath = null) {
        if (!context || !rect || rect.width < 6 || rect.height < 6) return false;
        if (tracePath !== null && typeof tracePath !== "function") return false;
        const optics = frostedIceOptics(profile);
        if (optics.rimWidth <= 0) return false;
        this.#rim(context, rect, radius, optics, tracePath);
        return true;
    }

    dispose() {
        this.webgpu?.dispose();
        this.webgpu = null;
        this.#disposeGpu(true);
        this.capture = null;
        this.captureSource = null;
        this.captureValid = false;
        this.backdropEpoch = NO_BACKDROP_EPOCH;
        this.softwareBackdrops.clear();
        this.softwareLens = null;
        this.retainedLenses?.clear();
        this.retainedLensSource = null;
        this.retainedLensEpoch = null;
    }

    #capture(source, region = null) {
        const width = region ? Math.ceil(region.width * (region.scale || 1)) : source.width;
        const height = region ? Math.ceil(region.height * (region.scale || 1)) : source.height;
        const regionKey = region ? `${region.x}:${region.y}:${width}:${height}` : "full";
        const retained = this.backdropEpoch !== NO_BACKDROP_EPOCH;
        if (
            retained && this.captureValid && this.captureSource === source &&
            this.captureRegionKey === regionKey &&
            this.capture?.width === width &&
            this.capture?.height === height
        ) {
            this.performance.captureReuses += 1;
            return this.capture;
        }
        if (!this.capture) {
            this.capture = this.canvasFactory(width, height);
        } else if (
            this.capture.width !== width ||
            this.capture.height !== height
        ) {
            // Resizing the retained surface preserves its context identity.
            // Replacing it would strand one graphics context on every native
            // window resize until the browser starts forcibly losing them.
            resizeCanvasBackingStore(this.capture, width, height);
        }
        const target = this.capture?.getContext?.("2d", { alpha: true, willReadFrequently: this.gpuUnavailable });
        if (!target?.drawImage) return null;
        target.setTransform?.(1, 0, 0, 1, 0, 0);
        target.clearRect?.(0, 0, width, height);
        if (region) target.drawImage(source, region.x, region.y, region.width, region.height, 0, 0, width, height);
        else target.drawImage(source, 0, 0);
        this.captureRegionKey = regionKey;
        this.captureSource = source;
        this.captureValid = retained;
        this.captureRevision += 1;
        this.performance.captures += 1;
        return this.capture;
    }

    #ensureGpu(width, height, preferHardware = false) {
        if (this.gpuUnavailable) return null;
        if (preferHardware && this.accelerationMode === 'auto' && this.gpu?.executionMode === 'software') {
            // A different theme may already have created an emulated context.
            // Do not inherit that expensive backend for full-window glass.
            this.#disposeGpu(true);
            this.capture = null;
            this.captureValid = false;
            this.softwareBackdrops.clear();
            return this.#fallback('hyprglass-hardware-unavailable');
        }
        if (this.effect) {
            // Share one capacity-sized buffer across differently sized windows.
            // Reallocating it for every title/body/window pass churns GPU memory.
            const nextWidth=Math.max(width,Math.min(this.effect.width,this.capture?.width||width));
            const nextHeight=Math.max(height,Math.min(this.effect.height,this.capture?.height||height));
            if(this.effect.width!==nextWidth||this.effect.height!==nextHeight){
                if(this.gpu)this.gpu.drawKey=null;
                this.effect.width=nextWidth;this.effect.height=nextHeight;
            }
        }
        if (this.gpu) return this.gpu;
        const selected = createRiceVisualEffectWebGl2Context(
            this.canvasFactory,
            width,
            height,
            preferHardware && this.accelerationMode === "auto" ? "hardware" : this.accelerationMode,
            {
                alpha: true,
                antialias: true,
                depth: false,
                stencil: false,
                premultipliedAlpha: true,
                preserveDrawingBuffer: false,
            },
        );
        const gl = selected?.gl;
        if (!gl?.createShader) return this.#fallback("webgl2-unavailable");
        this.effect = selected.canvas;
        const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
        const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
        if (!vertex || !fragment) {
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            return this.#fallback("shader-compilation-failed", selected);
        }
        const program = gl.createProgram();
        if (!program) {
            gl.deleteShader(vertex);
            gl.deleteShader(fragment);
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            return this.#fallback("program-allocation-failed", selected);
        }
        gl.attachShader(program, vertex);
        gl.attachShader(program, fragment);
        gl.linkProgram(program);
        gl.deleteShader(vertex);
        gl.deleteShader(fragment);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
            gl.deleteProgram(program);
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            return this.#fallback("shader-link-failed", selected);
        }
        const buffer = gl.createBuffer();
        if (!buffer) {
            gl.deleteProgram(program);
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            return this.#fallback("gpu-buffer-allocation-failed", selected);
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
            -1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1,
        ]), gl.STATIC_DRAW);
        const texture = gl.createTexture();
        if (!texture) {
            gl.deleteBuffer(buffer);
            gl.deleteProgram(program);
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            return this.#fallback("gpu-resource-allocation-failed", selected);
        }
        this.gpu = { gl, program, buffer, texture, ...selected };
        this.effect?.addEventListener?.("webglcontextlost", event => {
            event.preventDefault?.();
            this.gpu = null;
            this.effect = null;
            this.#fallback("webgl-context-lost", selected);
        }, { once: true });
        this.failed = "";
        this.onStatus({
            state: "ready",
            surface: this.surface,
            backend: "webgl2",
            mode: this.accelerationMode,
            executionMode: selected.executionMode,
            adapterClass: selected.adapterClass,
            renderer: selected.renderer,
            softwareFallback: selected.softwareFallback,
        });
        return this.gpu;
    }

    #gpuDraw(context, capture, rect, radius, optics, now, sampleRect) {
        try {
            const gpu = this.#ensureGpu(rect.width, rect.height);
            if (!gpu) return false;
            const { gl, program, buffer, texture } = gpu;
            // Body and titlebar often clip the same optical surface. Reuse only
            // identical pixels from the same captured backdrop and light time.
            const drawKey=JSON.stringify([this.captureRevision,rect.width,rect.height,radius,
                sampleRect.x,sampleRect.y,sampleRect.width,sampleRect.height,optics,now]);
            if(gpu.drawKey===drawKey){
                this.performance.gpuDrawReuses++;
                context.drawImage(this.effect,0,this.effect.height-rect.height,rect.width,rect.height,rect.x,rect.y,rect.width,rect.height);
                return true;
            }
            gl.viewport(0, 0, rect.width, rect.height);
            gl.clearColor(0, 0, 0, 0);
            gl.clear(gl.COLOR_BUFFER_BIT);
            gl.useProgram(program);
            gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
            const location = gl.getAttribLocation(program, "a_position");
            gl.enableVertexAttribArray(location);
            gl.vertexAttribPointer(location, 2, gl.FLOAT, false, 0, 0);
            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, texture);
            // Canvas and RiceWM scene coordinates are top-origin. The shader
            // already maps its top edge to texture v=0, so flipping the upload
            // here mirrors the captured wallpaper and offsets the blur on Y.
            gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
            gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
            const textureRevision = `${this.captureRevision}:${optics.hyprglass ? optics.blurPixels : ''}`;
            if (gpu.captureRevision !== textureRevision) {
                if(gpu.textureWidth===capture.width&&gpu.textureHeight===capture.height){
                    gl.texSubImage2D(gl.TEXTURE_2D,0,0,0,gl.RGBA,gl.UNSIGNED_BYTE,capture);
                }else{
                    gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,capture);
                    gpu.textureWidth=capture.width;gpu.textureHeight=capture.height;
                }
                gpu.captureRevision = textureRevision;
                this.performance.gpuBackdropUploads += 1;
            }
            const uniform = (name, ...values) => {
                const target = gl.getUniformLocation(program, name);
                if (values.length === 1) gl.uniform1f(target, values[0]);
                else if (values.length === 2) gl.uniform2f(target, values[0], values[1]);
                else gl.uniform3f(target, values[0], values[1], values[2]);
            };
            uniform("u_root", capture.width, capture.height);
            uniform("u_origin", sampleRect.x, sampleRect.y);
            uniform("u_size", sampleRect.width, sampleRect.height);
            uniform("u_radius", radius);
            uniform("u_thickness", optics.thickness);
            uniform("u_opacity", optics.opacity);
            uniform("u_blur", optics.blur);
            uniform("u_refraction", optics.refraction);
            uniform("u_chroma", optics.chroma);
            uniform("u_tint", optics.tintR, optics.tintG, optics.tintB);
            uniform("u_time", (Number(now) / 1000) % 10_000);
            uniform("u_light", optics.lightX, optics.lightY);
            uniform("u_pointer_light", optics.pointerLight ? 1 : 0);
            uniform("u_hyprglass", optics.hyprglass ? 1 : 0);
            gl.drawArrays(gl.TRIANGLES, 0, 6);
            gpu.drawKey=drawKey;
            this.performance.gpuDraws++;
            context.drawImage(this.effect,0,this.effect.height-rect.height,rect.width,rect.height,rect.x,rect.y,rect.width,rect.height);
            return true;
        } catch (error) {
            this.#fallback(error?.message || "webgl-render-failed", this.gpu);
            return false;
        }
    }

    #fallback(reason, selected = null) {
        this.gpuUnavailable = true;
        const error = String(reason || "webgl2-unavailable").slice(0, 256);
        if (error !== this.failed) {
            this.failed = error;
            this.onStatus({
                state: "fallback",
                surface: this.surface,
                backend: "canvas2d",
                mode: this.accelerationMode,
                executionMode: selected?.executionMode || "software",
                adapterClass: selected?.adapterClass || "none",
                renderer: selected?.renderer || "",
                softwareFallback: true,
                reason: error,
            });
        }
        return null;
    }

    #disposeGpu(loseContext) {
        const gpu = this.gpu;
        const gl = gpu?.gl;
        if (gl) {
            if (gpu.texture) gl.deleteTexture(gpu.texture);
            if (gpu.buffer) gl.deleteBuffer(gpu.buffer);
            if (gpu.program) gl.deleteProgram(gpu.program);
            if (loseContext) {
                gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            }
        }
        this.effect = null;
        this.gpu = null;
    }

    #canvasDraw(context, capture, rect, radius, optics, sampleRect) {
        if (optics.hyprglass && this.#softwareLens(context, capture, rect, radius, optics, sampleRect)) return;
        const retainedBackdrop = this.#softwareBackdrop(capture, optics);
        context.save();
        context.beginPath();
        context.roundRect?.(rect.x, rect.y, rect.width, rect.height, radius);
        if (!context.roundRect) context.rect(rect.x, rect.y, rect.width, rect.height);
        context.clip();
        context.filter = retainedBackdrop
            ? "none"
            : `blur(${optics.blurPixels}px) saturate(${optics.saturation}) contrast(${optics.contrast}) brightness(${optics.brightness})`;
        const scaleX = retainedBackdrop ? retainedBackdrop.width / capture.width : 1;
        const scaleY = retainedBackdrop ? retainedBackdrop.height / capture.height : 1;
        // The software fallback keeps the backdrop registered exactly with
        // the desktop. Optical bending belongs at the edge; stretching a
        // smaller source crop over the whole window shifts landmarks on Y.
        context.drawImage(
            retainedBackdrop || capture,
            sampleRect.x * scaleX,
            sampleRect.y * scaleY,
            sampleRect.width * scaleX,
            sampleRect.height * scaleY,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
        context.filter = "none";
        context.globalAlpha = optics.neutral ? 0 : .045 + optics.opacity * .16;
        context.fillStyle = `rgb(${Math.round(optics.tintR * 255)}, ` +
            `${Math.round(optics.tintG * 255)}, ` +
            `${Math.round(optics.tintB * 255)})`;
        context.fillRect(rect.x, rect.y, rect.width, rect.height);
        const light = this.#lightGradient(context, rect, optics);
        if (light && optics.rimWidth > 0) {
            light.addColorStop(0, "rgba(255,255,255,.24)");
            light.addColorStop(.12, "rgba(181,229,255,.075)");
            light.addColorStop(.48, "rgba(255,255,255,0)");
            light.addColorStop(.84, "rgba(82,166,207,.035)");
            light.addColorStop(1, "rgba(255,255,255,.14)");
            context.globalAlpha = 1;
            context.strokeStyle = light;
            context.lineWidth = 1.25 * optics.rimWidth;
            context.beginPath();
            context.roundRect?.(
                rect.x + .75,
                rect.y + .75,
                rect.width - 1.5,
                rect.height - 1.5,
                Math.max(0, radius - .75),
            );
            context.stroke?.();
        }
        context.restore();
    }

    #softwareLens(context, capture, rect, radius, optics, sample) {
        // Rasterize the optical background at a bounded resolution; application
        // text and controls are still drawn at the display's full resolution.
        const resolution = Math.min(.25, Math.sqrt(131072 / (rect.width * rect.height)));
        const width = Math.max(1, Math.ceil(rect.width * resolution));
        const height = Math.max(1, Math.ceil(rect.height * resolution));
        const key = JSON.stringify([this.captureRevision, rect.width, rect.height, radius, sample, optics.refraction, optics.chroma, optics.thickness, optics.blurPixels]);
        let lens = this.softwareLens;
        if (!lens) {
            const canvas = this.canvasFactory(width, height);
            const target = canvas?.getContext?.('2d');
            if (!target?.createImageData) return false;
            lens = this.softwareLens = {canvas, target};
        }
        if (lens.key !== key) {
            const source = capture.getContext('2d', {willReadFrequently: true});
            if (!source?.getImageData) return false;
            const data = source.getImageData(0, 0, capture.width, capture.height).data;
            if (lens.canvas.width !== width || lens.canvas.height !== height) {
                lens.canvas.width = width; lens.canvas.height = height;
            }
            if (lens.output?.width !== width || lens.output?.height !== height) lens.output = lens.target.createImageData(width, height);
            const output = lens.output, pixels = output.data;
            const scaleX = sample.width / rect.width, scaleY = sample.height / rect.height;
            const halfX = rect.width / 2, halfY = rect.height / 2;
            const r = Math.min(radius, halfX, halfY);
            const minDim = Math.min(rect.width, rect.height);
            const bezel = Math.max(1, minDim * optics.thickness / 350);
            const geometryKey = [width,height,rect.width,rect.height,radius,optics.refraction,optics.chroma,optics.thickness].join(':');
            if (lens.map?.length !== width*height*8) lens.map = new Float32Array(width*height*8);
            const map = lens.map;
            if (lens.geometryKey !== geometryKey) {
                for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
                const lx = (x + .5) * rect.width / width, ly = (y + .5) * rect.height / height;
                const px = lx-halfX, py = ly-halfY;
                const qx = Math.abs(px)-halfX+r, qy = Math.abs(py)-halfY+r;
                const distance = Math.hypot(Math.max(qx,0),Math.max(qy,0)) + Math.min(Math.max(qx,qy),0) - r;
                const edge = Math.exp(Math.min(distance,0)/bezel), length = Math.max(.1,Math.hypot(px,py));
                const offset = edge * optics.refraction * 50;
                const ox = -px/length*offset, oy = -py/length*offset;
                const cx = px/halfX, cy = py/halfY;
                const dome = .3 * minDim * .006 * (1-edge);
                const bx = lx-4*cx*(1-cy*cy)*dome, by = ly-4*cy*(1-cx*cx)*dome;
                const dispersion = edge > .01 ? optics.chroma * .35 : 0;
                const i = (y*width+x)*8;
                map[i]=bx+ox*(1-dispersion);map[i+1]=by+oy*(1-dispersion);
                map[i+2]=bx+ox;map[i+3]=by+oy;
                map[i+4]=bx+ox*(1+dispersion);map[i+5]=by+oy*(1+dispersion);
                map[i+6]=edge*edge;map[i+7]=ly/rect.height;
                }
                lens.geometryKey = geometryKey;
            }
            const samplingKey = JSON.stringify([geometryKey,sample,capture.width,capture.height]);
            if (lens.samples?.length !== width*height*6) lens.samples = new Float32Array(width*height*6);
            if (lens.indices?.length !== width*height*3) lens.indices = new Uint32Array(width*height*3);
            const indices = lens.indices;
            const samples = lens.samples;
            if (lens.samplingKey !== samplingKey) {
                for (let p=0;p<width*height;p++) for (let channel=0;channel<3;channel++) {
                    const m=p*8+channel*2, t=(p*3+channel)*2;
                    const x=Math.max(0,Math.min(capture.width-1.000001,sample.x+map[m]*scaleX-.5));
                    const y=Math.max(0,Math.min(capture.height-1.000001,sample.y+map[m+1]*scaleY-.5));
                    const ix=Math.floor(x),iy=Math.floor(y);
                    indices[p*3+channel]=(iy*capture.width+ix)*4+channel;
                    samples[t]=x-ix;samples[t+1]=y-iy;
                }
                lens.samplingKey = samplingKey;
            }
            const dx=capture.width>1?4:0,dy=capture.height>1?capture.width*4:0;
            const sampleChannel = t => {
                const p=indices[t],fx=samples[t*2],fy=samples[t*2+1];
                const top=data[p]+(data[p+dx]-data[p])*fx;
                const bottom=data[p+dy]+(data[p+dy+dx]-data[p+dy])*fx;
                return (top+(bottom-top)*fy)/255;
            };
            for (let index = 0, i = 0; index < pixels.length; index += 4, i += 8) {
                const t=index/4*3;
                let red = sampleChannel(t);
                let green = sampleChannel(t+1);
                let blue = sampleChannel(t+2);
                const lum = red*.2126+green*.7152+blue*.0722;
                const curve = Math.max(0,Math.min(1,(lum-.25)/.3));
                const dim = .82*(1-.3*curve*curve*(3-2*curve));
                red = .05+(lum+(red-lum)*.8)*dim*.9;
                green = .05+(lum+(green-lum)*.8)*dim*.9;
                blue = .05+(lum+(blue-lum)*.8)*dim*.9;
                const luma = red*.2126+green*.7152+blue*.0722;
                const vibrancy = 1+.15*(Math.max(red,green,blue)-Math.min(red,green,blue));
                const v = map[i+7], edge2 = map[i+6];
                const spec = (1-v)*(1-v)*edge2*.8*.08, rim = edge2*.4*.15, shadow = 1-v*v*edge2*.06;
                pixels[index] = ((luma+(red-luma)*vibrancy)+rim+spec)*shadow*255;
                pixels[index+1] = ((luma+(green-luma)*vibrancy)+rim+spec*.99)*shadow*255;
                pixels[index+2] = ((luma+(blue-luma)*vibrancy)+rim+spec*.97)*shadow*255;
                pixels[index+3] = 255;
            }
            lens.target.putImageData(output, 0, 0);
            lens.key = key;
        }
        this.#paintLens(context,lens.canvas,rect,radius);
        return true;
    }

    #paintLens(context,canvas,rect,radius) {
        context.save();
        context.beginPath();
        context.roundRect?.(rect.x,rect.y,rect.width,rect.height,radius);
        if (!context.roundRect) context.rect(rect.x,rect.y,rect.width,rect.height);
        context.clip();
        context.drawImage(canvas,rect.x,rect.y,rect.width,rect.height);
        context.restore();
    }

    #softwareBackdrop(capture, optics) {
        if (this.backdropEpoch === NO_BACKDROP_EPOCH) return null;
        // Diffuse glass has no high-frequency detail to preserve. Quarter-size
        // blur uses one sixteenth the pixels and is shared by every clipped
        // surface in this backdrop epoch. No pixel readback or CPU shader.
        const scale = optics.hyprglass && !optics.downsampled ? .25 : 1;
        const width = Math.max(1, Math.ceil(capture.width * scale));
        const height = Math.max(1, Math.ceil(capture.height * scale));
        const key = [
            scale,
            optics.blurPixels,
            optics.saturation,
            optics.contrast,
            optics.brightness,
        ].join(":");
        let retained = this.softwareBackdrops.get(key);
        if (!retained) {
            const canvas = this.canvasFactory(width, height);
            const context = canvas?.getContext?.("2d", { alpha: true, willReadFrequently: this.gpuUnavailable });
            if (!context?.drawImage) return null;
            retained = { canvas, context, revision: -1 };
            this.softwareBackdrops.set(key, retained);
            while (this.softwareBackdrops.size > MAX_SOFTWARE_BACKDROPS) {
                this.softwareBackdrops.delete(
                    this.softwareBackdrops.keys().next().value,
                );
            }
        } else {
            // Refresh LRU order without reallocating the backing surface.
            this.softwareBackdrops.delete(key);
            this.softwareBackdrops.set(key, retained);
        }
        if (
            retained.canvas.width !== width ||
            retained.canvas.height !== height
        ) {
            retained.canvas.width = width;
            retained.canvas.height = height;
            retained.revision = -1;
        }
        if (retained.revision === this.captureRevision) {
            this.performance.softwareBackdropReuses += 1;
            return retained.canvas;
        }
        const target = retained.context;
        target.save?.();
        try {
            target.setTransform?.(1, 0, 0, 1, 0, 0);
            target.clearRect?.(0, 0, width, height);
            target.filter = `blur(${optics.blurPixels * scale}px) ` +
                `saturate(${optics.saturation}) contrast(${optics.contrast}) brightness(${optics.brightness})`;
            target.drawImage(capture, 0, 0, width, height);
            target.filter = "none";
        } finally {
            target.restore?.();
        }
        retained.revision = this.captureRevision;
        this.performance.softwareBackdropBuilds += 1;
        return retained.canvas;
    }

    #rim(context, rect, radius, optics, tracePath = null) {
        context.save();
        context.beginPath();
        if (tracePath) {
            tracePath(context);
        } else {
            context.roundRect?.(rect.x + .75, rect.y + .75, rect.width - 1.5, rect.height - 1.5, Math.max(0, radius - .75));
            if (!context.roundRect) context.rect(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2);
        }
        const gradient = this.#lightGradient(context, rect, optics);
        if (gradient) {
            gradient.addColorStop(0, `rgba(255,255,255,${.82 - optics.opacity * .18})`);
            gradient.addColorStop(.16, "rgba(179,228,255,.34)");
            gradient.addColorStop(.42, "rgba(255,255,255,.07)");
            gradient.addColorStop(.68, "rgba(63,117,147,.11)");
            gradient.addColorStop(.86, "rgba(119,195,255,.28)");
            gradient.addColorStop(1, "rgba(255,255,255,.66)");
            context.strokeStyle = gradient;
        } else {
            context.strokeStyle = "rgba(255,255,255,.6)";
        }
        context.lineWidth = 1.25 * optics.rimWidth;
        context.shadowColor = "rgba(128,210,255,.22)";
        // Liquid glass already has a Fresnel/specular edge in the material.
        // A second canvas shadow rasterizes a window-sized blur for every rim.
        context.shadowBlur = optics.hyprglass ? 0 : 4 + optics.chroma * 7;
        context.stroke();
        context.shadowBlur = 0;
        context.beginPath();
        if (tracePath) {
            tracePath(context);
        } else {
            context.roundRect?.(
                rect.x + 2.5,
                rect.y + 2.5,
                rect.width - 5,
                rect.height - 5,
                Math.max(0, radius - 2.5),
            );
            if (!context.roundRect) context.rect(
                rect.x + 2.5,
                rect.y + 2.5,
                rect.width - 5,
                rect.height - 5,
            );
        }
        const inner = context.createLinearGradient?.(
            rect.x + rect.width,
            rect.y,
            rect.x,
            rect.y + rect.height,
        );
        if (inner) {
            inner.addColorStop(0, "rgba(255,255,255,.16)");
            inner.addColorStop(.38, "rgba(255,255,255,.025)");
            inner.addColorStop(.72, "rgba(0,17,27,.12)");
            inner.addColorStop(1, "rgba(85,190,229,.12)");
            context.strokeStyle = inner;
        } else {
            context.strokeStyle = "rgba(255,255,255,.1)";
        }
        context.lineWidth = tracePath ? .55 * optics.rimWidth : 1;
        context.stroke();
        context.restore();
    }

    #lightGradient(context, rect, optics) {
        if (!context.createLinearGradient) return null;
        if (!optics.pointerLight) {
            return context.createLinearGradient(
                rect.x, rect.y, rect.x + rect.width, rect.y + rect.height,
            );
        }
        const centerX = rect.x + rect.width / 2;
        const centerY = rect.y + rect.height / 2;
        const dx = centerX - optics.lightX;
        const dy = centerY - optics.lightY;
        return context.createLinearGradient(
            optics.lightX,
            optics.lightY,
            optics.lightX + dx * 2,
            optics.lightY + dy * 2,
        );
    }
}
