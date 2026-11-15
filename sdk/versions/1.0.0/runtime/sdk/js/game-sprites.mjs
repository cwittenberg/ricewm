const finite = (values) => {
  if (!values.every(Number.isFinite)) throw RangeError('Expected finite sprite coordinates');
};
const integer = (value, min, max) => Number.isInteger(value) && value >= min && value <= max;

/** Packed/trimmed RGBA atlases use the existing native image command. No image
 * decoder or renderer is bundled: openImageEngine can supply real PNG pixels. */
export class GameSpriteAtlas {
  static controlFactory = Object.freeze({
    topics: ['games'],
    constructor:
      'new GameSpriteAtlas({width,height,rgba,frames:{walk0:{x:0,y:0,width:32,height:48,pivotX:0.5,pivotY:1,samplingInset:0.5},walk1:{x:34,y:0,width:32,height:48,pivotX:0.5,pivotY:1,samplingInset:0.5}},animations:{walk:[{frame:"walk0",duration:0.1},{frame:"walk1",duration:0.1}]}})',
    methods:
      'draw(scene,imageId,frame,{x,y,scale:1,rotation:0,flipX:false,flipY:false,opacity:1,smoothing:true}); sample(animation,time,{loop:true}) returns frame name. For platform characters, NPCs and enemies, supply coherent distinct pose frames and all action-state clips required by the platformer contract; this two-frame example demonstrates playback only, not a complete actor. A static sprite or duplicate image cells do not satisfy character animation. Animation time/duration are seconds; rotation radians. Supports transparent/trimmed frames, pivots, flips, rotation, alpha, nearest-neighbour or smooth sampling. RGBA bytes come from openImageEngine or original procedural art. At most 1 megapixel/4096 frames. Immutable atlas pixels are captured once; each native frame includes only the sampled regions and filtering neighbours when this reduces its size. Frames remain self-contained. Reading or replacing atlas.rgba opts into the mutable compatibility path. Use a distinct imageId per atlas in the same frame. Optional per-frame samplingInset:0.5 samples inside its pixel centers to prevent adjacent-frame bleed when scaling filtered packed art; it does not change destination geometry or pivots. Defaults to 0 for compatibility and is ignored for smoothing:false. Keep transparent gutters too. Filtered sampling is the default; choose smoothing:false only for deliberate retro art. Sample skeletal models with parseGameGLTF/parseGameGLB and bakeGameModelAtlas before atlas playback. Advanced shaders and compressed GPU textures are not provided.',
  });
  constructor({ width, height, rgba, frames, animations = {} } = {}) {
    if (
      !integer(width, 1, 2048) ||
      !integer(height, 1, 2048) ||
      width * height > 1048576 ||
      !(rgba instanceof Uint8Array) ||
      rgba.length !== width * height * 4 ||
      !frames ||
      typeof frames !== 'object' ||
      Array.isArray(frames) ||
      Object.keys(frames).length < 1 ||
      Object.keys(frames).length > 4096
    )
      throw RangeError('Invalid bounded sprite atlas');
    this.width = width;
    this.height = height;
    const image = {resource:createSceneImageResource(width,height,rgba),width,height,mutable:null,exposed:false};
    atlasImages.set(this,image);
    Object.defineProperty(this,'rgba',{enumerable:true,configurable:true,
      get(){if(!image.exposed){image.mutable=copySceneImagePixels(image.resource);image.exposed=true;}return image.mutable;},
      set(value){image.mutable=value;image.exposed=true;},
    });
    this.frames = new Map();
    this.animations = new Map();
    this.defined = new WeakMap();
    for (const [name, frame] of Object.entries(frames)) {
      if (!frame || !name) throw TypeError('Invalid sprite frame');
      const {
        x,
        y,
        width: w,
        height: h,
        sourceWidth = w,
        sourceHeight = h,
        offsetX = 0,
        offsetY = 0,
        pivotX = 0.5,
        pivotY = 1,
        samplingInset = 0,
      } = frame;
      if (
        !integer(x, 0, width - 1) ||
        !integer(y, 0, height - 1) ||
        !integer(w, 1, width) ||
        !integer(h, 1, height) ||
        x + w > width ||
        y + h > height ||
        !integer(sourceWidth, 1, 4096) ||
        !integer(sourceHeight, 1, 4096) ||
        !integer(offsetX, 0, sourceWidth - w) ||
        !integer(offsetY, 0, sourceHeight - h)
      )
        throw RangeError('Sprite frame escapes atlas/source bounds');
      finite([pivotX, pivotY, samplingInset]);
      if(samplingInset<0||samplingInset>=Math.min(w,h)/2)throw RangeError('Sprite sampling inset escapes frame');
      if (pivotX < 0 || pivotX > 1 || pivotY < 0 || pivotY > 1)
        throw RangeError('Invalid sprite pivot');
      this.frames.set(
        name,
        Object.freeze({
          x,
          y,
          width: w,
          height: h,
          sourceWidth,
          sourceHeight,
          offsetX,
          offsetY,
          pivotX,
          pivotY,
          samplingInset,
        }),
      );
    }
    if (
      !animations ||
      typeof animations !== 'object' ||
      Array.isArray(animations) ||
      Object.keys(animations).length > 256
    )
      throw RangeError('Invalid sprite animations');
    for (const [name, sequence] of Object.entries(animations)) {
      if (
        !Array.isArray(sequence) ||
        !sequence.length ||
        sequence.length > 4096 ||
        sequence.some(
          (step) =>
            !this.frames.has(step.frame) ||
            !Number.isFinite(step.duration) ||
            step.duration <= 0 ||
            step.duration > 60,
        )
      )
        throw RangeError('Invalid sprite animation frames');
      this.animations.set(name, Object.freeze(sequence.map((step) => Object.freeze({ ...step }))));
    }
  }
  sample(animation, time, { loop = true } = {}) {
    finite([time]);
    if (time < 0 || typeof loop !== 'boolean') throw RangeError('Invalid sprite animation time');
    const sequence = this.animations.get(animation);
    if (!sequence) throw RangeError('Unknown sprite animation');
    const duration = sequence.reduce((sum, step) => sum + step.duration, 0);
    let elapsed = loop ? time % duration : Math.min(time, duration);
    for (const step of sequence) {
      if (elapsed < step.duration) return step.frame;
      elapsed -= step.duration;
    }
    return sequence.at(-1).frame;
  }
  draw(
    scene,
    imageId,
    frame,
    {
      x,
      y,
      scale = 1,
      rotation = 0,
      flipX = false,
      flipY = false,
      opacity = 1,
      smoothing = true,
    } = {},
  ) {
    const rect = this.frames.get(frame);
    if (!rect) throw RangeError('Unknown sprite frame');
    finite([x, y, scale, rotation, opacity]);
    if (
      scale <= 0 ||
      scale > 1024 ||
      opacity < 0 ||
      opacity > 1 ||
      typeof flipX !== 'boolean' ||
      typeof flipY !== 'boolean' ||
      typeof smoothing !== 'boolean'
    )
      throw RangeError('Invalid sprite transform');
    let ids = this.defined.get(scene);
    if (!ids) {
      ids = new Set();
      this.defined.set(scene, ids);
    }
    if (!ids.has(imageId)) {
      const image=atlasImages.get(this);
      if(scene instanceof SceneBuilder&&!image.exposed&&this.width===image.width&&this.height===image.height)
        appendSceneImageResource(scene,imageId,image.resource);
      else scene.defineImage(imageId, this.width, this.height, this.rgba);
      ids.add(imageId);
    }
    const sx = scale * (flipX ? -1 : 1),
      sy = scale * (flipY ? -1 : 1),
      cos = Math.cos(rotation),
      sin = Math.sin(rotation);
    const inset=smoothing?rect.samplingInset:0;
    scene.pushTransform([cos * sx, sin * sx, -sin * sy, cos * sy, x, y]);
    scene.image(
      imageId,
      [
        rect.offsetX - rect.sourceWidth * rect.pivotX,
        rect.offsetY - rect.sourceHeight * rect.pivotY,
        rect.width,
        rect.height,
      ],
      {
        uv: [
          (rect.x + inset) / this.width,
          (rect.y + inset) / this.height,
          (rect.x + rect.width - inset) / this.width,
          (rect.y + rect.height - inset) / this.height,
        ],
        opacity,
        smoothing,
      },
    );
    scene.popTransform();
    return scene;
  }
}

const isoSpec = ({
  tileWidth = 64,
  tileHeight = 32,
  originX = 320,
  originY = 32,
  elevationHeight = 32,
} = {}) => {
  finite([tileWidth, tileHeight, originX, originY, elevationHeight]);
  if (tileWidth <= 0 || tileHeight <= 0 || elevationHeight <= 0)
    throw RangeError('Invalid isometric projection');
  return { tileWidth, tileHeight, originX, originY, elevationHeight };
};
export function isometricPoint(column, row, elevation = 0, options = {}) {
  finite([column, row, elevation]);
  const s = isoSpec(options);
  return Object.freeze({
    x: s.originX + ((column - row) * s.tileWidth) / 2,
    y: s.originY + ((column + row) * s.tileHeight) / 2 - elevation * s.elevationHeight,
  });
}
isometricPoint.controlFactory = Object.freeze({
  topics: ['games'],
  constructor:
    'isometricPoint(column,row,elevation=0,{tileWidth:64,tileHeight:32,originX:320,originY:32,elevationHeight:32})',
  methods:
    'Returns top diamond {x,y}; draw the tile around x ± tileWidth/2, y..y+tileHeight. Place sprite feet at x,y+tileHeight/2. Sort tiles/entities from low to high column+row (then elevation) for occlusion; keep authoritative grid collision/turns/quests in the app.',
});
export function isometricCell(x, y, options = {}) {
  finite([x, y]);
  const s = isoSpec(options),
    dx = (x - s.originX) / s.tileWidth,
    dy = (y - s.originY) / s.tileHeight;
  return Object.freeze({ column: Math.floor(dy + dx), row: Math.floor(dy - dx) });
}
isometricCell.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'isometricCell(x,y,{tileWidth:64,tileHeight:32,originX:320,originY:32})',
  methods:
    'Inverse ground-plane diamond picking for isometricPoint; returns integer {column,row}. Check map bounds and walls in your model. Elevated tiles/objects need their own front-to-back hit tests.',
});
import { SceneBuilder } from './scene-builder.mjs';
import {createSceneImageResource, copySceneImagePixels, appendSceneImageResource} from './scene-image-resource.mjs';
const atlasImages = new WeakMap();
