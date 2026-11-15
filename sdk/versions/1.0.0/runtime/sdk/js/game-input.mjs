/** App-local action state. Physical keys and pointer controls share actions,
 * but releasing one source never releases another source that is still held. */
export class GameInput {
  static controlFactory = Object.freeze({
    topics: ['games'],
    constructor: 'new GameInput({left:[113,38],right:[114,40],jump:[65,111,25]})',
    methods: 'key({keyCode,pressed,modifiers:0}) returns newly pressed action names; ignores OS repeats. set(action,pressed,source) adds pointer/controller-local sources. down(action), axis(negative,positive), takePressed(action), takeReleased(action), endFrame(), clear(). Edges persist until a fixed update consumes them. Always clear on blur, hide, pause, restart and close. X11 W25 A38 S39 D40, arrows111/113/114/116, Space65. No global listeners.',
  });
  constructor(bindings = {}) {
    if (!bindings || typeof bindings !== 'object' || Array.isArray(bindings) || Object.keys(bindings).length > 64)
      throw TypeError('Game input needs up to 64 named bindings');
    this.bindings = new Map();
    for (const [name, codes] of Object.entries(bindings)) {
      if (!/^[A-Za-z][A-Za-z0-9_.:-]{0,63}$/.test(name) || !Array.isArray(codes) || codes.length > 16 || codes.some(code => !Number.isInteger(code) || code < 1 || code > 255))
        throw TypeError('Game bindings need names and native key codes');
      this.bindings.set(name, new Set(codes));
    }
    this.sources = new Map();
    this.presses = new Set();
    this.releases = new Set();
  }
  set(name, pressed, source = 'pointer') {
    if (!this.bindings.has(name) || typeof pressed !== 'boolean' || typeof source !== 'string' || !source || source.length > 80)
      throw TypeError('Unknown game action or invalid input source');
    let sources = this.sources.get(name);
    if (!sources) this.sources.set(name, sources = new Set());
    const previous = sources.size > 0;
    if (pressed) {
      if (sources.size >= 32 && !sources.has(source)) throw RangeError('Too many game input sources');
      sources.add(source);
    } else sources.delete(source);
    const current = sources.size > 0;
    if (current && !previous) this.presses.add(name);
    if (!current && previous) this.releases.add(name);
    return current && !previous;
  }
  key({ keyCode, pressed, modifiers = 0 } = {}) {
    if (!Number.isInteger(keyCode) || typeof pressed !== 'boolean') return [];
    const activated = [];
    for (const [name, codes] of this.bindings) {
      if (!codes.has(keyCode)) continue;
      // Key-up must clear even when Ctrl/Alt was pressed after key-down.
      if (pressed && modifiers & (4 | 8 | 64)) continue;
      if (this.set(name, pressed, 'key:' + keyCode)) activated.push(name);
    }
    return activated;
  }
  down(name) { return (this.sources.get(name)?.size || 0) > 0; }
  axis(negative, positive) { return Number(this.down(positive)) - Number(this.down(negative)); }
  takePressed(name) { return this.presses.delete(name); }
  takeReleased(name) { return this.releases.delete(name); }
  endFrame() { this.presses.clear(); this.releases.clear(); }
  clear() { this.sources.clear(); this.endFrame(); }
}
