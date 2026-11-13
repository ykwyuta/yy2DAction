// Asset Loader & 20-Frame Sprite Sheet Slicer
// Loads generated painterly assets, strips white backgrounds via border flood-fill,
// and slices unit_sheet_a.jpg + unit_sheet_b.jpg into 20 normalized 2D frames.

export const SPRITE_W = 192;
export const SPRITE_H = 288;

const FRAME_NAMES = [
  'Idle Front',
  'Idle Diag',
  'Idle Side',
  'Idle Back',
  'Walk Front A',
  'Walk Front B',
  'Walk Diag A',
  'Walk Diag B',
  'Walk Side A',
  'Walk Side B',
  'Aim Telegraph',
  'Shoot',
  'Recoil',
  'Reload',
  'Cover Crouch',
  'Alert',
  'Hit Flinch',
  'Hit Leg Kneel',
  'Down Falling',
  'Downed',
];

function loadImage(src) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error(`Failed to load ${src}`));
    img.src = src;
  });
}

// Edge-connected flood-fill to remove pure/near-white background without erasing internal highlights
function removeWhiteBackground(canvas, threshold = 232) {
  const ctx = canvas.getContext('2d');
  const w = canvas.width;
  const h = canvas.height;
  const imgData = ctx.getImageData(0, 0, w, h);
  const d = imgData.data;
  const visited = new Uint8Array(w * h);

  const isBg = (idx) => {
    const p = idx * 4;
    return d[p] >= threshold && d[p + 1] >= threshold && d[p + 2] >= threshold;
  };

  const stack = [];
  for (let x = 0; x < w; x++) {
    stack.push(x, (h - 1) * w + x);
  }
  for (let y = 1; y < h - 1; y++) {
    stack.push(y * w, y * w + (w - 1));
  }

  while (stack.length > 0) {
    const idx = stack.pop();
    if (visited[idx]) continue;
    visited[idx] = 1;
    if (!isBg(idx)) continue;

    d[idx * 4 + 3] = 0;

    const x = idx % w;
    const y = (idx - x) / w;
    if (x > 0) stack.push(idx - 1);
    if (x < w - 1) stack.push(idx + 1);
    if (y > 0) stack.push(idx - w);
    if (y < h - 1) stack.push(idx + w);
  }

  // Soft 1px edge feathering for pixels adjacent to transparent background
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      const p = idx * 4;
      if (d[p + 3] === 0) continue;
      if (d[p] > 210 && d[p + 1] > 210 && d[p + 2] > 210) {
        const hasTransparentNeighbor =
          d[(idx - 1) * 4 + 3] === 0 ||
          d[(idx + 1) * 4 + 3] === 0 ||
          d[(idx - w) * 4 + 3] === 0 ||
          d[(idx + w) * 4 + 3] === 0;
        if (hasTransparentNeighbor) {
          d[p + 3] = 96;
        }
      }
    }
  }

  ctx.putImageData(imgData, 0, 0);
  return canvas;
}

function getOpaqueBounds(canvas) {
  const ctx = canvas.getContext('2d');
  const w = canvas.width;
  const h = canvas.height;
  const d = ctx.getImageData(0, 0, w, h).data;
  let minX = w, minY = h, maxX = -1, maxY = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (d[(y * w + x) * 4 + 3] > 20) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }
  if (maxX < minX) return { x: 0, y: 0, w, h };
  return { x: minX, y: minY, w: maxX - minX + 1, h: maxY - minY + 1 };
}

function sliceSheet5x2(img, idOffset) {
  const cellW = img.width / 5;
  const cellH = img.height / 2;
  const frames = [];

  // Reference standing height from first pose in sheet or ~82% of cellH
  let refStandingH = cellH * 0.82;

  for (let row = 0; row < 2; row++) {
    for (let col = 0; col < 5; col++) {
      const idx = idOffset + row * 5 + col;
      const raw = document.createElement('canvas');
      raw.width = Math.floor(cellW);
      raw.height = Math.floor(cellH);
      const rctx = raw.getContext('2d');
      rctx.drawImage(
        img,
        col * cellW,
        row * cellH,
        cellW,
        cellH,
        0,
        0,
        raw.width,
        raw.height
      );

      removeWhiteBackground(raw, 232);
      const bounds = getOpaqueBounds(raw);

      if (row === 0 && col === 0 && idOffset === 0) {
        refStandingH = bounds.h;
      }

      // Normalize into SPRITE_W x SPRITE_H, bottom-aligned so feet rest on ground
      const out = document.createElement('canvas');
      out.width = SPRITE_W;
      out.height = SPRITE_H;
      const octx = out.getContext('2d');

      const scale = Math.min((SPRITE_H * 0.9) / refStandingH, (SPRITE_W * 0.92) / bounds.w);
      const dw = bounds.w * scale;
      const dh = bounds.h * scale;
      const dx = (SPRITE_W - dw) / 2;
      const dy = SPRITE_H - dh - 6;

      octx.drawImage(raw, bounds.x, bounds.y, bounds.w, bounds.h, dx, dy, dw, dh);
      frames.push({ id: idx, name: FRAME_NAMES[idx], canvas: out });
    }
  }

  return frames;
}

function imageToCanvas(img, maxDim = 256, stripWhite = false) {
  const c = document.createElement('canvas');
  const scale = Math.min(1, maxDim / Math.max(img.width, img.height));
  c.width = Math.round(img.width * scale);
  c.height = Math.round(img.height * scale);
  const ctx = c.getContext('2d');
  ctx.drawImage(img, 0, 0, c.width, c.height);
  if (stripWhite) {
    removeWhiteBackground(c, 232);
  }
  return c;
}

export async function loadAllAssets() {
  const [sheetA, sheetB, wallImg, coverImg, weaponImg] = await Promise.all([
    loadImage('./assets/unit_sheet_a.jpg'),
    loadImage('./assets/unit_sheet_b.jpg'),
    loadImage('./assets/wall.jpg'),
    loadImage('./assets/cover.jpg'),
    loadImage('./assets/weapon_fp.jpg'),
  ]);

  const unitSprites = [
    ...sliceSheet5x2(sheetA, 0),
    ...sliceSheet5x2(sheetB, 10),
  ];

  const wallTex = imageToCanvas(wallImg, 256, false);

  // Crop cover.jpg to opaque bounds so sandbags fill the barricade nicely
  const rawCover = imageToCanvas(coverImg, 256, true);
  const cb = getOpaqueBounds(rawCover);
  const coverTex = document.createElement('canvas');
  coverTex.width = 256;
  coverTex.height = 192;
  coverTex.getContext('2d').drawImage(rawCover, cb.x, cb.y, cb.w, cb.h, 0, 0, 256, 192);

  const weaponTex = imageToCanvas(weaponImg, 512, true);

  return { unitSprites, wallTex, coverTex, weaponTex };
}

export function drawFirstPersonWeapon(ctx, w, h, state, weaponTex) {
  const { bobX, bobY, recoil, reloading, aiming } = state;

  ctx.save();
  const scale = aiming ? 0.88 : 0.82;
  const dw = weaponTex.width * scale;
  const dh = weaponTex.height * scale;

  const baseX = aiming ? w * 0.44 : w * 0.56;
  const baseY = h - dh + (aiming ? 24 : 38) + (reloading ? 110 : 0) + bobY + recoil * 28;

  ctx.translate(baseX + bobX, baseY);
  ctx.rotate((aiming ? -0.02 : 0.0) + recoil * 0.08);
  ctx.drawImage(weaponTex, 0, 0, dw, dh);

  // Muzzle flash overlay on shot
  if (recoil > 0.65) {
    ctx.fillStyle = 'rgba(255, 215, 70, 0.85)';
    ctx.beginPath();
    ctx.arc(dw * 0.14, dh * 0.22, 30, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#ffffff';
    ctx.beginPath();
    ctx.arc(dw * 0.14, dh * 0.22, 13, 0, Math.PI * 2);
    ctx.fill();
  }

  ctx.restore();
}
