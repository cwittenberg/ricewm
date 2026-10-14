// Shared WebGL2 capability policy for compositor-owned visual effects.
//
// These effects use GLSL even when the desktop presenter selected WebGPU, so
// they cannot share a WebGPUDevice. They do share the same hardware/software
// policy, adapter classification, and deterministic Canvas2D fallback.

export function riceVisualEffectExecutionOrder(mode) {
    const policy = String(mode || "disabled");
    if (policy === "auto") return Object.freeze(["hardware", "software"]);
    if (policy === "hardware" || policy === "software") {
        return Object.freeze([policy]);
    }
    return Object.freeze([]);
}

export function riceWebGlRendererIsSoftware(renderer) {
    return /\b(?:swiftshader|subzero|llvmpipe|softpipe|lavapipe|swrast|warp|software rasterizer|microsoft basic render driver)\b/i
        .test(String(renderer || ""));
}

export function riceWebGlRendererIdentity(gl) {
    const debug = gl?.getExtension?.("WEBGL_debug_renderer_info");
    const name = debug
        ? gl.getParameter?.(debug.UNMASKED_RENDERER_WEBGL)
        : gl?.getParameter?.(gl.RENDERER);
    const renderer = String(name || "unknown").slice(0, 240);
    return Object.freeze({
        renderer,
        software: riceWebGlRendererIsSoftware(renderer),
    });
}

export function createRiceVisualEffectWebGl2Context(
    canvasFactory,
    width,
    height,
    mode,
    attributes = {},
) {
    if (typeof canvasFactory !== "function") return null;
    for (const executionMode of riceVisualEffectExecutionOrder(mode)) {
        const canvas = canvasFactory(width, height);
        const gl = canvas?.getContext?.("webgl2", {
            ...attributes,
            powerPreference: executionMode === "hardware"
                ? "high-performance" : "low-power",
            // Reject SwiftShader and other major caveats on the hardware
            // attempt. Auto then gets one compatibility attempt before the
            // owner falls back to its Canvas2D implementation.
            failIfMajorPerformanceCaveat: executionMode === "hardware",
        });
        if (!gl) continue;
        const identity = riceWebGlRendererIdentity(gl);
        if (executionMode === "hardware" && identity.software) {
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            continue;
        }
        if (executionMode === "software" && !identity.software) {
            // A low-power preference is still allowed to select a real GPU.
            // Never let the explicit software/fallback policy silently turn
            // hardware acceleration back on.
            gl.getExtension?.("WEBGL_lose_context")?.loseContext?.();
            continue;
        }
        return Object.freeze({
            canvas,
            gl,
            executionMode,
            renderer: identity.renderer,
            adapterClass: identity.software ? "software" : "hardware",
            softwareFallback: executionMode === "software",
        });
    }
    return null;
}
