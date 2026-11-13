import { isSolid, checkLineOfSight } from './map.js';
import { loadAllAssets } from './sprites.js';
import { renderScene } from './engine.js';

const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');

const hpEl = document.getElementById('hud-hp');
const ammoEl = document.getElementById('hud-ammo');
const stanceEl = document.getElementById('hud-stance');
const enemiesEl = document.getElementById('hud-enemies');
const statusBanner = document.getElementById('status-banner');
const restartBtn = document.getElementById('restart-btn');
const debugBtn = document.getElementById('debug-btn');

let loadedAssets = null;
let state;

function initGame() {
  state = {
    time: 0,
    showDebug: true,
    gameOver: false,
    victory: false,
    effects: [],
    popups: [],
    assets: loadedAssets,
    player: {
      x: 1.8,
      y: 1.8,
      angle: 0.35,
      hp: 100,
      maxHp: 100,
      ammo: 6,
      maxAmmo: 6,
      reserveAmmo: 24,
      crouching: false,
      crouchBlend: 0,
      aiming: false,
      recoil: 0,
      reloadTimer: 0,
      fireCooldown: 0,
      bobPhase: 0,
      bobX: 0,
      bobY: 0,
      damageFlash: 0,
    },
    enemies: [
      createEnemy(4.5, 3.5, Math.PI * 0.85, 'patrol'),
      createEnemy(7.5, 2.5, Math.PI, 'guard'),
      createEnemy(9.5, 5.5, -Math.PI * 0.6, 'patrol'),
      createEnemy(13.5, 3.5, Math.PI * 0.5, 'guard'),
      createEnemy(12.5, 8.5, Math.PI, 'patrol'),
      createEnemy(6.5, 9.5, -Math.PI * 0.25, 'guard'),
    ],
  };
  statusBanner.classList.add('hidden');
  updateHUD();
}

function createEnemy(x, y, angle, role = 'guard') {
  return {
    x,
    y,
    homeX: x,
    homeY: y,
    angle,
    role,
    hp: 100,
    maxHp: 100,
    speed: 0,
    state: 'idle', // idle | walk | alert | aim | shoot | recoil | reload | cover | falling | downed
    stateTimer: 0,
    ammo: 3,
    hitFlash: 0,
    legStunTimer: 0,
    patrolPhase: Math.random() * Math.PI * 2,
    screenBounds: null,
    lastFrameInfo: null,
  };
}

const keys = new Set();

window.addEventListener('keydown', (e) => {
  const code = e.code;
  keys.add(code);
  if (code === 'KeyR') tryReload();
  if (code === 'KeyV') {
    state.showDebug = !state.showDebug;
  }
  if (code === 'Space') {
    e.preventDefault();
    tryShoot();
  }
  if (code === 'KeyC') {
    state.player.crouching = !state.player.crouching;
  }
});

window.addEventListener('keyup', (e) => {
  keys.delete(e.code);
});

canvas.addEventListener('click', () => {
  if (document.pointerLockElement !== canvas) {
    canvas.requestPointerLock?.();
  }
});

canvas.addEventListener('mousedown', (e) => {
  if (e.button === 0) {
    tryShoot();
  } else if (e.button === 2) {
    state.player.aiming = true;
  }
});

window.addEventListener('mouseup', (e) => {
  if (e.button === 2) {
    state.player.aiming = false;
  }
});

canvas.addEventListener('contextmenu', (e) => e.preventDefault());

window.addEventListener('mousemove', (e) => {
  if (document.pointerLockElement === canvas && !state.gameOver) {
    const sens = state.player.aiming ? 0.0016 : 0.0026;
    state.player.angle += e.movementX * sens;
  }
});

restartBtn.addEventListener('click', initGame);
debugBtn.addEventListener('click', () => {
  state.showDebug = !state.showDebug;
});

function tryReload() {
  const p = state.player;
  if (state.gameOver || p.reloadTimer > 0 || p.ammo === p.maxAmmo || p.reserveAmmo <= 0) return;
  p.reloadTimer = 1.1;
}

function tryShoot() {
  const p = state.player;
  if (state.gameOver || p.reloadTimer > 0 || p.fireCooldown > 0) return;
  if (p.ammo <= 0) {
    tryReload();
    return;
  }

  p.ammo--;
  p.fireCooldown = 0.28;
  p.recoil = 1.0;

  // Hitscan against rendered enemy screenBounds around screen center (crosshair)
  const cx = canvas.width / 2;
  const cy = canvas.height / 2;

  // Sort front-to-back
  const candidates = state.enemies
    .filter((e) => e.hp > 0 && e.screenBounds)
    .sort((a, b) => a.screenBounds.perpDist - b.screenBounds.perpDist);

  for (const e of candidates) {
    const b = e.screenBounds;
    // Slight forgiveness on X when aiming
    const padX = p.aiming ? 6 : 2;
    if (cx >= b.x - padX && cx <= b.x + b.w + padX && cy >= b.y && cy <= b.y + b.h) {
      // Check actual 2D LoS (standing player shoots over low cover)
      const los = checkLineOfSight(p.x, p.y, e.x, e.y, false);
      if (!los.clear) continue;

      const relY = (cy - b.y) / b.h;
      let dmg = 34;
      let label = 'HIT';
      let color = '#f8fafc';

      if (relY < 0.24) {
        dmg = 85;
        label = 'HEADSHOT! -85';
        color = '#facc15';
      } else if (relY > 0.66) {
        dmg = 24;
        e.legStunTimer = 1.6;
        label = 'LEG HIT! (STUN)';
        color = '#38bdf8';
      } else {
        label = '-34';
      }

      e.hp = Math.max(0, e.hp - dmg);
      e.hitFlash = 0.18;

      // Knockback in 2D
      const kAngle = Math.atan2(e.y - p.y, e.x - p.x);
      const kx = e.x + Math.cos(kAngle) * 0.18;
      const ky = e.y + Math.sin(kAngle) * 0.18;
      if (!isSolid(kx, ky)) {
        e.x = kx;
        e.y = ky;
      }

      // Alert enemy if unaware
      if (e.state === 'idle' || e.state === 'walk') {
        e.state = 'alert';
        e.stateTimer = 0.35;
      }

      if (e.hp <= 0) {
        e.state = 'falling';
        e.stateTimer = 0.35;
        label = 'DOWN!';
        color = '#fb7185';
      }

      state.popups.push({ x: cx, y: cy - 20, text: label, color, life: 0.75 });
      state.effects.push({
        x: e.x,
        y: e.y,
        z: 1 - relY,
        color: relY < 0.24 ? '#facc15' : '#ef4444',
        life: 0.25,
      });
      break;
    }
  }
}

function moveWithCollision(entity, dx, dy, radius = 0.24) {
  const nx = entity.x + dx;
  if (
    !isSolid(nx + radius, entity.y + radius) &&
    !isSolid(nx - radius, entity.y + radius) &&
    !isSolid(nx + radius, entity.y - radius) &&
    !isSolid(nx - radius, entity.y - radius)
  ) {
    entity.x = nx;
  }
  const ny = entity.y + dy;
  if (
    !isSolid(entity.x + radius, ny + radius) &&
    !isSolid(entity.x - radius, ny + radius) &&
    !isSolid(entity.x + radius, ny - radius) &&
    !isSolid(entity.x - radius, ny - radius)
  ) {
    entity.y = ny;
  }
}

function updatePlayer(dt) {
  const p = state.player;
  const crouchHeld = keys.has('ShiftLeft') || keys.has('ShiftRight');
  const isCrouched = p.crouching || crouchHeld;
  p.crouchBlend += ((isCrouched ? 1 : 0) - p.crouchBlend) * Math.min(1, dt * 12);

  if (keys.has('KeyF')) p.aiming = true;
  else if (!keys.has('KeyF') && p.aiming && !document.pointerLockElement) p.aiming = false;

  // Keyboard turning
  const turnSpeed = 2.3;
  if (keys.has('ArrowLeft') || keys.has('KeyQ')) p.angle -= turnSpeed * dt;
  if (keys.has('ArrowRight') || keys.has('KeyE')) p.angle += turnSpeed * dt;

  // Movement
  let moveF = 0;
  let moveS = 0;
  if (keys.has('KeyW') || keys.has('ArrowUp')) moveF += 1;
  if (keys.has('KeyS') || keys.has('ArrowDown')) moveF -= 1;
  if (keys.has('KeyD')) moveS += 1;
  if (keys.has('KeyA')) moveS -= 1;

  const speed = (isCrouched ? 1.7 : p.aiming ? 2.0 : 3.2) * dt;
  const mag = Math.hypot(moveF, moveS);
  if (mag > 0) {
    const nf = moveF / mag;
    const ns = moveS / mag;
    const cos = Math.cos(p.angle);
    const sin = Math.sin(p.angle);
    const dx = (cos * nf - sin * ns) * speed;
    const dy = (sin * nf + cos * ns) * speed;
    moveWithCollision(p, dx, dy);

    p.bobPhase += dt * (isCrouched ? 6 : 10);
    p.bobX = Math.cos(p.bobPhase) * 6;
    p.bobY = Math.sin(p.bobPhase * 2) * 5;
  } else {
    p.bobX *= 0.85;
    p.bobY *= 0.85;
  }

  p.recoil = Math.max(0, p.recoil - dt * 5.5);
  p.fireCooldown = Math.max(0, p.fireCooldown - dt);
  p.damageFlash = Math.max(0, p.damageFlash - dt * 2.5);

  if (p.reloadTimer > 0) {
    p.reloadTimer -= dt;
    if (p.reloadTimer <= 0) {
      const needed = p.maxAmmo - p.ammo;
      const take = Math.min(needed, p.reserveAmmo);
      p.ammo += take;
      p.reserveAmmo -= take;
    }
  }
}

function updateEnemies(dt) {
  const p = state.player;
  const playerCrouched = p.crouchBlend > 0.6;
  let aliveCount = 0;

  for (const e of state.enemies) {
    e.hitFlash = Math.max(0, e.hitFlash - dt);
    e.legStunTimer = Math.max(0, e.legStunTimer - dt);

    if (e.state === 'downed') continue;
    if (e.state === 'falling') {
      e.stateTimer -= dt;
      if (e.stateTimer <= 0) e.state = 'downed';
      continue;
    }

    aliveCount++;

    if (e.legStunTimer > 0) {
      e.speed = 0;
      continue;
    }

    const dx = p.x - e.x;
    const dy = p.y - e.y;
    const dist = Math.hypot(dx, dy);
    const angleToPlayer = Math.atan2(dy, dx);
    const los = checkLineOfSight(e.x, e.y, p.x, p.y, playerCrouched);

    // Detection cone
    let angleDiff = angleToPlayer - e.angle;
    while (angleDiff > Math.PI) angleDiff -= Math.PI * 2;
    while (angleDiff < -Math.PI) angleDiff += Math.PI * 2;
    const canSee = los.clear && dist < 11 && (Math.abs(angleDiff) < 1.2 || dist < 3.0);

    if (e.state === 'idle' || e.state === 'walk') {
      if (e.role === 'patrol') {
        e.patrolPhase += dt * 0.9;
        const targetX = e.homeX + Math.cos(e.patrolPhase) * 1.4;
        const targetY = e.homeY + Math.sin(e.patrolPhase) * 0.8;
        const mdx = targetX - e.x;
        const mdy = targetY - e.y;
        const mdist = Math.hypot(mdx, mdy);
        if (mdist > 0.05) {
          e.angle = Math.atan2(mdy, mdx);
          moveWithCollision(e, (mdx / mdist) * 0.9 * dt, (mdy / mdist) * 0.9 * dt);
          e.speed = 0.9;
          e.state = 'walk';
        }
      } else {
        e.speed = 0;
        e.angle += Math.sin(state.time * 0.7 + e.homeX) * 0.25 * dt;
      }

      if (canSee) {
        e.state = 'alert';
        e.stateTimer = 0.45;
        e.speed = 0;
      }
    } else if (e.state === 'alert') {
      e.angle = angleToPlayer;
      e.stateTimer -= dt;
      if (e.stateTimer <= 0) {
        e.state = 'aim';
        e.stateTimer = 0.75; // Telegraph gives player time to duck or shoot!
      }
    } else if (e.state === 'aim') {
      e.angle = angleToPlayer;
      e.stateTimer -= dt;
      if (!los.clear) {
        // Player ducked behind low cover or broke LoS!
        e.state = 'walk';
        e.speed = 0;
      } else if (e.stateTimer <= 0) {
        e.state = 'shoot';
        e.stateTimer = 0.16;
        e.ammo--;
        // Hit check (reduced accuracy at range)
        const hitChance = Math.max(0.25, 0.88 - dist * 0.06);
        if (los.clear && Math.random() < hitChance) {
          p.hp = Math.max(0, p.hp - 12);
          p.damageFlash = 1.0;
          if (p.hp <= 0) {
            state.gameOver = true;
            showBanner('DEFEATED — Press Restart');
          }
        }
      }
    } else if (e.state === 'shoot') {
      e.stateTimer -= dt;
      if (e.stateTimer <= 0) {
        e.state = 'recoil';
        e.stateTimer = 0.25;
      }
    } else if (e.state === 'recoil') {
      e.stateTimer -= dt;
      if (e.stateTimer <= 0) {
        if (e.ammo <= 0) {
          e.state = 'reload';
          e.stateTimer = 1.5;
          e.ammo = 3;
        } else if (los.clear && dist > 3.2) {
          // Advance slightly while fighting
          moveWithCollision(e, Math.cos(angleToPlayer) * 1.1 * dt, Math.sin(angleToPlayer) * 1.1 * dt);
          e.state = 'aim';
          e.stateTimer = 0.65;
        } else {
          e.state = 'aim';
          e.stateTimer = 0.65;
        }
      }
    } else if (e.state === 'reload') {
      e.stateTimer -= dt;
      if (e.stateTimer <= 0) {
        e.state = 'aim';
        e.stateTimer = 0.55;
      }
    }
  }

  if (aliveCount === 0 && !state.victory) {
    state.victory = true;
    state.gameOver = true;
    showBanner('ALL UNITS CLEARED!');
  }
}

function showBanner(text) {
  statusBanner.querySelector('span').textContent = text;
  statusBanner.classList.remove('hidden');
  document.exitPointerLock?.();
}

function updateEffects(dt) {
  state.effects = state.effects.filter((fx) => {
    fx.life -= dt;
    return fx.life > 0;
  });
  state.popups = state.popups.filter((p) => {
    p.y -= 28 * dt;
    p.life -= dt;
    return p.life > 0;
  });
}

function updateHUD() {
  const p = state.player;
  hpEl.textContent = `HP: ${p.hp}`;
  ammoEl.textContent = p.reloadTimer > 0 ? 'RELOADING...' : `AMMO: ${p.ammo} / ${p.reserveAmmo}`;
  stanceEl.textContent = p.crouchBlend > 0.5 ? 'STANCE: CROUCH (COVER)' : 'STANCE: STAND';
  const alive = state.enemies.filter((e) => e.hp > 0).length;
  enemiesEl.textContent = `TARGETS: ${alive} / ${state.enemies.length}`;
}

let lastTime = performance.now();
function loop(now) {
  const dt = Math.min(0.05, (now - lastTime) / 1000);
  lastTime = now;

  if (!state.gameOver) {
    state.time += dt;
    updatePlayer(dt);
    updateEnemies(dt);
    updateEffects(dt);
  }

  renderScene(ctx, canvas.width, canvas.height, state);
  updateHUD();
  requestAnimationFrame(loop);
}

async function start() {
  ctx.fillStyle = '#0f172a';
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = '#e2e8f0';
  ctx.font = 'bold 18px sans-serif';
  ctx.textAlign = 'center';
  ctx.fillText('Loading 2D Painted Assets & Slicing 20-Frame Sprites...', canvas.width / 2, canvas.height / 2);

  loadedAssets = await loadAllAssets();
  initGame();
  lastTime = performance.now();
  requestAnimationFrame(loop);
}

start();
