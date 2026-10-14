import {
    createRiceVisualEffectWebGl2Context,
    riceVisualEffectExecutionOrder,
} from "./rice-visual-effect-capability.mjs";

const VERTEX_SHADER = `#version 300 es
precision highp float;
out vec2 rice_cogl_uv;
void main() {
    vec2 positions[3] = vec2[3](
        vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0)
    );
    vec2 position = positions[gl_VertexID];
    gl_Position = vec4(position, 0.0, 1.0);
    // Cogl actor texture coordinates are top-origin. WebGL clip coordinates
    // are bottom-origin, so translate Y here before unchanged Cogl snippets
    // consume cogl_tex_coord_in. Without this, directional effects run
    // vertically backwards.
    rice_cogl_uv = vec2(
        (position.x + 1.0) * 0.5,
        1.0 - (position.y + 1.0) * 0.5
    );
}`;

function compileShader(gl, type, source) {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("WebGL2 shader allocation failed");
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        const error = gl.getShaderInfoLog(shader) || "WebGL2 shader compilation failed";
        gl.deleteShader(shader);
        throw new Error(error);
    }
    return shader;
}

function fragmentShader(effect) {
    const snippets = (Array.isArray(effect?.snippets) ? effect.snippets : [])
        .filter(snippet => Number(snippet?.hook) === 1);
    if (!snippets.length) throw new Error("Cogl effect has no fragment snippet");
    const declarations = snippets.map(snippet => String(snippet.declarations || ""))
        .join("\n").replace(/\btexture2D\s*\(/g, "texture(");
    const code = snippets.map(snippet => String(snippet.code || ""))
        .join("\n").replace(/\btexture2D\s*\(/g, "texture(");
    if (declarations.includes("#version") || code.includes("#version")) {
        throw new Error("Cogl snippets cannot replace the WebGL version directive");
    }
    return `#version 300 es
precision highp float;
in vec2 rice_cogl_uv;
out vec4 rice_cogl_output;
${declarations}
void main() {
    vec4 cogl_tex_coord_in[1];
    cogl_tex_coord_in[0] = vec4(rice_cogl_uv, 0.0, 1.0);
    vec4 cogl_color_out = vec4(0.0);
    ${code}
    rice_cogl_output = cogl_color_out;
}`;
}

function tightlyPacked(resource) {
    const width = Math.max(1, Number(resource.width) || 1);
    const height = Math.max(1, Number(resource.height) || 1);
    const source = resource.bytes instanceof Uint8Array
        ? resource.bytes : new Uint8Array(resource.bytes || []);
    const rowstride = Math.max(1, Number(resource.rowstride) || 1);
    const channels = Math.max(1, Math.min(4, Math.floor(rowstride / width)));
    const rowBytes = width * channels;
    if (rowstride === rowBytes && source.byteLength >= rowBytes * height) {
        return { bytes: source.subarray(0, rowBytes * height), channels };
    }
    const bytes = new Uint8Array(rowBytes * height);
    for (let y = 0; y < height; y += 1) {
        bytes.set(
            source.subarray(y * rowstride, y * rowstride + rowBytes),
            y * rowBytes,
        );
    }
    return { bytes, channels };
}

export function riceGnomeEffectExecutionOrder(mode) {
    return riceVisualEffectExecutionOrder(mode);
}

export class RiceGnomeEffectRenderer {
    constructor({
        canvasFactory,
        hardwareOnly = true,
        onStatus = () => {},
    } = {}) {
        if (typeof canvasFactory !== "function") {
            throw new TypeError("GNOME effects require a canvas factory");
        }
        this.canvasFactory = canvasFactory;
        this.hardwareOnly = Boolean(hardwareOnly);
        this.onStatus = typeof onStatus === "function" ? onStatus : () => {};
        this.mode = "disabled";
        this.executionMode = "disabled";
        this.enabled = false;
        this.canvas = null;
        this.gl = null;
        this.vertexArray = null;
        this.programs = new Map();
        this.effectSources = new Map();
        this.resources = new Map();
        this.textures = new Map();
        this.effects = [];
        this.failed = "";
        this.frameCache = null;
    }

    setEnabled(enabled) {
        this.setMode(enabled ? (this.hardwareOnly ? "hardware" : "software") : "disabled");
    }

    setMode(mode) {
        const next = ["auto", "hardware", "software"].includes(String(mode))
            ? String(mode) : "disabled";
        if (next === this.mode) return;
        this.#disposeGpu();
        this.mode = next;
        this.executionMode = next;
        this.enabled = next !== "disabled";
        this.onStatus({
            state: "configured",
            backend: this.enabled ? "webgl2" : "none",
            mode: this.mode,
            softwareFallback: this.mode === "software",
        });
    }

    update(payload = {}) {
        for (const resource of Array.isArray(payload.resources) ? payload.resources : []) {
            const id = String(resource?.id || "");
            const bytes = resource?.bytes instanceof Uint8Array
                ? resource.bytes : new Uint8Array(resource?.bytes || []);
            const width = Math.max(1, Math.min(4096, Number(resource?.width) || 1));
            const height = Math.max(1, Math.min(4096, Number(resource?.height) || 1));
            if (!id || !bytes.byteLength || bytes.byteLength > 64 * 1024 * 1024) continue;
            this.resources.set(id, {
                id,
                width,
                height,
                rowstride: Math.max(1, Number(resource?.rowstride) || width * 4),
                pixelFormat: Math.max(0, Number(resource?.pixelFormat) || 0),
                bytes: new Uint8Array(bytes),
            });
            const texture = this.textures.get(id);
            if (texture && this.gl) this.gl.deleteTexture(texture);
            this.textures.delete(id);
            this.frameCache = null;
        }
        this.effects = (Array.isArray(payload.effects) ? payload.effects : [])
            .filter(effect =>
                this.resources.has(String(effect?.resourceId || "")) &&
                Number(effect?.width) > 0 && Number(effect?.height) > 0
            )
            .slice(0, 32).map(effect => {
                const snippets = Array.isArray(effect.snippets) ? effect.snippets : [];
                const previous = this.effectSources.get(effect.id);
                const same = previous && previous.snippets.length === snippets.length &&
                    snippets.every((snippet, index) => ['hook', 'declarations', 'code', 'replace']
                        .every(key => snippet[key] === previous.snippets[index][key]));
                const cached = same ? previous : {
                    snippets: snippets.map(snippet => ({ ...snippet })),
                    source: null,
                };
                this.effectSources.set(effect.id, cached);
                return { ...effect, shaderSource: cached };
            });
        const activeIds = new Set(this.effects.map(effect => effect.id));
        for (const id of this.effectSources.keys()) {
            if (!activeIds.has(id)) this.effectSources.delete(id);
        }
    }

    render(context, viewport) {
        if (!this.enabled || !this.effects.length) return false;
        let gl;
        try {
            gl = this.#ensureGpu(this.effects[0]);
        } catch (error) {
            this.#fail(error, this.effects[0]);
            return false;
        }
        let rendered = false;
        for (const effect of this.effects) {
            try {
                const sourceWidth = Math.max(1, Number(effect.sourceWidth) || viewport.width || 1);
                const sourceHeight = Math.max(1, Number(effect.sourceHeight) || viewport.height || 1);
                const width = Math.max(1, Math.min(4096, Math.round(
                    Number(effect.width) * Number(viewport.width) / sourceWidth,
                )));
                const height = Math.max(1, Math.min(4096, Math.round(
                    Number(effect.height) * Number(viewport.height) / sourceHeight,
                )));
                if (this.canvas.width !== width || this.canvas.height !== height) {
                    this.canvas.width = width;
                    this.canvas.height = height;
                }
                const uniforms = JSON.stringify(effect.uniforms || {});
                const cached = this.frameCache;
                if (!cached || cached.width !== width || cached.height !== height ||
                    cached.source !== effect.shaderSource || cached.resourceId !== effect.resourceId ||
                    cached.uniforms !== uniforms) {
                    gl.viewport(0, 0, width, height);
                    gl.disable(gl.BLEND);
                    gl.clearColor(0, 0, 0, 0);
                    gl.clear(gl.COLOR_BUFFER_BIT);
                    const program = this.#program(effect);
                    const texture = this.#texture(String(effect.resourceId));
                    gl.useProgram(program.program);
                    gl.bindVertexArray(this.vertexArray);
                    gl.activeTexture(gl.TEXTURE0);
                    gl.bindTexture(gl.TEXTURE_2D, texture);
                    const sampler = this.#uniformLocation(program, "tex");
                    if (sampler !== null) gl.uniform1i(sampler, 0);
                    this.#uniforms(program, effect.uniforms);
                    gl.drawArrays(gl.TRIANGLES, 0, 3);
                    this.frameCache = { width, height, source: effect.shaderSource,
                        resourceId: effect.resourceId, uniforms };
                }

                const x = Number(effect.x) * Number(viewport.width) / sourceWidth;
                const y = Number(effect.y) * Number(viewport.height) / sourceHeight;
                const targetWidth = Number(effect.width) * Number(viewport.width) / sourceWidth;
                const targetHeight = Number(effect.height) * Number(viewport.height) / sourceHeight;
                context.save();
                context.globalAlpha *= Math.max(0, Math.min(1, Number(effect.opacity) || 0));
                context.drawImage(this.canvas, x, y, targetWidth, targetHeight);
                context.restore();
                rendered = true;
            } catch (error) {
                this.#fail(error, effect);
            }
        }
        return rendered;
    }

    dispose() {
        this.#disposeGpu();
        this.resources.clear();
        this.effectSources.clear();
        this.effects = [];
    }

    #ensureGpu(effect = null) {
        if (this.gl) return this.gl;
        const selected = createRiceVisualEffectWebGl2Context(
            this.canvasFactory,
            1,
            1,
            this.mode,
            {
                alpha: true,
                antialias: false,
                depth: false,
                stencil: false,
                premultipliedAlpha: true,
                // Retain the last shader result across unrelated compositor
                // damage. Only actual shader-input changes need a GPU draw.
                preserveDrawingBuffer: true,
            },
        );
        if (!selected) {
            throw new Error(this.mode === "hardware"
                ? "GNOME GLSL effects require a hardware WebGL2 adapter"
                : "WebGL2 software fallback is unavailable for GNOME GLSL effects");
        }
        const { gl } = selected;
        this.canvas = selected.canvas;
        this.executionMode = selected.executionMode;
        this.gl = selected.gl;
        this.vertexArray = gl.createVertexArray();
        if (!this.vertexArray) throw new Error("WebGL2 vertex array allocation failed");
        gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
        // Resource rows arrive in GdkPixbuf's top-to-bottom order. Since the
        // Cogl-compatible shader coordinates above also use Y=0 at the top,
        // retain that row order when uploading typed pixel buffers.
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
        this.failed = "";
        this.onStatus({
            state: "ready",
            backend: "webgl2",
            mode: this.mode,
            executionMode: selected.executionMode,
            uuid: String(effect?.uuid || ""),
            renderer: selected.renderer,
            adapterClass: selected.adapterClass,
            softwareFallback: selected.softwareFallback,
        });
        return gl;
    }

    #program(effect) {
        const fragment = effect.shaderSource.source ??= fragmentShader(effect);
        const cached = this.programs.get(fragment);
        if (cached) return cached;
        const gl = this.gl;
        const vertex = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
        const pixel = compileShader(gl, gl.FRAGMENT_SHADER, fragment);
        const program = gl.createProgram();
        if (!program) throw new Error("WebGL2 program allocation failed");
        gl.attachShader(program, vertex);
        gl.attachShader(program, pixel);
        gl.linkProgram(program);
        gl.deleteShader(vertex);
        gl.deleteShader(pixel);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
            const error = gl.getProgramInfoLog(program) || "WebGL2 effect link failed";
            gl.deleteProgram(program);
            throw new Error(error);
        }
        const value = { program, uniforms: new Map() };
        this.programs.set(fragment, value);
        return value;
    }

    #texture(id) {
        const cached = this.textures.get(id);
        if (cached) return cached;
        const resource = this.resources.get(id);
        if (!resource) throw new Error(`GNOME effect texture ${id} is unavailable`);
        const gl = this.gl;
        const texture = gl.createTexture();
        if (!texture) throw new Error("WebGL2 texture allocation failed");
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        const packed = tightlyPacked(resource);
        const format = packed.channels >= 4 ? gl.RGBA : gl.RGB;
        const internal = packed.channels >= 4 ? gl.RGBA8 : gl.RGB8;
        gl.texImage2D(
            gl.TEXTURE_2D,
            0,
            internal,
            resource.width,
            resource.height,
            0,
            format,
            gl.UNSIGNED_BYTE,
            packed.bytes,
        );
        this.textures.set(id, texture);
        return texture;
    }

    #uniformLocation(program, name) {
        if (!program.uniforms.has(name)) {
            program.uniforms.set(name, this.gl.getUniformLocation(program.program, name));
        }
        return program.uniforms.get(name);
    }

    #uniforms(program, uniforms) {
        const gl = this.gl;
        for (const [name, value] of Object.entries(uniforms || {}).slice(0, 128)) {
            const location = this.#uniformLocation(program, String(name).slice(0, 128));
            if (location === null) continue;
            if (Array.isArray(value)) {
                const numbers = value.map(Number).filter(Number.isFinite).slice(0, 16);
                if (numbers.length === 1) gl.uniform1f(location, numbers[0]);
                else if (numbers.length === 2) gl.uniform2fv(location, numbers);
                else if (numbers.length === 3) gl.uniform3fv(location, numbers);
                else if (numbers.length === 4) gl.uniform4fv(location, numbers);
            } else if (value?.matrix && Array.isArray(value.matrix)) {
                const numbers = value.matrix.map(Number).filter(Number.isFinite);
                const size = Math.max(1, Math.min(4, Number(value.dimensions) || 1));
                if (size === 2 && numbers.length >= 4) gl.uniformMatrix2fv(location, false, numbers.slice(0, 4));
                else if (size === 3 && numbers.length >= 9) gl.uniformMatrix3fv(location, false, numbers.slice(0, 9));
                else if (size === 4 && numbers.length >= 16) gl.uniformMatrix4fv(location, false, numbers.slice(0, 16));
            }
        }
    }

    #fail(error, effect = null) {
        const message = String(error?.message || error || "GNOME GLSL effect failed").slice(0, 512);
        if (message === this.failed) return;
        this.failed = message;
        this.onStatus({
            state: "failed",
            backend: "webgl2",
            mode: this.mode,
            executionMode: this.executionMode,
            softwareFallback: this.executionMode === "software",
            uuid: String(effect?.uuid || ""),
            effect: String(effect?.id || ""),
            error: message,
        });
    }

    #disposeGpu() {
        const gl = this.gl;
        if (gl) {
            for (const texture of this.textures.values()) gl.deleteTexture(texture);
            for (const value of this.programs.values()) gl.deleteProgram(value.program);
            if (this.vertexArray) gl.deleteVertexArray(this.vertexArray);
            gl.getExtension("WEBGL_lose_context")?.loseContext?.();
        }
        this.frameCache = null;
        this.textures.clear();
        this.programs.clear();
        this.vertexArray = null;
        this.gl = null;
        this.canvas = null;
    }
}
