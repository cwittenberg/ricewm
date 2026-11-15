import { game2DStarterSource } from './game-2d-starters.mjs';
import { gameWorldStarterSource } from './game-world-starters.mjs';
/** Small original examples using shared native game helpers. */
export function gameStarterSource(template,options={}) {
  const twoD = game2DStarterSource(template,options);
  if(twoD!==null)return twoD;
  if (template === 'game-3d')
    return `import { defineApp, createGameView, GameHUD, drawGameBoxes } from '@ricewm/app';
export default defineApp({
  start({ createWindow }) {
    const obstacles = [
      { lane: 0, z: 8 },
      { lane: -1, z: 14 },
      { lane: 1, z: 20 },
      { lane: 0, z: 26 },
      { lane: -1, z: 32 },
    ];
    let lane = 0,
      distance = 0,
      ended = false,
      view;
    function reset() {
      lane = 0;
      distance = 0;
      ended = false;
    }
    view = createGameView({
      createWindow,
      hud: new GameHUD({theme:{accent:0xffffcf80}}),
      instructions:
        'P starts. A/D or Left/Right change lanes. Dodge the blocks to reach 38 metres. N steps while paused.',
      reset() {
        reset();
        view.setStatus('Ready: 0 / 38 m');
      },
      actions: [
        {
          name: 'left',
          text: 'Left',
          keys: [113, 38],
          run() {
            if (!ended) lane = Math.max(-1, lane - 1);
          },
        },
        {
          name: 'right',
          text: 'Right',
          keys: [114, 40],
          run() {
            if (!ended) lane = Math.min(1, lane + 1);
          },
        },
      ],
      update(delta) {
        if (ended) return false;
        const previous = distance;
        distance += delta * 4;
        if (
          obstacles.some(
            (box) => box.lane === lane && previous < box.z + 0.8 && distance >= box.z - 0.8,
          )
        ) {
          ended = true;
          view.setStatus('Collision! Restart to try again.');
          view.stop();
          return false;
        }
        if (distance >= 38) {
          ended = true;
          view.setStatus('You win! Course complete.');
          view.stop();
          return false;
        }
        view.setStatus('Distance ' + Math.floor(distance) + ' / 38 m');
      },
      render(scene) {
        scene.rect([0, 0, 640, 360], 0xffecdcc8);
        const boxes = [
          { x: -3, y: -0.3, z: 0, width: 6, height: 0.3, depth: 46, color: 0xff9d9b8c },
        ];
        for (const obstacle of obstacles)
          boxes.push({
            x: obstacle.lane * 1.8 - 0.55,
            y: 0,
            z: obstacle.z,
            width: 1.1,
            height: 1.2,
            depth: 1.1,
            color: 0xff537cb8,
          });
        boxes.push({ x: -3, y: 0, z: 40, width: 6, height: 2, depth: 0.15, color: 0xff62a266 });
        drawGameBoxes(scene, boxes, {
          width: 640,
          height: 360,
          x: lane * 1.8,
          y: 1.8,
          z: distance - 2,
          focalLength: 260,
          near: 0.2,
        });
      },
    });
    view.setStatus('Ready: 0 / 38 m');
  },
});
`;
  return gameWorldStarterSource(template);
}
gameStarterSource.controlFactory = Object.freeze({
  topics: ['games'],
  constructor:
    'gameStarterSource("game-3d"|"game-platform"|"game-board"|"game-card"|"game-voxel"|"game-isometric"|"game-shooter")',
  methods:
    'Returns complete readable app.mjs source from the selected SDK. Herdr sdk.scaffold accepts the same template names. Board/card starters use finite deal/flip/piece/win animation and settle when idle. Platform uses editable ricewm.platform-map/v1 map data, real enemy/pickup/score state, layered parallax and illustrated sprite animation. Optional gameStarterSource("game-platform",{map}) embeds a custom validated map. Card/platform source requires {artwork:{path,frames,animations}} for newly generated project assets; without it the scaffold reports an unfinished artwork draft. No artwork pack is bundled. Adapt the game-specific art and model/rules; keep native input, lifecycle and verification.',
});
