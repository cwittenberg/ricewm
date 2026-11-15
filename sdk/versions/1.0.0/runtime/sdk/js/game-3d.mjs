import { movePlatformBody } from './games.mjs';

const finite = (values) => {
  if (!values.every(Number.isFinite)) throw RangeError('Expected finite 3D coordinates');
};
const vector = (value) => {
  if (!Array.isArray(value) || value.length !== 3) throw TypeError('Expected [x,y,z]');
  finite(value);
  return value;
};
function cameraSpec(camera) {
  const {
    width,
    height,
    x = 0,
    y = 0,
    z = 0,
    yaw = 0,
    pitch = 0,
    focalLength = height,
    near = 0.1,
  } = camera || {};
  finite([width, height, x, y, z, yaw, pitch, focalLength, near]);
  if (width <= 0 || height <= 0 || focalLength <= 0 || near <= 0)
    throw RangeError('Invalid game camera');
  return { width, height, x, y, z, yaw, pitch, focalLength, near };
}
function cameraPoint(point, camera) {
  const dx = point[0] - camera.x,
    dy = point[1] - camera.y,
    dz = point[2] - camera.z;
  const x = dx * Math.cos(camera.yaw) - dz * Math.sin(camera.yaw),
    z = dx * Math.sin(camera.yaw) + dz * Math.cos(camera.yaw);
  return [
    x,
    dy * Math.cos(camera.pitch) - z * Math.sin(camera.pitch),
    dy * Math.sin(camera.pitch) + z * Math.cos(camera.pitch),
  ];
}
function project(point, camera) {
  return {
    x: camera.width / 2 + (point[0] * camera.focalLength) / point[2],
    y: camera.height / 2 - (point[1] * camera.focalLength) / point[2],
    depth: point[2],
  };
}
export function projectGamePoint(point, camera) {
  vector(point);
  const spec = cameraSpec(camera),
    p = cameraPoint(point, spec);
  return p[2] < spec.near ? null : Object.freeze(project(p, spec));
}
projectGamePoint.controlFactory = Object.freeze({
  topics: ['games'],
  constructor:
    'projectGamePoint([x,y,z],{width:640,height:360,x:0,y:2,z:-4,yaw:0,pitch:0,focalLength:360,near:0.1})',
  methods:
    'Y-up, positive-Z-forward perspective. yaw/pitch are radians. Returns {x,y,depth} or null behind near plane. Native vector renderer; no browser WebGL authority.',
});

export function gameCameraRay({ yaw = 0, pitch = 0 } = {}) {
  finite([yaw, pitch]);
  return [Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), Math.cos(yaw) * Math.cos(pitch)];
}
gameCameraRay.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'gameCameraRay({yaw:0,pitch:0})',
  methods:
    'Normalized [x,y,z] forward ray for the same radian camera as projectGamePoint. Use with pickGameBox for aiming, block editing and hitscan shooting.',
});

export function transformGamePoint(
  point,
  { position = [0, 0, 0], rotation = [0, 0, 0], scale = [1, 1, 1] } = {},
) {
  vector(point);
  vector(position);
  vector(rotation);
  vector(scale);
  let [x, y, z] = point.map((value, i) => value * scale[i]);
  [y, z] = [
    y * Math.cos(rotation[0]) - z * Math.sin(rotation[0]),
    y * Math.sin(rotation[0]) + z * Math.cos(rotation[0]),
  ];
  [x, z] = [
    x * Math.cos(rotation[1]) + z * Math.sin(rotation[1]),
    -x * Math.sin(rotation[1]) + z * Math.cos(rotation[1]),
  ];
  [x, y] = [
    x * Math.cos(rotation[2]) - y * Math.sin(rotation[2]),
    x * Math.sin(rotation[2]) + y * Math.cos(rotation[2]),
  ];
  return [x + position[0], y + position[1], z + position[2]];
}
transformGamePoint.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'transformGamePoint([x,y,z],{position:[0,0,0],rotation:[0,0,0],scale:[1,1,1]})',
  methods:
    'Scale, then local X/Y/Z radian rotations, then translation. Use on mesh vertices for rotating objects and procedural 3D shapes.',
});

function boxSpec(box) {
  if (!box) throw TypeError('Expected 3D box');
  finite([box.x, box.y, box.z, box.width, box.height, box.depth]);
  if (box.width <= 0 || box.height <= 0 || box.depth <= 0) throw RangeError('Invalid game box');
}
export function pickGameBox(origin, direction, boxes, { maxDistance = 64 } = {}) {
  vector(origin);
  vector(direction);
  finite([maxDistance]);
  if (maxDistance <= 0 || !Array.isArray(boxes) || boxes.length > 4096)
    throw RangeError('Invalid 3D picking bounds');
  const magnitude = Math.hypot(...direction);
  if (!magnitude) throw RangeError('Picking ray must have direction');
  const ray = direction.map((v) => v / magnitude);
  let result = null;
  boxes.forEach((box, index) => {
    boxSpec(box);
    const minimum = [box.x, box.y, box.z],
      size = [box.width, box.height, box.depth];
    let enter = -Infinity,
      exit = Infinity,
      enterNormal = null,
      exitNormal = null;
    for (let axis = 0; axis < 3; axis++) {
      if (Math.abs(ray[axis]) < 1e-12) {
        if (origin[axis] < minimum[axis] || origin[axis] > minimum[axis] + size[axis]) return;
        continue;
      }
      let a = (minimum[axis] - origin[axis]) / ray[axis],
        b = (minimum[axis] + size[axis] - origin[axis]) / ray[axis];
      const normal = [0, 0, 0];
      normal[axis] = ray[axis] > 0 ? -1 : 1;
      if (a > b) [a, b] = [b, a];
      if (a > enter) {
        enter = a;
        enterNormal = normal;
      }
      if (b < exit) {
        exit = b;
        exitNormal = normal.map((v) => (v === 0 ? 0 : -v));
      }
      if (enter > exit) return;
    }
    const distance = enter >= 0 ? enter : exit,
      normal = enter >= 0 ? enterNormal : exitNormal;
    if (distance < 0 || distance > maxDistance || (result && result.distance <= distance)) return;
    result = Object.freeze({
      index,
      distance,
      point: origin.map((v, i) => v + ray[i] * distance),
      normal,
    });
  });
  return result;
}
pickGameBox.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'pickGameBox([x,y,z],direction,boxes,{maxDistance:64})',
  methods:
    'Nearest slab-tested AABB hit {index,distance,point:[x,y,z],normal:[x,y,z]} or null. Handles inside starts and parallel rays. Up to 4096 boxes. Pick walls and actors together so walls occlude shots; normal gives the adjacent voxel placement face.',
});

export function moveGameCamera(
  camera,
  boxes,
  { forward = 0, strafe = 0, distance = 0.35, radius = 0.2, height = 1.6 } = {},
) {
  const { x, y, z, yaw = 0 } = camera;
  finite([x, y, z, yaw, forward, strafe, distance, radius, height]);
  if (
    radius <= 0 ||
    height <= 0 ||
    distance < 0 ||
    distance > 4 ||
    Math.abs(forward) > 1 ||
    Math.abs(strafe) > 1 ||
    !Array.isArray(boxes) ||
    boxes.length > 4096
  )
    throw RangeError('Invalid game movement');
  const solids = boxes
    .filter((box) => {
      boxSpec(box);
      return y > box.y && y - height < box.y + box.height;
    })
    .map((box) => ({ x: box.x, y: box.z, width: box.width, height: box.depth }));
  const magnitude = Math.max(1, Math.hypot(forward, strafe));
  const dx = ((Math.sin(yaw) * forward + Math.cos(yaw) * strafe) * distance) / magnitude,
    dz = ((Math.cos(yaw) * forward - Math.sin(yaw) * strafe) * distance) / magnitude;
  const body = {
    x: x - radius,
    y: z - radius,
    width: radius * 2,
    height: radius * 2,
    vx: dx / 0.1,
    vy: dz / 0.1,
  };
  movePlatformBody(body, solids, 0.1, { gravity: 0 });
  return { ...camera, x: body.x + radius, z: body.y + radius };
}
moveGameCamera.controlFactory = Object.freeze({
  topics: ['games'],
  constructor:
    'moveGameCamera(camera,boxes,{forward:1,strafe:0,distance:0.35,radius:0.2,height:1.6})',
  methods:
    'Returns camera after swept horizontal collision/sliding against AABBs at player height. eye position is camera x/y/z; height is eye-to-feet. Inputs -1..1, distance 0..4 scene units per action. Start outside blocks. Vertical gravity, jumping and moving geometry remain game rules.',
});

const pathIds = new WeakMap();
const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const sub = (a, b) => a.map((v, i) => v - b[i]);
const dot = (a, b) => a.reduce((sum, v, i) => sum + v * b[i], 0);
const shade = (color, light) => {
  if (!Number.isInteger(color) || color < 0 || color > 0xffffffff)
    throw RangeError('Expected packed AABBGGRR color');
  return (
    ((color & 0xff000000) |
      Math.round((color & 255) * light) |
      (Math.round(((color >>> 8) & 255) * light) << 8) |
      (Math.round(((color >>> 16) & 255) * light) << 16)) >>>
    0
  );
};
function clip(polygon, near) {
  const output = [];
  for (let i = 0; i < polygon.length; i++) {
    const a = polygon[i],
      b = polygon[(i + 1) % polygon.length],
      inside = a[2] >= near;
    if (inside) output.push(a);
    if (inside !== b[2] >= near) {
      const t = (near - a[2]) / (b[2] - a[2]);
      output.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), near]);
    }
  }
  return output;
}
const interpolate = (points, u, v) =>
  points[0].map(
    (_, i) =>
      points[0][i] * (1 - u) * (1 - v) +
      points[1][i] * u * (1 - v) +
      points[2][i] * u * v +
      points[3][i] * (1 - u) * v,
  );
/** Small convex/low-poly scenes, clipped and painter-sorted in one batch. */
export function drawGameMesh(
  scene,
  { vertices, faces },
  camera,
  { cull = true, light = [-0.5, 1, -0.7], ambient = 0.55, maximumFaces = 4096 } = {},
) {
  const spec = cameraSpec(camera);
  vector(light);
  finite([ambient]);
  if (
    !Array.isArray(vertices) ||
    vertices.length > 16384 ||
    !Array.isArray(faces) ||
    faces.length > 4096 ||
    !Number.isInteger(maximumFaces) ||
    maximumFaces < 1 ||
    maximumFaces > 8192 ||
    ambient < 0 ||
    ambient > 1 ||
    typeof cull !== 'boolean' ||
    !Math.hypot(...light)
  )
    throw RangeError('Invalid bounded game mesh');
  vertices.forEach(vector);
  const direction = light.map((v) => v / Math.hypot(...light)),
    paint = [];
  for (const face of faces) {
    if (
      !Array.isArray(face.indices) ||
      face.indices.length < 3 ||
      face.indices.length > 8 ||
      face.indices.some((i) => !Number.isInteger(i) || i < 0 || i >= vertices.length)
    )
      throw RangeError('Invalid mesh face');
    const points = face.indices.map((i) => vertices[i]),
      normal = cross(sub(points[1], points[0]), sub(points[2], points[0])),
      length = Math.hypot(...normal);
    if (!length) continue;
    if (cull && dot(normal, sub(points[0], [spec.x, spec.y, spec.z])) >= 0) continue;
    const intensity = ambient + (1 - ambient) * Math.max(0, dot(normal, direction) / length);
    const pattern = face.pattern;
    if (
      pattern &&
      (!['checker', 'stripes', 'grid'].includes(pattern.kind) ||
        points.length !== 4 ||
        !Number.isInteger(pattern.cells) ||
        pattern.cells < 1 ||
        pattern.cells > 8)
    )
      throw RangeError('Patterns require quad faces and 1..8 cells');
    const count = pattern?.cells || 1;
    for (let row = 0; row < count; row++)
      for (let column = 0; column < count; column++) {
        const polygon = pattern
          ? [
              [column / count, row / count],
              [(column + 1) / count, row / count],
              [(column + 1) / count, (row + 1) / count],
              [column / count, (row + 1) / count],
            ].map(([u, v]) => interpolate(points, u, v))
          : points;
        const clipped = clip(
          polygon.map((p) => cameraPoint(p, spec)),
          spec.near,
        );
        if (clipped.length < 3) continue;
        const projected = clipped.map((p) => project(p, spec));
        if (
          projected.every((p) => p.x < 0) ||
          projected.every((p) => p.x > spec.width) ||
          projected.every((p) => p.y < 0) ||
          projected.every((p) => p.y > spec.height)
        )
          continue;
        const alternate =
          pattern &&
          (pattern.kind === 'stripes'
            ? column % 2
            : pattern.kind === 'grid'
              ? column === 0 || row === 0
              : (column + row) % 2);
        paint.push({
          points: projected,
          depth: clipped.reduce((sum, p) => sum + p[2], 0) / clipped.length,
          color: shade(alternate ? pattern.color : face.color, intensity),
        });
        if (paint.length > maximumFaces) throw RangeError('Game mesh exceeded visible face budget');
      }
  }
  paint.sort((a, b) => b.depth - a.depth);
  let path = pathIds.get(scene) || 1;
  for (const face of paint) {
    scene
      .definePath(path, face.points.map((p, i) => [i ? 'L' : 'M', p.x, p.y]).concat([['Z']]))
      .fillPath(path++, face.color);
  }
  pathIds.set(scene, path);
  return scene;
}
drawGameMesh.controlFactory = Object.freeze({
  topics: ['games'],
  constructor:
    'drawGameMesh(scene,{vertices:[[x,y,z]],faces:[{indices:[0,1,2],color:0xff669933,pattern?:{kind:"checker"|"stripes"|"grid",cells:4,color:0xff335522}}]},camera,{cull:true,light:[-0.5,1,-0.7],ambient:0.55,maximumFaces:4096})',
  methods:
    'Native low-poly 3D with radian yaw/pitch camera, near clipping, viewport rejection, outward-wound face culling, flat directional light and perspective-projected procedural quad patterns. Up to 16384 vertices/4096 source faces; visible face budget 1..8192. Batch intersecting objects in one mesh where possible. Painter sorting has no depth buffer or perspective image textures; split intersecting geometry. Reserves path ids from 1, advancing across calls on the same builder.',
});

export function drawGameBoxes(scene, boxes, camera, options) {
  if (!Array.isArray(boxes) || boxes.length > 256)
    throw RangeError('Expected up to 256 game boxes');
  const vertices = [],
    faces = [];
  for (const box of boxes) {
    boxSpec(box);
    const { x, y, z, width: w, height: h, depth: d, color, pattern } = box,
      base = vertices.length;
    vertices.push(
      [x, y, z],
      [x + w, y, z],
      [x + w, y + h, z],
      [x, y + h, z],
      [x, y, z + d],
      [x + w, y, z + d],
      [x + w, y + h, z + d],
      [x, y + h, z + d],
    );
    for (const indices of [
      [0, 3, 2, 1],
      [4, 5, 6, 7],
      [0, 4, 7, 3],
      [1, 2, 6, 5],
      [3, 7, 6, 2],
      [0, 1, 5, 4],
    ])
      faces.push({ indices: indices.map((i) => base + i), color, pattern });
  }
  return drawGameMesh(scene, { vertices, faces }, camera, options);
}
drawGameBoxes.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'drawGameBoxes(scene,boxes,camera,options?)',
  methods:
    'Up to 256 boxes {x,y,z,width,height,depth,color,pattern?}. Shares drawGameMesh clipping, culling, flat light and checker/stripes/grid procedural patterns. Use small bounded voxel scenes and positive-Z camera with yaw/pitch radians; no engine dependency or browser WebGL. Face sorting cannot resolve arbitrary intersecting polygons. Add boxes in one call for shared depth sorting.',
});
