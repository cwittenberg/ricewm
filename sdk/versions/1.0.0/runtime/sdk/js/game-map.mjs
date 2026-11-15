import { gameSeededRandom } from './game-chunks.mjs';

const coord = n => Number.isInteger(n) && Math.abs(n) <= 2147483647;
const identifier = s => typeof s === 'string' && /^[A-Za-z][\w.-]{0,63}$/.test(s);
const hash = (seed, x, y) => gameSeededRandom((seed ^ Math.imul(x, 73856093) ^ Math.imul(y, 19349663)) | 0)();
const smooth = t => t * t * (3 - 2 * t);
function noise(seed, x, y, scale) {
  x /= scale; y /= scale;
  const c = Math.floor(x), r = Math.floor(y), u = smooth(x - c), v = smooth(y - r);
  const a = hash(seed, c, r) * (1 - u) + hash(seed, c + 1, r) * u;
  const b = hash(seed, c, r + 1) * (1 - u) + hash(seed, c + 1, r + 1) * u;
  return a * (1 - v) + b * v;
}
const rect = value => value && coord(value.column) && coord(value.row) &&
  Number.isInteger(value.columns) && value.columns > 0 && value.columns <= 256 &&
  Number.isInteger(value.rows) && value.rows > 0 && value.rows <= 256 &&
  coord(value.column + value.columns - 1) && coord(value.row + value.rows - 1);
const overlaps = (a, b) => a.column < b.column + b.columns && a.column + a.columns > b.column &&
  a.row < b.row + b.rows && a.row + a.rows > b.row;

/** Editable map data overlays deterministic procedural regions. No rendering,
 * filesystem authority or executable scripts are embedded in map JSON. */
export class GameMap {
  static controlFactory = Object.freeze({
    topics: ['games'], constructor: 'new GameMap({seed:42,procedural:true,settlements:true,patches:[{column:0,row:0,columns:8,rows:8,tile:"stone"}],entities:[{id:"inn",kind:"building",type:"cottage",column:8,row:2,columns:3,rows:3}]})',
    methods: 'tile(globalColumn,globalRow), paint({column,row,columns,rows,tile}), place({id,kind,type,column,row,columns,rows,...data}), remove(id), region(rect), chunk({chunkX,chunkY,size}), toJSON(), GameMap.fromJSON(data). Validated ricewm.game-map/v1 JSON supports seed/biomes, ordered terrain rectangles, authored buildings/props/NPCs/enemies/quests/loot/player spawns and explicit procedural-entity removals. Default coherent forest/grass/stone biomes and roads; optional deterministic multi-building settlements every 64 cells. Authored entities replace generated IDs. place rejects overlapping buildings atomically. region returns only intersecting entities, bounded to 4096 queried cells/1024 results; chunk plugs directly into GameChunkWorld.generate. Edits are map data, not simulation state; invalidate/rebuild affected loaded chunks/navigation/fog/terrain and persist map JSON through granted app storage. Keep encounter progress separate so editing/reloading cannot resurrect defeated enemies. Up to 4096 paint rectangles, 4096 entities and 4096 removals; JSON <=2 MiB. A scene editor must use the exact same map coordinates and model, never a disconnected decorative minimap.',
  });
  constructor({ seed = 1, procedural = true, settlements = true, biomes = {}, patches = [], entities = [], removed = [] } = {}) {
    if (!Number.isInteger(seed) || typeof procedural !== 'boolean' || typeof settlements !== 'boolean' ||
        !Array.isArray(patches) || patches.length > 4096 || !Array.isArray(entities) || entities.length > 4096 ||
        !Array.isArray(removed) || removed.length > 4096 || removed.some(id => !identifier(id))) throw RangeError('Invalid game map');
    this.seed = seed; this.procedural = procedural; this.settlements = settlements;
    this.biomes = { grass: 'grass', forest: 'forest', stone: 'stone', ...biomes };
    if (Object.keys(this.biomes).some(k => !['grass', 'forest', 'stone'].includes(k)) || Object.values(this.biomes).some(v => !identifier(v))) throw RangeError('Invalid map biomes');
    this.patches = []; this.entities = new Map(); this.removed = new Set(removed); this.revision = 0;
    for (const patch of patches) this.paint(patch);
    for (const entity of entities) if (!this.place(entity)) throw Error('Overlapping map building');
    this.revision = 0;
  }
  tile(column, row) {
    if (!coord(column) || !coord(row)) throw RangeError('Invalid map coordinate');
    for (let i = this.patches.length - 1; i >= 0; i--) {
      const p = this.patches[i];
      if (column >= p.column && column < p.column + p.columns && row >= p.row && row < p.row + p.rows) return p.tile;
    }
    if (!this.procedural) return this.biomes.grass;
    // The road grid joins regions; noise is evaluated globally, without chunk seams.
    const mod = n => ((n % 64) + 64) % 64;
    if (mod(column - 5) < 2 || mod(row - 4) < 2) return this.biomes.stone;
    const value = noise(this.seed, column, row, 21) * 0.72 + noise(this.seed ^ 71, column, row, 7) * 0.28;
    return value < 0.48 ? this.biomes.forest : value > 0.76 ? this.biomes.stone : this.biomes.grass;
  }
  paint(patch) {
    if (!rect(patch) || !identifier(patch.tile) || this.patches.length >= 4096) throw RangeError('Invalid map paint rectangle');
    this.patches.push({ column: patch.column, row: patch.row, columns: patch.columns, rows: patch.rows, tile: patch.tile });
    this.revision++; return this;
  }
  generated(rectangle) {
    if (!this.procedural || !this.settlements) return [];
    const result = [];
    for (let ry = Math.floor((rectangle.row - 24) / 64); ry <= Math.floor((rectangle.row + rectangle.rows + 24) / 64); ry++)
      for (let cx = Math.floor((rectangle.column - 24) / 64); cx <= Math.floor((rectangle.column + rectangle.columns + 24) / 64); cx++) {
        const village = [
          ['cottage', 8, -2, 3, 3], ['chapel', -4, -2, 3, 4],
          ['tower', 14, 0, 2, 2], ['keep', 19, 14, 5, 4], ['cottage', -4, 9, 3, 3],
        ];
        village.forEach(([type, dx, dy, columns, rows], i) => {
          const e = { id: `village.${cx}.${ry}.${i}`, kind: 'building', type, column: cx * 64 + dx,
            row: ry * 64 + dy, columns, rows, seed: (hash(this.seed, cx + i, ry) * 0x7fffffff) | 0 };
          if (overlaps(e, rectangle)) result.push(e);
        });
      }
    return result;
  }
  place(entity) {
    if (!rect(entity) || !identifier(entity.id) || !identifier(entity.type) ||
        !['building', 'prop', 'npc', 'enemy', 'quest', 'loot', 'spawn'].includes(entity.kind) ||
        !this.entities.has(entity.id) && this.entities.size >= 4096) throw RangeError('Invalid map entity');
    const copy = JSON.parse(JSON.stringify(entity));
    if (JSON.stringify(copy).length > 16384) throw RangeError('Map entity data exceeds budget');
    if (entity.kind === 'building') {
      const others = [...this.generated(entity), ...this.entities.values()];
      if (others.some(e => e.id !== entity.id && e.kind === 'building' && !this.removed.has(e.id) &&
          !this.entities.has(e.id) && overlaps(e, entity)) ||
          [...this.entities.values()].some(e => e.id !== entity.id && e.kind === 'building' && overlaps(e, entity))) return false;
    }
    this.entities.set(entity.id, copy); this.removed.delete(entity.id); this.revision++; return true;
  }
  remove(id) {
    if (!identifier(id) || !this.removed.has(id) && this.removed.size >= 4096) throw RangeError('Invalid map removal');
    this.entities.delete(id); this.removed.add(id); this.revision++;
  }
  region(rectangle) {
    if (!rect(rectangle) || rectangle.columns * rectangle.rows > 4096) throw RangeError('Map region exceeds budget');
    const result = new Map(this.generated(rectangle).filter(e => !this.removed.has(e.id)).map(e => [e.id, e]));
    for (const [id, entity] of this.entities) {
      result.delete(id);
      if (overlaps(entity, rectangle)) result.set(id, structuredClone(entity));
    }
    if (result.size > 1024) throw RangeError('Map region entity budget exceeded');
    return [...result.values()];
  }
  chunk({ chunkX, chunkY, size }) {
    if (!coord(chunkX) || !coord(chunkY) || !Number.isInteger(size) || size < 8 || size > 64) throw RangeError('Invalid map chunk');
    const tiles = [];
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) tiles.push(this.tile(chunkX * size + x, chunkY * size + y));
    return { tiles, state: {} };
  }
  toJSON() {
    return { schema: 'ricewm.game-map/v1', seed: this.seed, procedural: this.procedural, settlements: this.settlements,
      biomes: { ...this.biomes }, patches: this.patches.map(p => ({ ...p })), entities: [...this.entities.values()].map(e => structuredClone(e)), removed: [...this.removed] };
  }
  static fromJSON(data) {
    if (typeof data === 'string') { if (data.length > 2 * 1024 * 1024) throw RangeError('Map JSON exceeds budget'); data = JSON.parse(data); }
    if (!data || data.schema !== 'ricewm.game-map/v1' || JSON.stringify(data).length > 2 * 1024 * 1024) throw Error('Expected ricewm.game-map/v1');
    return new GameMap(data);
  }
}
