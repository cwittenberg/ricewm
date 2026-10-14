import { HYPRGLASS_WGSL } from "./rice-hyprglass-shaders.mjs";
// Optical-equivalent WGSL companion to rice-frosted-ice.mjs.
export const FROSTED_ICE_WGSL = `
struct Uniforms { rootOrigin: vec4f, shape: vec4f, optics: vec4f, tintTime: vec4f, light: vec4f }
@group(0) @binding(0) var backdrop: texture_2d<f32>;
@group(0) @binding(1) var backdropSampler: sampler;
@group(0) @binding(2) var<uniform> u: Uniforms;
struct VertexOutput { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOutput {
 let positions=array<vec2f,3>(vec2f(-1.,-1.),vec2f(3.,-1.),vec2f(-1.,3.));
 let p=positions[index];var result: VertexOutput;
 result.position=vec4f(p,0.,1.);result.uv=vec2f(p.x*.5+.5,.5-p.y*.5);return result;
}
fn roundedDistance(p: vec2f) -> f32 {
    var halfSize: vec2f = u.shape.xy * .5;
    var q: vec2f = abs(p) - max(halfSize - vec2f(u.shape.z), vec2f(0.));
    return length(max(q, vec2f(0.))) + min(max(q.x, q.y), 0.) - u.shape.z;
}
fn surfaceHeight(p: vec2f) -> f32 {
    var depth: f32 = clamp(-roundedDistance(p) / max(u.shape.w, 1.), 0., 1.);
    var n: f32 = 3.4;
    return pow(max(1. - pow(1. - depth, n), 0.), 1. / n);
}
fn transmitted(uv: vec2f, diffusion: f32) -> vec3f {
    var texel: vec2f = 1. / max(u.rootOrigin.xy, vec2f(1.));
    var radius: f32 = 2. + diffusion * 24.;
    var inner: f32 = radius * .45;
    var c: vec3f = textureSampleLevel(backdrop, backdropSampler, uv, 0.0).rgb * .04;
    for (var index: i32 = 0; index < 8; index++) {
        var angle: f32 = (f32(index) + .5) * .78539816339;
        var direction: vec2f = vec2f(cos(angle), sin(angle));
        c += textureSampleLevel(backdrop, backdropSampler, clamp(uv + direction * texel * inner, vec2f(.001), vec2f(.999)), 0.0).rgb * .04;
        c += textureSampleLevel(backdrop, backdropSampler, clamp(uv + direction * texel * radius, vec2f(.001), vec2f(.999)), 0.0).rgb * .08;
    }
    return c;
}
${HYPRGLASS_WGSL}
@fragment fn fragment_main(input: VertexOutput) -> @location(0) vec4f {
    var local: vec2f = input.uv * u.shape.xy;
    var p: vec2f = local - u.shape.xy * .5;
    var distance: f32 = roundedDistance(p);
    var mask: f32 = 1. - smoothstep(-1., 1., distance);
    if (mask <= 0.) { return vec4f(0.); }
    if (u.light.w > .5) { return hyprGlass(local, distance, mask); }

    var e: f32 = 1.25;
    var gradient: vec2f = vec2f(
        surfaceHeight(p + vec2f(e, 0.)) - surfaceHeight(p - vec2f(e, 0.)),
        surfaceHeight(p + vec2f(0., e)) - surfaceHeight(p - vec2f(0., e))
    ) / (2. * e);
    var depth: f32 = clamp(-distance / max(u.shape.w, 1.), 0., 1.);
    var edge: f32 = 1. - smoothstep(.18, .96, depth);
    var normal: vec3f = normalize(vec3f(-gradient * u.shape.w * .32, 1.));
    var ray: vec3f = refract(vec3f(0., 0., -1.), normal, 1. / 1.46);
    var displacement: vec2f = ray.xy / max(-ray.z, .18) * edge *
        (8. + u.optics.z * 34.) / max(u.rootOrigin.xy, vec2f(1.));
    var baseUv: vec2f = (u.rootOrigin.zw + local) / u.rootOrigin.xy;
    var uv: vec2f = clamp(baseUv + displacement, vec2f(.002), vec2f(.998));

    var diffusion: f32 = .08 + u.optics.y * .46 + edge * .20;
    var glass: vec3f = transmitted(uv, diffusion);
    if (edge > 0.) { glass = mix(glass, transmitted(clamp(baseUv - displacement * .38, vec2f(.002), vec2f(.998)), diffusion * .72), edge * .20); }
    if (edge > 0. && u.optics.w > 0.) {
    var dispersion: vec2f = normalize(
        displacement + normal.xy / max(u.rootOrigin.xy, vec2f(1.)) + vec2f(.000001)
    ) *
        edge * (1. + u.optics.w * 7.) / max(u.rootOrigin.xy, vec2f(1.));
    var dispersed: vec3f = glass;
    dispersed.r = textureSampleLevel(backdrop, backdropSampler, clamp(uv + dispersion, vec2f(.002), vec2f(.998)), 0.0).r;
    dispersed.b = textureSampleLevel(backdrop, backdropSampler, clamp(uv - dispersion, vec2f(.002), vec2f(.998)), 0.0).b;
    glass = mix(glass, dispersed, edge * u.optics.w * .28);
    }

    var luma: f32 = dot(glass, vec3f(.2126, .7152, .0722));
    glass = mix(vec3f(luma), glass, 1.10 + u.optics.y * .22);
    var tint: f32 = (.035 + u.optics.x * .16) * (.72 + luma * .28);
    glass = mix(glass, u.tintTime.xyz, tint);

    var ambientLightPoint: vec2f = vec2f(
        .16 + sin(u.tintTime.w * .11) * .025,
        .08 + cos(u.tintTime.w * .09) * .018
    ) * u.rootOrigin.xy;
    var lightPoint: vec2f = mix(ambientLightPoint, u.light.xy, u.light.z);
    var toLight: vec2f = (lightPoint - (u.rootOrigin.zw + local)) /
        max(max(u.rootOrigin.xy.x, u.rootOrigin.xy.y), 1.);
    var light: vec3f = normalize(vec3f(toLight * 1.9, .72));
    var view: vec3f = vec3f(0., 0., 1.);
    var facing: f32 = max(dot(normal, view), 0.);
    var fresnel: f32 = pow(1. - facing, 2.2);
    var specular: f32 = pow(max(dot(normalize(light + view), normal), 0.), 88.);
    var broadReflection: f32 = pow(max(dot(normalize(light + view), normal), 0.), 9.);
    var litRim: f32 = pow(max(dot(normal.xy, normalize(toLight + vec2f(.0001))), 0.), 2.4);
    var outerLine: f32 = exp(-abs(distance) * 1.15);
    var innerLine: f32 = exp(-abs(distance + u.shape.w * .56) * .31);
    var caustic: f32 = innerLine * litRim * (.35 + u.optics.z * .65);
    var orientation: f32 = dot(normal.xy, normalize(vec2f(-.72, -.48)));
    var spectrum: vec3f = vec3f(
        max(-orientation, 0.),
        .30 + (1. - abs(orientation)) * .38,
        max(orientation, 0.)
    ) * edge * u.optics.w * (.06 + fresnel * .24);
    var reflection: vec3f = vec3f(.72, .88, 1.) *
        (specular * .34 + broadReflection * edge * .055);
    reflection += vec3f(.42, .78, 1.) * caustic * .18;
    reflection += vec3f(1.) * outerLine * (.055 + litRim * .12);
    reflection += vec3f(.62, .83, .92) * innerLine * .035;
    var oppositeShade: f32 = edge * pow(max(dot(-normal.xy, normalize(toLight + vec2f(.0001))), 0.), 2.) * .09;
    var color: vec3f = glass * (1. - oppositeShade) + reflection + spectrum;
    var alpha: f32 = mask;
    return vec4f(color * alpha, alpha);
}`;
