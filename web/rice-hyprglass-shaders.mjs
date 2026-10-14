export const HYPRGLASS_GLSL = `// Hyprglass optical model, BSD-3-Clause, Jeremy Trufier (2025).
// See assets/licenses/hyprglass.txt. Gaussian diffusion is cached before this pass;
// spectral sampling is confined to the bezel. No time-driven lighting.
vec3 hyprBlur(vec2 uv) {
    float pad = 4. * 1.41421356237 * u_blur / .62 * 3. + 4.;
    vec2 low = max(vec2(0.), (u_origin - pad) / u_root);
    vec2 high = min(vec2(1.), (u_origin + u_size + pad) / u_root);
    return texture(u_backdrop, clamp(uv, low, high)).rgb;
}
vec4 hyprGlass(vec2 local, float distance, float mask) {
    vec2 size = u_size;
    vec2 uv = local / size;
    float minDim = min(size.x, size.y);
    float bezel = max(1., minDim * u_thickness / 350.);
    float edge = exp(min(distance, 0.) / bezel);
    vec2 toCenter = size * .5 - local;
    vec2 direction = toCenter / max(length(toCenter), .1);
    vec2 offset = direction * edge * u_refraction * 50. / u_root;
    vec2 c = (uv - .5) * 2.;
    vec2 dome = vec2(-4. * c.x * (1. - c.y*c.y), -4. * c.y * (1. - c.x*c.x)) * (.3 * minDim * .006) * (1. - edge) / u_root;
    vec2 base = (u_origin + local) / u_root + dome;
    vec3 color = hyprBlur(base + offset);
    if (u_chroma > .001 && edge > .01) {
        color.r = hyprBlur(base + offset * (1. - u_chroma * .35)).r;
        color.b = hyprBlur(base + offset * (1. + u_chroma * .35)).b;
    }
    float lum = dot(color, vec3(.2126, .7152, .0722));
    color = mix(vec3(lum), color, .80);
    color *= .82 * (1. - .3 * smoothstep(.25, .55, lum));
    color = mix(vec3(.5), color, .90);
    float currentLum = dot(color, vec3(.2126, .7152, .0722));
    float sat = max(color.r, max(color.g, color.b)) - min(color.r, min(color.g, color.b));
    color = mix(vec3(currentLum), color, 1. + .15 * sat);
    color += vec3(edge * edge * .4 * .15);
    color += vec3(1., .99, .97) * pow(1. - uv.y, 2.) * edge * edge * .8 * .08;
    color *= 1. - uv.y * uv.y * edge * edge * .06;
    return vec4(clamp(color, vec3(0.), vec3(1.)) * mask, mask);
}
`;

export const HYPRGLASS_WGSL = `// Hyprglass optical model, BSD-3-Clause, Jeremy Trufier (2025).
// See assets/licenses/hyprglass.txt. Gaussian diffusion is cached before this pass;
// spectral sampling is confined to the bezel. No time-driven lighting.
fn hyprBlur(uv: vec2f) -> vec3f {
    let pad = 4. * 1.41421356237 * u.optics.y / .62 * 3. + 4.;
    let low = max(vec2f(0.), (u.rootOrigin.zw - vec2f(pad)) / u.rootOrigin.xy);
    let high = min(vec2f(1.), (u.rootOrigin.zw + u.shape.xy + vec2f(pad)) / u.rootOrigin.xy);
    return textureSampleLevel(backdrop, backdropSampler, clamp(uv, low, high), 0.).rgb;
}
fn hyprGlass(local: vec2f, distance: f32, mask: f32) -> vec4f {
    var size: vec2f = u.shape.xy;
    var uv: vec2f = local / size;
    var minDim: f32 = min(size.x, size.y);
    var bezel: f32 = max(1., minDim * u.shape.w / 350.);
    var edge: f32 = exp(min(distance, 0.) / bezel);
    var toCenter: vec2f = size * .5 - local;
    var direction: vec2f = toCenter / max(length(toCenter), .1);
    var offset: vec2f = direction * edge * u.optics.z * 50. / u.rootOrigin.xy;
    var c: vec2f = (uv - .5) * 2.;
    var dome: vec2f = vec2f(-4. * c.x * (1. - c.y*c.y), -4. * c.y * (1. - c.x*c.x)) * (.3 * minDim * .006) * (1. - edge) / u.rootOrigin.xy;
    var base: vec2f = (u.rootOrigin.zw + local) / u.rootOrigin.xy + dome;
    var color: vec3f = hyprBlur(base + offset);
    if (u.optics.w > .001 && edge > .01) {
        color.r = hyprBlur(base + offset * (1. - u.optics.w * .35)).r;
        color.b = hyprBlur(base + offset * (1. + u.optics.w * .35)).b;
    }
    var lum: f32 = dot(color, vec3f(.2126, .7152, .0722));
    color = mix(vec3f(lum), color, .80);
    color *= .82 * (1. - .3 * smoothstep(.25, .55, lum));
    color = mix(vec3f(.5), color, .90);
    var currentLum: f32 = dot(color, vec3f(.2126, .7152, .0722));
    var sat: f32 = max(color.r, max(color.g, color.b)) - min(color.r, min(color.g, color.b));
    color = mix(vec3f(currentLum), color, 1. + .15 * sat);
    color += vec3f(edge * edge * .4 * .15);
    color += vec3f(1., .99, .97) * pow(1. - uv.y, 2.) * edge * edge * .8 * .08;
    color *= 1. - uv.y * uv.y * edge * edge * .06;
    return vec4f(clamp(color, vec3f(0.), vec3f(1.)) * mask, mask);
}
`;
