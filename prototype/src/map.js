// 2D Grid Map & Line-of-Sight helpers
// 0 = empty, 1 = full wall (height 1.0), 2 = low cover / sandbag (height 0.48)

export const TILE_EMPTY = 0;
export const TILE_WALL = 1;
export const TILE_COVER = 2;

export const MAP_DATA = [
  [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
  [1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1],
  [1, 0, 0, 2, 2, 0, 1, 0, 2, 0, 0, 1, 1, 0, 0, 1],
  [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1],
  [1, 0, 1, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 1],
  [1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
  [1, 0, 0, 0, 2, 0, 0, 1, 1, 2, 0, 1, 0, 0, 0, 1],
  [1, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 2, 0, 1],
  [1, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 1],
  [1, 0, 2, 2, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1],
  [1, 0, 0, 0, 0, 0, 0, 2, 0, 0, 1, 0, 0, 1, 0, 1],
  [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];

export const MAP_H = MAP_DATA.length;
export const MAP_W = MAP_DATA[0].length;

export function getTile(x, y) {
  const gx = Math.floor(x);
  const gy = Math.floor(y);
  if (gx < 0 || gy < 0 || gx >= MAP_W || gy >= MAP_H) return TILE_WALL;
  return MAP_DATA[gy][gx];
}

export function isSolid(x, y) {
  return getTile(x, y) !== TILE_EMPTY;
}

// 2D DDA Raycast returning { dist, tile, hitX, hitY, side }
export function castRay(ox, oy, angle, maxDist = 24, ignoreCover = false) {
  const dx = Math.cos(angle);
  const dy = Math.sin(angle);

  let mapX = Math.floor(ox);
  let mapY = Math.floor(oy);

  const deltaDistX = dx === 0 ? 1e30 : Math.abs(1 / dx);
  const deltaDistY = dy === 0 ? 1e30 : Math.abs(1 / dy);

  let stepX, sideDistX;
  if (dx < 0) {
    stepX = -1;
    sideDistX = (ox - mapX) * deltaDistX;
  } else {
    stepX = 1;
    sideDistX = (mapX + 1.0 - ox) * deltaDistX;
  }

  let stepY, sideDistY;
  if (dy < 0) {
    stepY = -1;
    sideDistY = (oy - mapY) * deltaDistY;
  } else {
    stepY = 1;
    sideDistY = (mapY + 1.0 - oy) * deltaDistY;
  }

  let dist = 0;
  let side = 0;
  let tile = TILE_EMPTY;

  while (dist < maxDist) {
    if (sideDistX < sideDistY) {
      sideDistX += deltaDistX;
      mapX += stepX;
      side = 0;
    } else {
      sideDistY += deltaDistY;
      mapY += stepY;
      side = 1;
    }

    if (mapX < 0 || mapY < 0 || mapX >= MAP_W || mapY >= MAP_H) {
      return { dist: maxDist, tile: TILE_WALL, side, hitX: ox + dx * maxDist, hitY: oy + dy * maxDist };
    }

    tile = MAP_DATA[mapY][mapX];
    if (tile === TILE_WALL || (!ignoreCover && tile === TILE_COVER)) {
      if (side === 0) {
        dist = (mapX - ox + (1 - stepX) / 2) / dx;
      } else {
        dist = (mapY - oy + (1 - stepY) / 2) / dy;
      }
      return {
        dist: Math.max(0.01, dist),
        tile,
        side,
        hitX: ox + dx * dist,
        hitY: oy + dy * dist,
      };
    }
  }

  return { dist: maxDist, tile: TILE_EMPTY, side: 0, hitX: ox + dx * maxDist, hitY: oy + dy * maxDist };
}

// Check 2D Line of Sight between two points.
// If attacker/target is standing, low cover does NOT block full LoS (can shoot over),
// but if target is crouched behind low cover, cover blocks the shot!
export function checkLineOfSight(x1, y1, x2, y2, targetCrouched = false) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const dist = Math.hypot(dx, dy);
  if (dist < 0.05) return { clear: true, coverBetween: false };
  const angle = Math.atan2(dy, dx);

  const fullHit = castRay(x1, y1, angle, dist, true);
  if (fullHit.dist < dist - 0.1) {
    return { clear: false, coverBetween: false };
  }

  const coverHit = castRay(x1, y1, angle, dist, false);
  const hasCover = coverHit.tile === TILE_COVER && coverHit.dist < dist - 0.1;
  if (hasCover && targetCrouched) {
    return { clear: false, coverBetween: true };
  }
  return { clear: true, coverBetween: hasCover };
}
