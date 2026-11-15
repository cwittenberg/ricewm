/** Small original examples using shared native game helpers. */
export function gameWorldStarterSource(template) {
  if (template === 'game-voxel')
    return `import {
  defineApp,
  createGameView,
  GameHUD,
  drawGameBoxes,
  gameCameraRay,
  pickGameBox,
  moveGameCamera,
  gameOverlaps,
} from '@ricewm/app';
export default defineApp({
  start({ createWindow }) {
    let blocks, camera, view;
    function reset() {
      camera = {
        width: 640,
        height: 360,
        x: 0,
        y: 1.6,
        z: -4,
        yaw: 0,
        pitch: -0.12,
        focalLength: 300,
        near: 0.12,
      };
      blocks = [
        { x: -8, y: -0.25, z: -8, width: 16, height: 0.25, depth: 16, color: 0xff7b9a78 },
        {
          x: -0.5,
          y: 0,
          z: 2,
          width: 1,
          height: 1,
          depth: 1,
          color: 0xff6ea575,
          pattern: { kind: 'checker', cells: 4, color: 0xff5b8960 },
        },
        {
          x: 2.5,
          y: 0,
          z: 3,
          width: 1,
          height: 1,
          depth: 1,
          color: 0xff6594b0,
          pattern: { kind: 'stripes', cells: 4, color: 0xff51768d },
        },
        { x: 2.5, y: 1, z: 3, width: 1, height: 1, depth: 1, color: 0xff7d9c75 },
        { x: -3.5, y: 0, z: 4, width: 1, height: 1, depth: 1, color: 0xff9e9590 },
      ];
    }
    const status = (text) =>
      view.setStatus((text ? text + ' · ' : '') + (blocks.length - 1) + ' blocks');
    function walk(forward) {
      if (view.playing) return;
      camera = moveGameCamera(camera, blocks, { forward, distance: 0.4 });
      status('Explore');
    }
    function edit(place) {
      const hit = pickGameBox([camera.x, camera.y, camera.z], gameCameraRay(camera), blocks, {
        maxDistance: 8,
      });
      if (!hit || hit.index === 0) {
        status('Aim at a nearby block');
        return;
      }
      if (!place) {
        blocks.splice(hit.index, 1);
        status('Block removed');
        return;
      }
      if (blocks.length >= 97) {
        status('96 block limit');
        return;
      }
      const target = blocks[hit.index],
        block = {
          x: target.x + hit.normal[0],
          y: target.y + hit.normal[1],
          z: target.z + hit.normal[2],
          width: 1,
          height: 1,
          depth: 1,
          color: 0xff6ea575,
          pattern: { kind: 'checker', cells: 4, color: 0xff5b8960 },
        };
      if (
        block.y < 0 ||
        blocks.some((b) => b.x === block.x && b.y === block.y && b.z === block.z)
      ) {
        status('Space occupied');
        return;
      }
      if (
        camera.y > block.y &&
        camera.y - 1.6 < block.y + 1 &&
        gameOverlaps(
          { x: camera.x - 0.2, y: camera.z - 0.2, width: 0.4, height: 0.4 },
          { x: block.x, y: block.z, width: 1, height: 1 },
        )
      ) {
        status('Player occupies that space');
        return;
      }
      blocks.push(block);
      status('Block placed');
    }
    reset();
    view = createGameView({
      createWindow,
      hud: new GameHUD({theme:{accent:0xff9cddaa,panel:0xee242d16}}),
      instructions: 'P starts. Hold WASD to walk/strafe; arrows turn/walk; drag to look. Enter places; Space removes.',
      reset() {
        reset();
        status('New world');
      },
      drag({ dx, dy }) {
        camera.yaw += dx * 0.008;
        camera.pitch = Math.max(-1.2, Math.min(1.2, camera.pitch - dy * 0.008));
      },
      actions: [
        {name:'strafeLeft',text:'A',keys:[38],run(){}},
        {name:'strafeRight',text:'D',keys:[40],run(){}},
        {
          name: 'forward',
          text: 'Forward',
          keys: [111, 25],
          run() {
            walk(1);
          },
        },
        {
          name: 'back',
          text: 'Back',
          keys: [116, 39],
          run() {
            walk(-1);
          },
        },
        {
          name: 'left',
          text: 'Turn left',
          keys: [113],
          run() {
            if (!view.playing) camera.yaw -= 0.12;
          },
        },
        {
          name: 'right',
          text: 'Turn right',
          keys: [114],
          run() {
            if (!view.playing) camera.yaw += 0.12;
          },
        },
        {
          name: 'place',
          text: 'Place block',
          keyCode: 36,
          run() {
            edit(true);
          },
        },
        {
          name: 'remove',
          text: 'Remove block',
          keyCode: 65,
          run() {
            edit(false);
          },
        },
      ],
      update(delta, { input }) {
        camera.yaw += input.axis('left','right') * delta * 1.8;
        const forward=input.axis('back','forward'),strafe=input.axis('strafeLeft','strafeRight');
        if (forward || strafe) {
          camera = moveGameCamera(camera,blocks,{forward,strafe,distance:delta*3});
          status('Explore');
        }
      },
      render(scene) {
        scene.rect([0, 0, 640, 360], 0xffead4b8);
        drawGameBoxes(scene, blocks, camera, { maximumFaces: 8192 });
        scene
          .line([312, 180], [328, 180], 2, 0xfffafafa)
          .line([320, 172], [320, 188], 2, 0xfffafafa);
      },
    });
    status('New world');
  },
});
`;
  if (template === 'game-isometric')
    return `import { defineApp, createGameView, GameHUD, GameSpriteAtlas, isometricPoint, isometricCell } from '@ricewm/app';
export default defineApp({
  start({ createWindow }) {
    const rgba = new Uint8Array(32 * 24 * 4);
    // Two original transparent character frames with shading and separate feet.
    for (let f = 0; f < 2; f++)
      for (let y = 0; y < 24; y++)
        for (let x = 0; x < 16; x++) {
          let color = null;
          if (y >= 2 && y < 9 && x >= 5 && x < 11)
            color = y < 4 ? [50, 53, 73, 255] : [238, 188, 137, 255];
          if (y >= 9 && y < 19 && x >= 3 && x < 13)
            color = x < 7 ? [55, 104, 163, 255] : [78, 138, 192, 255];
          if (
            y >= 19 &&
            y < 23 &&
            ((x >= 4 && x < 7) || (x >= 9 && x < 12)) &&
            !(f === 1 && y === 22 && x < 7)
          )
            color = [49, 47, 65, 255];
          if (color) rgba.set(color, (y * 32 + x + f * 16) * 4);
        }
    const sprites = new GameSpriteAtlas({
      width: 32,
      height: 24,
      rgba,
      frames: {
        idle: { x: 0, y: 0, width: 16, height: 24 },
        walk: { x: 16, y: 0, width: 16, height: 24 },
      },
      animations: {
        walk: [
          { frame: 'idle', duration: 0.2 },
          { frame: 'walk', duration: 0.2 },
        ],
      },
    });
    const projection = {
        tileWidth: 56,
        tileHeight: 28,
        originX: 320,
        originY: 96,
        elevationHeight: 28,
      },
      walls = new Set(['1,2', '2,2', '4,3']);
    let player, guardian, gem, key, ended, time, view;
    function reset() {
      player = { column: 0, row: 0, hp: 5 };
      guardian = { column: 3, row: 3, hp: 2 };
      gem = false;
      key = false;
      ended = false;
      time = 0;
    }
    function status(message = '') {
      view.setStatus(
        message ||
          'HP ' + player.hp + ' · Gem ' + (gem ? '1' : '0') + '/1 · Key ' + (key ? 'yes' : 'no'),
      );
    }
    function retaliate() {
      if (
        guardian.hp > 0 &&
        Math.abs(player.column - guardian.column) + Math.abs(player.row - guardian.row) === 1
      ) {
        player.hp--;
        if (player.hp <= 0) {
          ended = true;
          status('Defeated! Restart to try again.');
          view.stop();
        }
      }
    }
    function move(dc, dr) {
      if (ended) return;
      const c = player.column + dc,
        r = player.row + dr;
      if (
        c < 0 ||
        r < 0 ||
        c > 5 ||
        r > 5 ||
        walls.has(c + ',' + r) ||
        (guardian.hp > 0 && c === guardian.column && r === guardian.row)
      ) {
        status('Path blocked');
        return;
      }
      player.column = c;
      player.row = r;
      time += 0.2;
      if (c === 4 && r === 1) gem = true;
      retaliate();
      if (ended) return;
      if (c === 5 && r === 5) {
        if (gem && key) {
          ended = true;
          status('Quest complete! Gem and key delivered.');
          view.stop();
          return;
        }
        status('The gate needs the gem and guardian key');
        return;
      }
      status();
    }
    function attack() {
      if (ended) return;
      if (
        guardian.hp <= 0 ||
        Math.abs(player.column - guardian.column) + Math.abs(player.row - guardian.row) !== 1
      ) {
        status('No adjacent guardian');
        return;
      }
      guardian.hp--;
      if (guardian.hp === 0) key = true;
      else retaliate();
      if (!ended) status();
    }
    reset();
    view = createGameView({
      createWindow,
      hud: new GameHUD({theme:{accent:0xff82c6df,panel:0xee222d31}}),
      instructions:
        'WASD/Arrows move. Space/Enter attacks. Click an adjacent tile to move or the adjacent guardian to attack. Collect gem and key, reach the gate.',
      reset() {
        reset();
        status();
      },
      pointer({x,y}) {
        const tile=isometricCell(x,y,projection);
        if (!tile) return;
        const dc=tile.column-player.column,dr=tile.row-player.row;
        if (Math.abs(dc)+Math.abs(dr)!==1) return;
        if (tile.column===guardian.column&&tile.row===guardian.row&&guardian.hp>0) attack();
        else move(dc,dr);
      },
      update(delta) {
        if (ended) return false;
        time += delta;
      },
      actions: [
        {
          name: 'left',
          text: 'West',
          keys: [113, 38],
          run() {
            move(-1, 0);
          },
        },
        {
          name: 'right',
          text: 'East',
          keys: [114, 40],
          run() {
            move(1, 0);
          },
        },
        {
          name: 'up',
          text: 'North',
          keys: [111, 25],
          run() {
            move(0, -1);
          },
        },
        {
          name: 'down',
          text: 'South',
          keys: [116, 39],
          run() {
            move(0, 1);
          },
        },
        { name: 'attack', text: 'Attack', keys: [65, 36], run: attack },
      ],
      render(scene, { hud }) {
        hud.set('health',{type:'meter',rect:[12,52,104,12],value:player.hp/5});
        scene.rect([0, 0, 640, 360], 0xffe7eeed);
        let path = 1;
        for (let depth = 0; depth <= 10; depth++)
          for (let row = 0; row < 6; row++) {
            const column = depth - row;
            if (column < 0 || column > 5) continue;
            const p = isometricPoint(column, row, 0, projection),
              wall = walls.has(column + ',' + row);
            scene
              .definePath(path, [
                ['M', p.x, p.y],
                ['L', p.x + 28, p.y + 14],
                ['L', p.x, p.y + 28],
                ['L', p.x - 28, p.y + 14],
                ['Z'],
              ])
              .fillPath(path++, wall ? 0xff7c8185 : (column + row) % 2 ? 0xff9fb997 : 0xffabc8a4);
            if (column === 5 && row === 5) scene.rect([p.x - 10, p.y + 2, 20, 20], 0xff4b994c);
            if (!gem && column === 4 && row === 1)
              scene.ellipse([p.x - 6, p.y + 6, 12, 12], 0xff41cbef);
            if (guardian.hp > 0 && column === guardian.column && row === guardian.row)
              scene.ellipse([p.x - 10, p.y - 8, 20, 25], 0xff5454c2);
            if (column === player.column && row === player.row) {
              scene.ellipse([p.x - 10, p.y + 10, 20, 8], 0x40403020);
              sprites.draw(scene, 1, sprites.sample('walk', time), {
                x: p.x,
                y: p.y + 16,
                scale: 1.8,
              });
            }
          }
      },
    });
    status();
  },
});
`;
  if (template === 'game-shooter')
    return `import {
  defineApp,
  createGameView,
  GameHUD,
  drawGameBoxes,
  gameCameraRay,
  pickGameBox,
  moveGameCamera,
} from '@ricewm/app';
export default defineApp({
  start({ createWindow }) {
    const world = [
      { x: -5, y: -0.25, z: -5, width: 10, height: 0.25, depth: 10, color: 0xff9d9993 },
      {
        x: -5,
        y: 0,
        z: -5,
        width: 1,
        height: 3,
        depth: 10,
        color: 0xff9c8080,
        pattern: { kind: 'checker', cells: 4, color: 0xff806868 },
      },
      { x: 4, y: 0, z: -5, width: 1, height: 3, depth: 10, color: 0xff9c8080 },
      { x: -5, y: 0, z: -5, width: 10, height: 3, depth: 1, color: 0xff9c8080 },
      {
        x: -5,
        y: 0,
        z: 4,
        width: 10,
        height: 3,
        depth: 1,
        color: 0xff9c8080,
        pattern: { kind: 'stripes', cells: 4, color: 0xff806868 },
      },
    ];
    let camera, targets, hp, kills, cooldown, ended, view;
    const status = (text) => view.setStatus(text || 'Health ' + hp + ' · Targets ' + kills + '/3');
    function reset() {
      camera = {
        width: 640,
        height: 360,
        x: 0,
        y: 1,
        z: -3,
        yaw: 0,
        pitch: 0,
        focalLength: 300,
        near: 0.12,
      };
      hp = 3;
      kills = 0;
      cooldown = 0;
      ended = false;
      targets = [0, -2, 2].map((x) => ({
        x: x - 0.35,
        y: 0,
        z: x === 0 ? 1 : 2,
        width: 0.7,
        height: 1.6,
        depth: 0.7,
        color: 0xff536fc1,
        hp: 2,
      }));
    }
    function walk(forward) {
      if (!ended && !view.playing) {
        camera = moveGameCamera(camera, world, { forward, distance: 0.4, height: 1 });
        status();
      }
    }
    function fire() {
      if (ended) return;
      const alive = targets.filter((t) => t.hp > 0),
        hit = pickGameBox(
          [camera.x, camera.y, camera.z],
          gameCameraRay(camera),
          [...world, ...alive],
          { maxDistance: 20 },
        );
      if (!hit || hit.index < world.length) {
        status('Shot blocked by wall');
        return;
      }
      const target = alive[hit.index - world.length];
      target.hp--;
      if (target.hp === 0) kills++;
      if (kills === 3) {
        ended = true;
        status('You win! All targets disabled.');
        view.stop();
      } else status();
    }
    reset();
    view = createGameView({
      createWindow,
      hud: new GameHUD({theme:{accent:0xffe0d06c,panel:0xee282219}}),
      instructions:
        'P starts. Hold WASD to move/strafe, arrows to turn/walk; drag to aim. Click or Space/Enter fires. Two hits per target.',
      reset() {
        reset();
        status();
      },
      pointer({button}) { if (button===1) fire(); },
      drag({ dx, dy }) {
        if (!ended) {
          camera.yaw += dx * 0.008;
          camera.pitch = Math.max(-1.1, Math.min(1.1, camera.pitch - dy * 0.008));
        }
      },
      actions: [
        {name:'strafeLeft',text:'A',keys:[38],run(){}},
        {name:'strafeRight',text:'D',keys:[40],run(){}},
        {
          name: 'forward',
          text: 'Forward',
          keys: [111, 25],
          run() {
            walk(1);
          },
        },
        {
          name: 'back',
          text: 'Back',
          keys: [116, 39],
          run() {
            walk(-1);
          },
        },
        {
          name: 'left',
          text: 'Turn left',
          keys: [113],
          run() {
            if (!ended && !view.playing) camera.yaw -= 0.12;
          },
        },
        {
          name: 'right',
          text: 'Turn right',
          keys: [114],
          run() {
            if (!ended && !view.playing) camera.yaw += 0.12;
          },
        },
        { name: 'fire', text: 'Fire', keys: [65, 36], run: fire },
      ],
      update(delta, { input }) {
        if (ended) return false;
        camera.yaw += input.axis('left','right') * delta * 1.8;
        camera = moveGameCamera(camera,world,{forward:input.axis('back','forward'),strafe:input.axis('strafeLeft','strafeRight'),distance:delta*3,height:1});
        cooldown += delta;
        if (cooldown < 2) return;
        cooldown -= 2;
        const seen = targets.some((target) => {
          if (target.hp <= 0) return false;
          const origin = [target.x + 0.35, 1, target.z + 0.35],
            ray = [camera.x - origin[0], camera.y - origin[1], camera.z - origin[2]],
            distance = Math.hypot(...ray);
          return !pickGameBox(origin, ray, world, { maxDistance: Math.max(0.01, distance - 0.1) });
        });
        if (seen) hp--;
        if (hp <= 0) {
          ended = true;
          status('Defeated! Restart to try again.');
          view.stop();
          return false;
        }
        status();
      },
      render(scene, { hud }) {
        hud.set('health',{type:'meter',rect:[12,52,120,12],value:hp/3});
        scene.rect([0, 0, 640, 360], 0xffc1b1a7);
        drawGameBoxes(scene, [...world, ...targets.filter((t) => t.hp > 0)], camera);
        scene
          .line([312, 180], [328, 180], 2, 0xffffffff)
          .line([320, 172], [320, 188], 2, 0xffffffff);
        scene.rect([290, 320, 60, 40], 0xff51453d).rect([312, 300, 16, 40], 0xff71655d);
      },
    });
    status();
  },
});
`;
  throw RangeError('Unknown game template');
}
