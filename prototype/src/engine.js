import { MAP_DATA, MAP_W, MAP_H, TILE_WALL, TILE_COVER, castRay } from './map.js';
import { SPRITE_W, SPRITE_H, drawFirstPersonWeapon } from './sprites.js';

// Select which of the 20 frames (0..19) + horizontal flip to display based on enemy state & relative angle
export function selectUnitFrame(enemy, player, time) {
  if (enemy.state === 'downed') {
    return { frameId: 19, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.state === 'falling') {
    return { frameId: 18, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.legStunTimer > 0) {
    return { frameId: 17, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.hitFlash > 0) {
    return { frameId: 16, flip: false, bobY: 0, tilt: -0.06 };
  }
  if (enemy.state === 'alert') {
    return { frameId: 15, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.state === 'cover') {
    return { frameId: 14, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.state === 'reload') {
    return { frameId: 13, flip: false, bobY: Math.sin(time * 6) * 2, tilt: 0 };
  }
  if (enemy.state === 'recoil') {
    return { frameId: 12, flip: false, bobY: 0, tilt: -0.05 };
  }
  if (enemy.state === 'shoot') {
    return { frameId: 11, flip: false, bobY: 0, tilt: 0 };
  }
  if (enemy.state === 'aim') {
    return { frameId: 10, flip: false, bobY: 0, tilt: 0 };
  }

  // Compute relative angle between enemy's facing and vector from enemy to player
  const toPlayerAngle = Math.atan2(player.y - enemy.y, player.x - enemy.x);
  let rel = toPlayerAngle - enemy.angle;
  while (rel > Math.PI) rel -= Math.PI * 2;
  while (rel < -Math.PI) rel += Math.PI * 2;

  const absRel = Math.abs(rel);
  const flip = rel < 0;
  const walking = enemy.speed > 0.05;
  const walkStep = Math.floor(time * 6) % 2; // 0 or 1
  const bobY = walking ? Math.abs(Math.sin(time * 6 * Math.PI)) * 6 : Math.sin(time * 2.5) * 1.5;
  const tilt = walking ? Math.sin(time * 6 * Math.PI) * 0.04 : 0;

  if (absRel < Math.PI * 0.22) {
    return { frameId: walking ? 4 + walkStep : 0, flip: false, bobY, tilt };
  } else if (absRel < Math.PI * 0.45) {
    return { frameId: walking ? 6 + walkStep : 1, flip, bobY, tilt };
  } else if (absRel < Math.PI * 0.75) {
    return { frameId: walking ? 8 + walkStep : 2, flip, bobY, tilt };
  } else {
    return {
      frameId: 3,
      flip: walking ? walkStep === 1 : false,
      bobY,
      tilt,
    };
  }
}

export function renderScene(ctx, w, h, state) {
  const { player, enemies, assets, effects, popups, time, showDebug } = state;
  const { unitSprites, wallTex, coverTex, weaponTex } = assets;

  const baseFov = player.aiming ? Math.PI / 4.2 : Math.PI / 2.8;
  const focalDist = (w / 2) / Math.tan(baseFov / 2);

  // Horizon shifts up when crouched (looking up slightly from cover) + subtle walk bob
  const crouchOffset = player.crouchBlend * (h * 0.14);
  const horizon = h / 2 + crouchOffset + player.bobY * 0.5;

  // 1. Watercolor Sky & Distant Sketchy Mountains (2D Parallax)
  const skyGrad = ctx.createLinearGradient(0, 0, 0, horizon);
  skyGrad.addColorStop(0, '#9eb3c2');
  skyGrad.addColorStop(0.55, '#dce2e6');
  skyGrad.addColorStop(1, '#f4efe6');
  ctx.fillStyle = skyGrad;
  ctx.fillRect(0, 0, w, horizon);

  // Parallax mountain silhouette
  ctx.fillStyle = '#8fa399';
  ctx.beginPath();
  ctx.moveTo(0, horizon);
  const angleOffset = (player.angle / (Math.PI * 2)) * w * 2;
  for (let x = 0; x <= w; x += 24) {
    const wx = x + angleOffset;
    const mh = Math.sin(wx * 0.007) * 28 + Math.cos(wx * 0.017) * 15 + 38;
    ctx.lineTo(x, horizon - mh);
  }
  ctx.lineTo(w, horizon);
  ctx.closePath();
  ctx.fill();

  // 2. Ground
  const floorGrad = ctx.createLinearGradient(0, horizon, 0, h);
  floorGrad.addColorStop(0, '#8d9676');
  floorGrad.addColorStop(0.35, '#687058');
  floorGrad.addColorStop(1, '#393d32');
  ctx.fillStyle = floorGrad;
  ctx.fillRect(0, horizon, w, h - horizon);

  // 3. Textured Raycast Walls & Low Cover (Column-by-column 2.5D projection)
  const colWidth = 2;
  const numCols = Math.ceil(w / colWidth);
  const zBuffer = new Float32Array(numCols);
  const coverZBuffer = new Float32Array(numCols);

  const eyeHeightFactor = 0.5 - player.crouchBlend * 0.24;

  for (let i = 0; i < numCols; i++) {
    const screenX = i * colWidth + colWidth / 2;
    const rayAngle = player.angle + Math.atan2(screenX - w / 2, focalDist);
    const cosFix = Math.cos(rayAngle - player.angle);

    // Cast ignoring low cover to get the full wall behind
    const wallHit = castRay(player.x, player.y, rayAngle, 26, true);
    const wallDist = wallHit.dist * cosFix;
    zBuffer[i] = wallDist;

    const wallH = (1.0 / Math.max(0.05, wallDist)) * focalDist;
    const wallBottom = horizon + wallH * eyeHeightFactor;
    const wallTop = wallBottom - wallH;

    // Texture U coordinate
    let wallU = wallHit.side === 0 ? wallHit.hitY : wallHit.hitX;
    wallU = wallU - Math.floor(wallU);
    const texX = Math.max(0, Math.min(wallTex.width - 2, Math.floor(wallU * wallTex.width)));

    ctx.drawImage(wallTex, texX, 0, 2, wallTex.height, i * colWidth, wallTop, colWidth, wallH);

    // Distance fog + directional side shading
    const fogAlpha = Math.min(0.65, wallDist / 22) + (wallHit.side === 1 ? 0.14 : 0);
    if (fogAlpha > 0.01) {
      ctx.fillStyle = `rgba(30, 36, 48, ${Math.min(0.75, fogAlpha)})`;
      ctx.fillRect(i * colWidth, wallTop, colWidth, wallH);
    }

    // Check low cover (sandbag/crate) in front of the wall
    const coverHit = castRay(player.x, player.y, rayAngle, wallHit.dist, false);
    if (coverHit.tile === TILE_COVER && coverHit.dist < wallHit.dist) {
      const cDist = coverHit.dist * cosFix;
      coverZBuffer[i] = cDist;
      const fullH = (1.0 / Math.max(0.05, cDist)) * focalDist;
      const cBottom = horizon + fullH * eyeHeightFactor;
      const cH = fullH * 0.48;
      const cTop = cBottom - cH;

      let coverU = coverHit.side === 0 ? coverHit.hitY : coverHit.hitX;
      coverU = coverU - Math.floor(coverU);
      const cTexX = Math.max(0, Math.min(coverTex.width - 2, Math.floor(coverU * coverTex.width)));

      ctx.drawImage(coverTex, cTexX, 0, 2, coverTex.height, i * colWidth, cTop, colWidth, cH);

      const cFog = Math.min(0.55, cDist / 20) + (coverHit.side === 1 ? 0.15 : 0);
      if (cFog > 0.01) {
        ctx.fillStyle = `rgba(28, 25, 23, ${Math.min(0.7, cFog)})`;
        ctx.fillRect(i * colWidth, cTop, colWidth, cH);
      }
    } else {
      coverZBuffer[i] = wallDist;
    }
  }

  // 4. Sort & Draw Billboard Units (Back to Front)
  const renderList = enemies
    .map((e) => {
      const dx = e.x - player.x;
      const dy = e.y - player.y;
      const dist = Math.hypot(dx, dy);
      const angleTo = Math.atan2(dy, dx) - player.angle;
      let relAngle = angleTo;
      while (relAngle > Math.PI) relAngle -= Math.PI * 2;
      while (relAngle < -Math.PI) relAngle += Math.PI * 2;
      const perpDist = dist * Math.cos(relAngle);
      return { enemy: e, dist, perpDist, relAngle };
    })
    .filter((item) => item.perpDist > 0.25 && Math.abs(item.relAngle) < baseFov * 0.85)
    .sort((a, b) => b.perpDist - a.perpDist);

  for (const e of enemies) {
    e.screenBounds = null;
  }

  for (const item of renderList) {
    const { enemy, perpDist, relAngle } = item;
    const screenX = w / 2 + Math.tan(relAngle) * focalDist;
    const fullH = (1.0 / perpDist) * focalDist;
    const spriteH = fullH * 0.96;
    const spriteW = spriteH * (SPRITE_W / SPRITE_H);

    const groundY = horizon + fullH * eyeHeightFactor;
    const frameInfo = selectUnitFrame(enemy, player, time);
    enemy.lastFrameInfo = frameInfo;

    const drawY = groundY - spriteH - (frameInfo.bobY / SPRITE_H) * spriteH;
    const drawX = screenX - spriteW / 2;

    enemy.screenBounds = {
      x: drawX,
      y: drawY,
      w: spriteW,
      h: spriteH,
      perpDist,
    };

    // Contact shadow on ground
    const centerCol = Math.floor(screenX / colWidth);
    if (centerCol >= 0 && centerCol < numCols && perpDist < zBuffer[centerCol]) {
      ctx.fillStyle = 'rgba(20, 24, 30, 0.38)';
      ctx.beginPath();
      ctx.ellipse(screenX, groundY - 2, spriteW * 0.28, Math.max(3, spriteH * 0.04), 0, 0, Math.PI * 2);
      ctx.fill();
    }

    // Draw sprite column-by-column clipped against Z-buffer
    const spriteCanvas = unitSprites[frameInfo.frameId].canvas;
    const startCol = Math.max(0, Math.floor(drawX / colWidth));
    const endCol = Math.min(numCols - 1, Math.floor((drawX + spriteW) / colWidth));

    for (let c = startCol; c <= endCol; c++) {
      const colScreenX = c * colWidth;
      const wallBehind = zBuffer[c];
      const coverInFront = coverZBuffer[c];

      if (perpDist >= wallBehind) continue;

      let maxVisibleY = h;
      if (perpDist > coverInFront && coverInFront < wallBehind) {
        const cFullH = (1.0 / coverInFront) * focalDist;
        const cBottom = horizon + cFullH * eyeHeightFactor;
        const cTop = cBottom - cFullH * 0.48;
        maxVisibleY = cTop;
      }

      const u = (colScreenX - drawX) / spriteW;
      if (u < 0 || u >= 1) continue;
      const srcU = frameInfo.flip ? 1 - u : u;
      const sx = Math.max(0, Math.min(SPRITE_W - 2, Math.floor(srcU * SPRITE_W)));

      const visibleH = Math.min(spriteH, maxVisibleY - drawY);
      if (visibleH <= 0) continue;
      const srcH = (visibleH / spriteH) * SPRITE_H;

      ctx.drawImage(spriteCanvas, sx, 0, 2, srcH, colScreenX, drawY, colWidth, visibleH);
    }

    // Enemy muzzle flash effect
    if (enemy.state === 'shoot' && enemy.stateTimer > 0.05) {
      ctx.fillStyle = 'rgba(255, 220, 80, 0.85)';
      ctx.beginPath();
      ctx.arc(screenX, drawY + spriteH * 0.38, spriteW * 0.14, 0, Math.PI * 2);
      ctx.fill();
    }

    // Enemy HP bar
    if (enemy.hp > 0 && enemy.hp < enemy.maxHp && centerCol >= 0 && centerCol < numCols && perpDist < zBuffer[centerCol]) {
      const bw = Math.max(30, spriteW * 0.55);
      const bx = screenX - bw / 2;
      const by = drawY - 12;
      ctx.fillStyle = 'rgba(0,0,0,0.65)';
      ctx.fillRect(bx, by, bw, 5);
      ctx.fillStyle = '#e63946';
      ctx.fillRect(bx, by, bw * (enemy.hp / enemy.maxHp), 5);
    }

    // Debug overlay: show which of the 20 frames is active
    if (showDebug && enemy.screenBounds) {
      const fName = unitSprites[frameInfo.frameId].name;
      const label = `#${frameInfo.frameId} ${fName}${frameInfo.flip ? ' (Flip)' : ''}`;
      ctx.font = 'bold 11px monospace';
      const tw = ctx.measureText(label).width + 8;
      ctx.fillStyle = 'rgba(15, 23, 42, 0.85)';
      ctx.fillRect(screenX - tw / 2, drawY - 28, tw, 16);
      ctx.fillStyle = '#38bdf8';
      ctx.textAlign = 'center';
      ctx.fillText(label, screenX, drawY - 16);
    }
  }

  // 5. Hit sparks / effects
  for (const fx of effects) {
    const dx = fx.x - player.x;
    const dy = fx.y - player.y;
    const angleTo = Math.atan2(dy, dx) - player.angle;
    let rel = angleTo;
    while (rel > Math.PI) rel -= Math.PI * 2;
    while (rel < -Math.PI) rel += Math.PI * 2;
    const perpDist = Math.hypot(dx, dy) * Math.cos(rel);
    if (perpDist > 0.2 && Math.abs(rel) < baseFov) {
      const sx = w / 2 + Math.tan(rel) * focalDist;
      const sy = horizon + (0.5 - fx.z + (eyeHeightFactor - 0.5)) * (1 / perpDist) * focalDist;
      const r = (1 / perpDist) * 28 * fx.life;
      ctx.fillStyle = fx.color;
      ctx.beginPath();
      ctx.arc(sx, sy, Math.max(2, r), 0, Math.PI * 2);
      ctx.fill();
    }
  }

  // 6. 1st-Person Weapon Sprite
  drawFirstPersonWeapon(
    ctx,
    w,
    h,
    {
      bobX: player.bobX,
      bobY: Math.abs(player.bobY),
      recoil: player.recoil,
      reloading: player.reloadTimer > 0,
      aiming: player.aiming,
    },
    weaponTex
  );

  // 7. Crosshair
  const cx = w / 2;
  const cy = h / 2;
  const spread = player.aiming ? 6 : 14 + player.recoil * 16;
  ctx.strokeStyle = player.aiming ? '#ffbe0b' : 'rgba(255,255,255,0.9)';
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.moveTo(cx - spread - 6, cy);
  ctx.lineTo(cx - spread, cy);
  ctx.moveTo(cx + spread, cy);
  ctx.lineTo(cx + spread + 6, cy);
  ctx.moveTo(cx, cy - spread - 6);
  ctx.lineTo(cx, cy - spread);
  ctx.moveTo(cx, cy + spread);
  ctx.lineTo(cx, cy + spread + 6);
  ctx.stroke();

  // 8. Floating Hit Popups
  for (const p of popups) {
    ctx.save();
    ctx.globalAlpha = Math.min(1, p.life * 2);
    ctx.font = 'bold 16px sans-serif';
    ctx.textAlign = 'center';
    ctx.strokeStyle = '#000';
    ctx.lineWidth = 3;
    ctx.strokeText(p.text, p.x, p.y);
    ctx.fillStyle = p.color;
    ctx.fillText(p.text, p.x, p.y);
    ctx.restore();
  }

  // 9. Damage vignette
  if (player.damageFlash > 0) {
    ctx.fillStyle = `rgba(220, 38, 38, ${player.damageFlash * 0.42})`;
    ctx.fillRect(0, 0, w, h);
  }

  // 10. 2D Minimap
  drawMinimap(ctx, player, enemies);

  // 11. 20-Frame Sprite Strip at bottom when Debug (V) is active
  if (showDebug) {
    drawSpriteStrip(ctx, w, h, unitSprites);
  }
}

function drawMinimap(ctx, player, enemies) {
  const tile = 9;
  const ox = 14;
  const oy = 14;
  ctx.save();
  ctx.fillStyle = 'rgba(15, 23, 42, 0.72)';
  ctx.fillRect(ox - 4, oy - 4, MAP_W * tile + 8, MAP_H * tile + 8);

  for (let y = 0; y < MAP_H; y++) {
    for (let x = 0; x < MAP_W; x++) {
      const t = MAP_DATA[y][x];
      if (t === TILE_WALL) {
        ctx.fillStyle = '#cbd5e1';
        ctx.fillRect(ox + x * tile, oy + y * tile, tile - 1, tile - 1);
      } else if (t === TILE_COVER) {
        ctx.fillStyle = '#d97706';
        ctx.fillRect(ox + x * tile + 1, oy + y * tile + 1, tile - 3, tile - 3);
      }
    }
  }

  for (const e of enemies) {
    if (e.hp <= 0) continue;
    ctx.fillStyle = e.state === 'aim' || e.state === 'shoot' ? '#ef4444' : '#38bdf8';
    ctx.beginPath();
    ctx.arc(ox + e.x * tile, oy + e.y * tile, 3, 0, Math.PI * 2);
    ctx.fill();
  }

  const px = ox + player.x * tile;
  const py = oy + player.y * tile;
  ctx.strokeStyle = 'rgba(250, 204, 21, 0.7)';
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.moveTo(px, py);
  ctx.lineTo(px + Math.cos(player.angle) * 16, py + Math.sin(player.angle) * 16);
  ctx.stroke();

  ctx.fillStyle = '#facc15';
  ctx.beginPath();
  ctx.arc(px, py, 3.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function drawSpriteStrip(ctx, w, h, unitSprites) {
  const thumbW = 34;
  const thumbH = 51;
  const totalW = unitSprites.length * (thumbW + 3);
  const startX = Math.max(10, (w - totalW) / 2);
  const y = h - thumbH - 10;

  ctx.save();
  ctx.fillStyle = 'rgba(15, 23, 42, 0.85)';
  ctx.fillRect(startX - 6, y - 20, totalW + 12, thumbH + 26);
  ctx.fillStyle = '#e2e8f0';
  ctx.font = 'bold 11px monospace';
  ctx.textAlign = 'left';
  ctx.fillText('20-Frame Unit Sprite Sheet (1 Unit = 20 Images):', startX, y - 6);

  unitSprites.forEach((s, i) => {
    const x = startX + i * (thumbW + 3);
    ctx.fillStyle = 'rgba(255,255,255,0.12)';
    ctx.fillRect(x, y, thumbW, thumbH);
    ctx.drawImage(s.canvas, x, y, thumbW, thumbH);
    ctx.fillStyle = '#94a3b8';
    ctx.font = '9px monospace';
    ctx.fillText(String(i), x + 2, y + 10);
  });
  ctx.restore();
}
