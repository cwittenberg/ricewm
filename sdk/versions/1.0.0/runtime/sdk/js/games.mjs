/** Small game primitives for native SceneSurface apps. Coordinates and velocity
 * use scene units and seconds; rendering stays with the existing RiceUI host. */
export class GameStepper {
  static controlFactory = Object.freeze({
    topics: ['games'],
    constructor: 'new GameStepper({step:1/60,maxSteps:8})',
    methods:
      'advance(delta,update) runs bounded fixed updates; reset() drops accumulated time. Pass AnimationClock delta in seconds. Pause/reset on hidden windows; never catch up wall time after resume.',
  });
  constructor({ step = 1 / 60, maxSteps = 8 } = {}) {
    if (
      !Number.isFinite(step) ||
      step < 1 / 240 ||
      step > 0.1 ||
      !Number.isInteger(maxSteps) ||
      maxSteps < 1 ||
      maxSteps > 32
    )
      throw RangeError('Invalid fixed game step');
    this.step = step;
    this.maxSteps = maxSteps;
    this.accumulator = 0;
  }
  advance(delta, update) {
    if (!Number.isFinite(delta) || delta < 0 || typeof update !== 'function')
      throw TypeError('Expected game delta and update callback');
    this.accumulator = Math.min(this.accumulator + delta, this.step * this.maxSteps);
    let count = 0;
    while (this.accumulator + 1e-12 >= this.step && count < this.maxSteps) {
      this.accumulator = Math.max(0, this.accumulator - this.step);
      count++;
      if (update(this.step) === false) {
        this.reset();
        break;
      }
    }
    return count;
  }
  reset() {
    this.accumulator = 0;
  }
}

const finite = (values) => {
  if (!values.every(Number.isFinite)) throw RangeError('Expected finite game coordinates');
};
const rectangle = (value) => {
  if (!value) throw TypeError('Expected game rectangle');
  finite([value.x, value.y, value.width, value.height]);
  if (value.width <= 0 || value.height <= 0) throw RangeError('Game rectangles need positive size');
};
export function gameOverlaps(a, b) {
  rectangle(a);
  rectangle(b);
  return a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
}
gameOverlaps.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'gameOverlaps(a,b)',
  methods: 'Strict AABB overlap for {x,y,width,height}; touching edges are not collisions.',
});

/** Swept axis resolution against static, non-overlapping starting geometry.
 * Resolves X then Y, including thin platforms at high speeds. Mutates body. */
export function movePlatformBody(body, solids, delta, { gravity = 900 } = {}) {
  rectangle(body);
  finite([body.vx, body.vy, delta, gravity]);
  if (delta < 0 || delta > 0.1 || !Array.isArray(solids) || solids.length > 4096)
    throw RangeError('Invalid platform step');
  solids.forEach(rectangle);
  body.grounded = false;
  body.vy += gravity * delta;
  for (const [axis, other, size, otherSize, velocity] of [
    ['x', 'y', 'width', 'height', 'vx'],
    ['y', 'x', 'height', 'width', 'vy'],
  ]) {
    let distance = body[velocity] * delta,
      hit = false;
    for (const solid of solids) {
      if (
        body[other] >= solid[other] + solid[otherSize] ||
        body[other] + body[otherSize] <= solid[other]
      )
        continue;
      if (
        distance > 0 &&
        body[axis] + body[size] <= solid[axis] &&
        body[axis] + body[size] + distance >= solid[axis]
      ) {
        distance = Math.min(distance, solid[axis] - body[axis] - body[size]);
        hit = true;
      } else if (
        distance < 0 &&
        body[axis] >= solid[axis] + solid[size] &&
        body[axis] + distance <= solid[axis] + solid[size]
      ) {
        distance = Math.max(distance, solid[axis] + solid[size] - body[axis]);
        hit = true;
      }
    }
    body[axis] += distance;
    if (hit) {
      if (axis === 'y' && body.vy >= 0) body.grounded = true;
      body[velocity] = 0;
    }
  }
  if (body.vy === 0)
    body.grounded = solids.some(
      (solid) =>
        Math.abs(body.y + body.height - solid.y) < 1e-7 &&
        body.x < solid.x + solid.width &&
        body.x + body.width > solid.x,
    );
  return body;
}
movePlatformBody.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'movePlatformBody(body,solids,delta,{gravity:900})',
  methods:
    'Mutates {x,y,width,height,vx,vy,grounded}; solids are static {x,y,width,height}. Delta 0..0.1 seconds. Sweeps X then Y to prevent thin-platform tunneling. Start outside solids; jump only when grounded, by assigning negative vy.',
});

export function gameGridCell(x, y, { x: left = 0, y: top = 0, width, height, columns, rows } = {}) {
  finite([x, y, left, top, width, height]);
  if (
    width <= 0 ||
    height <= 0 ||
    !Number.isInteger(columns) ||
    !Number.isInteger(rows) ||
    columns < 1 ||
    rows < 1
  )
    throw RangeError('Invalid game grid');
  if (x < left || y < top || x >= left + width || y >= top + height) return null;
  const column = Math.floor(((x - left) / width) * columns),
    row = Math.floor(((y - top) / height) * rows);
  return Object.freeze({ column, row, index: row * columns + column });
}
gameGridCell.controlFactory = Object.freeze({
  topics: ['games'],
  constructor: 'gameGridCell(x,y,{x:0,y:0,width:300,height:300,columns:3,rows:3})',
  methods:
    'Returns {column,row,index} or null outside bounds. Use native pointer viewport coordinates, with the same grid geometry as drawing. Board rules, turns, illegal moves, wins and draws belong to the app model.',
});

export * from './game-3d.mjs';
export * from './game-input.mjs';
export * from './game-hud.mjs';

export const SDK_GAME_CONTRACTS = Object.freeze({
  rendering:
    'Use createGameView with native SceneSurface + SceneBuilder, AnimationClock and GameStepper. Its internal Factory selects a WebGPU renderer Strategy, GPU-instancing consecutive rectangles/sprites and retaining texture/path resources, with CPU rendering only when usable WebGPU is unavailable. Board, card, puzzle, platform and projected-vector games share this path. No raw browser GPU access is needed or granted. Simple 3D uses projectGamePoint/drawGameBoxes on the existing native vector renderer. Keep scenes small and bounded. drawGameMesh adds low-poly faces, yaw/pitch cameras, culling, flat light and perspective-projected checker/stripe/grid procedural patterns. GameSpriteAtlas supplies trimmed RGBA sprites, pivots, flips, rotation, alpha and timed atlas animation; use the existing image decoder for external art. isometricPoint/isometricCell supply diamond projection/picking. Full spatial desktop worlds use their separate declarative world API.',
  presentation:
    'A game is a visual experience: fill the client with the game scene and design its own visual identity, HUD, buttons, inventory, pause/result screens and input feedback using GameHUD, GameSpriteAtlas and SceneBuilder. Game controls are part of the game art, not standard desktop Button/Row/Label strips around a small scene. createGameView defaults to one SceneSurface with scene-painted controls. Customize GameHUD theme, layoutHud and item.draw callbacks with vectors/sprites; drawGameText uses smooth scalable Inter outlines without font downloads or compute permissions; drawGamePixelText is opt-in retro lettering, and drawText can supply complex-script typography. Keep ordinary desktop controls for external settings/file dialogs only when useful. Readable prompts, visible focus, distinct hover/pressed/disabled states and clear keyboard hints remain required.',
  input:
    'Use native Message.POINTER and Message.KEY on a focusable SceneSurface; detail.pressed/keyCode carry key transitions (X11 ArrowLeft=113, Right=114, Up=111, Down=116, Space=65). Use GameHUD for functional game-painted buttons, health/ammo/score meters, menus and focus rings inside the scene. Use GameInput for held actions, simultaneous movement/jump and WASD/arrow aliases; fire/jump/turn moves use press edges, never OS-repeat timing. Keyboard-only navigation and mouse-assisted play are required. Clear held input on BLUR, window hide, pause, restart and close. Never capture global keys.',
  lifecycle:
    'Start paused with game-painted Play/Pause/Restart and readable in-scene instructions/status. P/Escape play/pause, R restarts, N steps, H opens help; Tab/Shift+Tab, arrows and Enter/Space navigate the HUD. Stop clocks at game end, when hidden and on close. AnimationClock honors reduced motion; provide a deterministic Step action so games remain playable without continuous animation. Reset GameStepper on pause/resume to discard elapsed wall time.',
  rules:
    'Keep authoritative game state separate from drawing. Implement collision/landing/death/goal for platform games; legal turns, occupied-cell rejection, win/draw and restart for board games; real movement, obstacles and win/loss for 3D games. Voxel sandboxes support nearest-face block picking/editing and collision; enforce map/block limits and prevent placement inside players. RPGs need stats, combat, inventory/quests and an exit condition. Shooters use gameCameraRay, pickGameBox and moveGameCamera for aiming, wall occlusion, movement, health, damage and terminal states. GameFPSMotion adds acceleration/braking, actual-distance footsteps/bob, landing weight, look sway and critically damped shot recoil; sample.pitch must affect both the camera and authoritative hit ray. Apply collision before travel and cancel blocked velocity with contact. GameImpactEffects supplies bounded gravity-driven blood/spark particles and persistent surface stains from actual occlusion-tested hits. Render its world-space particles/stains with game-owned meshes/textures, provide ground and swept-wall collision, and keep character hit/death reactions tied to real damage. These helpers do not supply authored combat clips, sound, ragdolls or dismemberment. A sandbox does not require an invented victory condition. Board/card games commit rules immediately and request finite GameTransitions through createGameView animate/view.animate(); settle to no idle loop. Save app-owned scores/settings only when requested, with exact settings permissions.',
  animation2d:
    'Board and card games should animate real moves: deal/fan/flip/lift/capture, stone placement, legal-move focus and win feedback. Use GameTransitions for finite numeric visual state, createGameView({continuous:false,animate}) and view.animate() after changes; return false when settled. Keep authoritative rules immediate and independent of tween completion. Reuse GameDeck for owned unique cards and atomic draw/deal/discard/recycle, then implement the requested card rules, turns, legal hands and terminal states. Cards need coherent illustrated faces/backs, softened material edges, contact shadows, actual animated hit bounds, keyboard selection, readable face values and reveal/action locking that prevents double rewards. Reduced motion settles visual work; hidden/closed windows stop clocks. Never retain a permanent idle loop just for a board/table. Use optional layoutGame2DHud for a small unframed status and compact rounded game menu, with shortcuts inside Help, not desktop Button rows.',
  platformer:
    'Use GamePlatformMap for editable ricewm.platform-map/v1 JSON: grounded spawn, bounded world, goal, solid/oneWay platforms, unique pickups, supported patrols and decorative atlas placements. GamePlatformSession shares this geometry for acceleration, swept collision, coyote/buffered variable-height jumps, real enemy patrols, descending-contact stomps versus contact damage, invulnerability, health, exactly-once score/pickups and win/loss. Camera follows the actual player across a level wider than the viewport. Every player, enemy and NPC must have its own complete, coherent multi-frame animation set for its real actions: idle, walk/run or fly, takeoff/airborne/landing where applicable, attack or interaction, hurt and defeat. Use distinct drawn poses with consistent silhouettes, scale, pivots and transparent gutters; a single overall sprite template, repeated identical cells, a bobbing static image or camera motion is not character animation. Plan named state clips and measured source frames before generating sheets; split sheets per actor/type when needed to respect atlas budgets. Drive GameSpriteAtlas.sample from actual actor state, velocity and independent animation time, with correct looping and one-shot completion; grounded movement, attacks, damage and defeat must change the visible pose. Reuse a complete species animation set across instances with independent phases. Verify every actor type in actual gameplay and inspect at least two distinct drawn frames per active clip, state transitions, pause/hide/reduced-motion behavior, and no per-frame image generation or decoding before claiming completion. Use GameSpriteAtlas idle/run/jump/hurt/enemy clips driven by actual velocity/time, filtered detailed artwork, varied collectible/vegetation/prop frames, coin/kill point feedback and a clear goal. drawGameParallax uses the real camera with distinct distant sky/mountain, mid forest/architecture and near foreground depth, transparent plates and independent cloud/wind drift, coherent horizon/lighting and seamless wrapping; a static wallpaper or repeated rectangles is insufficient. The SDK contains no artwork pack: createGameStarterAtlas requires this game\'s own pixels/frames, and loadGameAtlas reads this game\'s own generated PNG through the approved image decoder. Generate/source new coherent high-resolution sprites and layered plates for the exact requested setting; retain source/rights/prompt records and correct trimmed pivots/gutters. Author and verify reachable map routes, gaps, enemy losses, score, complete keyboard play, motion, parallax at two camera positions, resize and cleanup. Schema validation alone does not prove a reachable or fun level.',
  progression:
    'Use GameCharacter for app-authored classes, base/growth/equipment stats, health/mana/regen, XP/levels, earned skill points, prerequisite/class/level-gated skill trees, spell mana costs and cooldowns. cast returns a real effect receipt after spending resources; validate target range/team/line of sight before cast, then apply its damage/heal/resource effects to authoritative actors. Connect actor HP and equipment to this same state. Use rollGameLoot with real GameInventory definitions, deterministic encounter random when useful, rarity/quantity, full-inventory ground drops and exactly-once boss/quest rewards. Story/dialogue/NPC choices, encounter design, skill-tree UI, visual spell effects and campaign content remain authored app data and logic. GameQuestLog provides the progression contract; a complete story is not a prewritten SDK catalog.',
  openWorld:
    'Use GameChunkWorld and gameSeededRandom for huge explorable terrain with deterministic chunk generation, negative/global coordinates, a bounded resident cache and explicit app-owned persistence. Load around the player, retain authoritative actor/quest/loot coordinates globally, make a bounded GameTileWorld snapshot with its origin for near-player rendering/navigation/fog, and convert local/global coordinates consistently. Persist defeated enemies, opened chests, buildings, quest flags and terrain edits before unloading; rebase active actors/camera when changing the simulation window. Dirty eviction, failed persistence or failed generation must stop streaming and preserve resident progress. A huge open world streams bounded nearby simulation; never claim literal infinite loaded memory or silently regenerate saved state. The generator/store are explicit app implementations, not network authority or invented services. Use exact settings/file permissions for the chosen save format.',
  graphics3d:'For realistic perspective, isometric, RPG, RTS, voxel and FPS 3D games use openGameRenderer({services,view,width:960,height:540}) and graphics.open/upload/render/close permissions. Create the owned createGameView first with a loading state, return from start promptly, and prepare assets/graphics asynchronously in that same view. Use createCancellationScope for cancellation in the restricted app realm. renderer.draw(scene,frame) composes a GPU-backed frame without CPU pixel transport; the same method handles the explicit unavailable-WebGPU fallback. Close during loading must cancel work and release any opened renderer. Never lengthen or bypass the app responsiveness watchdog. Muzzle flashes need brief combustion frames, correctly transformed barrel sockets, warm transient illumination and separately advected smoke; use original translucent raster art instead of a static geometric star or screen gradient. Fixed package-owned WebGPU shaders support physical materials with authored normals and base-color/normal/roughness/metalness textures, environment lighting, real bounded shadow maps, fog, bloom and tone mapping. Use createGameRig for full glTF/GLB characters, normalized four-weight skin attributes and independently sampled continuous skeletal poses; GPU skinning deforms the actual model rather than replacing it with rigid primitives. Upload resources once; render camera/object/light frames with at most one pending request, then call renderer.draw(scene,frame) beneath crisp native HUD art. Do not use the legacy RGBA/readback mode for new generated games. Orthographic camera projection and height provide a real 3D isometric world; perspective is the default. Source mesh generation, simulation and one-time font/asset preparation may use CPU; do not rasterize live 3D models, terrain, fog or particles into per-frame CPU pixel buffers when WebGPU is available. Parse detailed glTF/GLB/OBJ using explicit profile:"gpu" and preserve resource/source notices. Discover graphics.context, then report the actual open/inspect backend, fallback reason, software/adapter classification, application readbacks and measured throughput. createGameView inspection includes renderer diagnostics; a preferred backend alone is not evidence of use. Stop on window hide/close; close releases shared GPU admission and resources. No raw app WebGL, DOM, shaders, pointer lock, URL loader or unbounded engine authority. Visual references require actual pixel review of architecture, models, materials, lighting, effects and HUD padding; a functional box-room prototype is not a quality pass.',
  visualQuality:
    'Default to smooth native Inter outline text and filtered high-resolution art. Pixel lettering, nearest-neighbour sprites, crude placeholder circles, visibly repeating tiny diamond textures and ordinary desktop button strips are not acceptable for realistic RTS/action-RPG requests. drawGamePixelText/smoothing:false are only for an explicitly requested retro style. Use actual photographic PBR materials through graphics3d for live 3D, with coherent detailed vegetation/architecture, correctly scaled sampled humanoid animations and grounded lighting/shadows. Render fog from GameFogOfWar sight/exploration, not a decorative dark vignette. Hide unrevealed enemies from rendering, minimap, targeting and selection. Use scene-painted game controls, readable meters/skills/inventory, good contrast and appropriate focus/hover/disabled feedback; game art must cover the useful client. Keep the HUD compact: health/mana, a small skill/potion bar, minimap and tracked objective. Use contextual interactions and clear artwork icons; put key bindings and control instructions in an on-demand Help menu, with no permanent shortcut-key strip. Open full inventory, journal, character and skill-tree panels on demand. Avoid permanent large Play/Attack/Talk/Inventory button strips across the playfield; pause/help/start controls can live in a compact menu with keyboard access. Budget source resolution and pre-bakes for the real output size; never upscale 32/48px placeholders and call them realistic. Review actual rendered pixels against the supplied reference and revise. The bounded CPU/sprite path does not imply visual or performance parity with a full GPU/PBR engine; state unsupported effects and measured limits honestly.',
  materials:
    'Herdr gameAssets exposes sdk.assets.search/files/import/inspect for selected public HTTPS artwork, live Poly Haven textures/models and project-local provenance. Select resolution-appropriate textures and OBJ/MTL or glTF/GLB; explicitly package every buffer/material/image and its notice. sdk.resources image reuses the existing PNG/JPEG decoder; prepare static atlases within GameSpriteAtlas budgets or upload decoded textures to the owned graphics3d renderer. parseGameOBJ/parseGameMTL/parseGameGLTF/parseGameGLB do not fetch anything. rasterizeGameModel supplies actual diffuse UV textures, near clipping, depth buffer and orthographic or perspective cameras. parseGameGLTF/GLB samples LINEAR/STEP node animation and four-weight skeletal skinning; bakeGameModelAtlas creates reusable directional sprite animations. Bake static props into an atlas once; use GameSpriteAtlas for directional/frame animation with smoothing:true; use high-resolution source frames sized for the actual viewport. Do not download a model and claim it is rendered without loading its actual materials and reviewing pixels. Use graphics3d for supported normal/roughness/metallic PBR maps. Unsupported animation interpolation, morph targets and required extensions need an actual conversion/rendering capability. Never invent one or silently substitute decorative placeholders.',
  strategy:
    'For Age of Empires/0 A.D./RTS requests, use GameTileWorld + GameNavigation + GameStrategy and actual model/texture assets. Author unit/building definitions, map regions, team orders and enemy decision logic; use paid resource gathering/construction/training and explicit objectives. Bind click/drag selection, formation destinations, contextual attack/work/build orders, visible command/selection feedback and a keyboard-only equivalent. The simulation uses bounded A*, dynamic footprints, continuous motion with local separation, line-of-sight combat and deterministic fixed updates. Use sampled glTF skeletal poses with graphics3d GPU skinning for live walk/work/attack/death. Pre-baked bakeGameModelAtlas sprites are an explicit 2D presentation choice, not a per-frame CPU rasterizer. Ensure terrain, unit scale, buildings, weapons, UI and photographic diffuse art form one coherent visual direction. Show resources, selected unit stats/actions, build placement validity, progress, minimap, production feedback and victory/defeat. Validate crossing/converging units, obstruction changes, unreachable orders, placement/cost atomicity, worker gathering, construction, training, enemy combat and a complete scenario. 256 units/128 buildings per simulation and bounded sprite/raster budgets do not establish performance of a large battle. Use GameFogOfWar with opaque terrain/buildings for authoritative current sight and persistent exploration; update GPU-visible fog geometry or bounded dirty fog textures when sight changes, and omit hidden enemies from world/selection/minimap/targeting. Use graphics3d textured ground and blended materials with detailed vegetation and buildings at consistent camera angles. rasterizeGameTerrain is available for one-time static preparation or the unavailable-WebGPU fallback. Faction economies, technologies, campaign rules, large-army crowd solving and multiplayer require explicit app implementation; The CPU path does not supply PBR; use the separate graphics3d contract and openGameRenderer for physical GPU rendering.',
  artDirection:
    'Establish a game-specific art direction from the requested setting before composing the HUD: story, architecture/silhouettes, palette, material language, motifs, typography, resource vessels, item/spell art and panel composition must agree. Ashfall/Diablo-like iron and ember art is only one probe; wizard-school, elven, dwarven, epic fantasy or other requested themes require their own original generated/sourced artwork and procedural recipes, not recolored copies of the same template. Use measured atlas source bounds with transparent padding so adjacent artwork cannot bleed into controls; never assume generated artwork exactly fills an equal grid. Measure all text against its real interior bounds; keep tooltips on screen and expand/reflow controls before text can be clipped. Use GameHUDSkin with the game-supplied atlas for artwork-based semantic controls, nine-slice material panels, resource-vessel depletion, inset item slots and radial cooldowns. Use drawGameText font:serif for licensed Playfair Display narrative headings, body for readable Inter details; keep body text legible and do not bake labels/stats into artwork. Set createGameBuilding roofStyle/palette and createGameMaterial baseColor as real procedural parameters; bind distinct photographic/generated wall/roof/wood textures as appropriate. Code-driven variation and AI-generated art are separate capabilities: use a real available image-generation tool or source/import selected suitable artwork, never invent a Herdr generation endpoint or claim arbitrary polished asset generation from a seed. Validate actual atlas coordinates, scene-image budgets, visual fidelity and runtime pixels. A capability probe is not the final art direction for all generated games.',
  proceduralMaps:
    'Use GameMap for validated ricewm.game-map/v1 data: a procedural seed/biomes/settlements plus authored terrain patches and building/prop/NPC/enemy/quest/loot placements. Connect chunk generation to map.chunk and query map.region for active-window content. Use exactly the same global coordinates/data for editor, world rendering, navigation, fog and encounter spawning. Provide an on-demand world designer with terrain brushes, building placement/removal, collision feedback and granted map JSON save/load. The generated default roads connect regions; built-in settlements contain actual cottages, chapels, towers and keeps. Use createGameTree for lit volumetric oak/pine/birch geometry with real trunks, branches and folded leaves; upload that geometry to graphics3d for live lighting, wind motion and ground shadows. Static baking is an explicit 2D asset-preparation option. Photographic tree cutouts alone do not establish 3D vegetation. Use createGameBuilding for modular textured triangle meshes with roofs, timber beams, windows, doors, steps and authoritative footprints; moonwell, observatory and gateway add open colonnades, round basins, ribbed domes, armillary rings and curved arches. A gateway returns local collision rectangles for its columns so the passage can remain walkable; upload geometry once to graphics3d; explicit 2D presentations may bake static assets once using rasterizeGameModel, consistent pivots and actual output resolution. createGameMaterial supplies seeded shingle/wood/masonry diffuse textures; imported photographic wall/roof materials can replace them. Stream chunks as the player walks, follow the player with the camera, rebase active navigation/actors/fog, and preserve global texture UVs through streamed mesh coordinates (or rasterizeGameTerrain uvOrigin for static atlas preparation). A fixed-screen tiny island or arbitrary teleport button is not proof of an explorable large world. Save map design separately from defeated enemies, opened loot and quest progression. Verify authored map round-trip, placement overlap rejection, real walking across chunk boundaries, return visits and changes surviving unload/reload.',
  spellEffects:
    'Use GameEffects for native world-space fire projectiles with trails and impact light, seeded branching lightning, expanding nova/shockwaves, sparks and dissipating smoke; frost adds traveling ice lances and growing/melting impact crystals, while moonbeam adds descending rings, glyph circles and rising star motes. Pay GameCharacter mana/cooldown once, store the cast receipt by effect ID, and apply authoritative damage on the single drainImpacts receipt. Effects must follow the same live camera/world projection and stop while paused/hidden/closed. Validate visibility/range/team/line of sight before casting, omit unrevealed effects and keep target/reward state coherent if an actor dies or unloads. Bound active effects and raster/scene work; these are layered native vectors/gradients, not GPU particle/PBR parity. Capture actual travel and impact frames, prove mana spending, delayed damage, exactly-once impact and cleanup.',
  actionRpg:
    'For Diablo-style/isometric fantasy requests, author an original action RPG with its own story/art. Start with a bounded, complete playable chapter and explicitly describe its scale. GameStrategy and GameNavigation supply unit selection, continuous movement, dynamic building footprints, A* paths, formations, explicit attack/chase orders, worker harvesting, construction and training. GameTileWorld provides camera-consistent picking, collision, shortest paths, viewport culling and depth-sorted atlas drawing. GameInventory provides item stacks, capacity, equipment slots and real stat modifiers. GameQuestLog provides prerequisites, event-driven objectives and exactly-once completion. Author region transitions, checkpoints, NPC dialogue/choices, shops if requested, loot tables/rarity, weapon/armour comparison, health/resource costs, enemy aggro/pathfinding, cooldowns, hit feedback, death/recovery and boss telegraph/wind-up/recovery phases as app state. Give every quest a start, progress feedback and reward/ending; ensure regions, objectives and exits are reachable. Keep combat deterministic under Step, and pause simulation while dialogue/inventory/journal/help is open. Design an integrated fantasy HUD with health/resource meters, skills/potions and cooldown/disabled feedback, tracked objective, contextual interaction prompt, minimap and readable inventory/journal/equipment panels. Mouse click-to-move/target and keyboard movement/action/UI navigation share the same state; panel hit regions must not leak into world movement. Include keyboard-only completion, enemy/boss loss, loot/equip stat change, NPC quest acceptance/turn-in, region travel, win, restart, resize and close in acceptance. Save/resume is app-owned with exact settings permissions when requested. The existing game-isometric scaffold is a small starting example, not a completed large RPG.',
  starters: Object.freeze({
    'game-3d': 'Three-lane perspective obstacle course',
    'game-card': 'Illustrated memory cards with staggered deal, flips, pair matching and points',
    'game-platform': 'Illustrated scrolling platform adventure: configurable map, parallax, animated sprites, enemies, pickups, score and health',
    'game-board': 'Animated inlaid board, flying carved stones, legal turns and drawn winning line',
    'game-voxel': 'Minecraft-style bounded block building sandbox',
    'game-isometric': 'Isometric sprite RPG with combat, gem and key quest',
    'game-shooter': 'First-person arena with aiming, wall collision, hitscan, health and targets',
  }),
  acceptance:
    'Verify meaningful movement, collision/illegal moves, terminal win/loss or draw, pause, restart, a full keyboard-only primary journey, held key-up and simultaneous movement/jump or movement/aim, HUD pointer hit regions/cancel/disabled/focus, aspect-preserving resize and hidden/close cleanup. Inspect real game pixels for integrated HUD art rather than office control rows. Use sdk.develop/interact/inspect/visual and review real pixels; a screenshot alone does not prove gameplay. Keep voxel worlds bounded and cull hidden/internal geometry for larger scenes. This is a bounded native vector/atlas/model toolkit with explicit chunk streaming and app-owned distant simulation. Vector painter sorting does not resolve arbitrary polygon intersections; rasterizeGameModel has a local depth buffer for static textured model batches. Use the owned graphics3d renderer for PBR shading. Multiplayer, gamepads, arbitrary WebGL/DOM and physics engines are not supplied by these helpers.',
});
