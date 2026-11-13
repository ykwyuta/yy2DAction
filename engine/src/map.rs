pub const TILE_EMPTY: u8 = 0;
pub const TILE_WALL: u8 = 1;
pub const TILE_COVER: u8 = 2;

pub const MAP_W: usize = 16;
pub const MAP_H: usize = 12;

#[rustfmt::skip]
pub const MAP_DATA: [[u8; MAP_W]; MAP_H] = [
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

pub fn get_tile(x: f32, y: f32) -> u8 {
    let gx = x.floor() as isize;
    let gy = y.floor() as isize;
    if gx < 0 || gy < 0 || gx >= MAP_W as isize || gy >= MAP_H as isize {
        return TILE_WALL;
    }
    MAP_DATA[gy as usize][gx as usize]
}

pub fn is_solid(x: f32, y: f32) -> bool {
    get_tile(x, y) != TILE_EMPTY
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub dist: f32,
    pub tile: u8,
    pub side: u8,
    pub hit_x: f32,
    pub hit_y: f32,
}

pub fn cast_ray(ox: f32, oy: f32, angle: f32, max_dist: f32, ignore_cover: bool) -> RayHit {
    let dx = angle.cos();
    let dy = angle.sin();

    let mut map_x = ox.floor() as isize;
    let mut map_y = oy.floor() as isize;

    let delta_dist_x = if dx == 0.0 { 1e30 } else { (1.0 / dx).abs() };
    let delta_dist_y = if dy == 0.0 { 1e30 } else { (1.0 / dy).abs() };

    let (step_x, mut side_dist_x) = if dx < 0.0 {
        (-1_isize, (ox - map_x as f32) * delta_dist_x)
    } else {
        (1_isize, (map_x as f32 + 1.0 - ox) * delta_dist_x)
    };

    let (step_y, mut side_dist_y) = if dy < 0.0 {
        (-1_isize, (oy - map_y as f32) * delta_dist_y)
    } else {
        (1_isize, (map_y as f32 + 1.0 - oy) * delta_dist_y)
    };

    let mut dist = 0.0_f32;
    let mut side: u8;

    while dist < max_dist {
        if side_dist_x < side_dist_y {
            side_dist_x += delta_dist_x;
            map_x += step_x;
            side = 0;
        } else {
            side_dist_y += delta_dist_y;
            map_y += step_y;
            side = 1;
        }

        if map_x < 0 || map_y < 0 || map_x >= MAP_W as isize || map_y >= MAP_H as isize {
            return RayHit {
                dist: max_dist,
                tile: TILE_WALL,
                side,
                hit_x: ox + dx * max_dist,
                hit_y: oy + dy * max_dist,
            };
        }

        let tile = MAP_DATA[map_y as usize][map_x as usize];
        if tile == TILE_WALL || (!ignore_cover && tile == TILE_COVER) {
            dist = if side == 0 {
                (map_x as f32 - ox + (1 - step_x) as f32 * 0.5) / dx
            } else {
                (map_y as f32 - oy + (1 - step_y) as f32 * 0.5) / dy
            };
            let dist = dist.max(0.01);
            return RayHit {
                dist,
                tile,
                side,
                hit_x: ox + dx * dist,
                hit_y: oy + dy * dist,
            };
        }
    }

    RayHit {
        dist: max_dist,
        tile: TILE_EMPTY,
        side: 0,
        hit_x: ox + dx * max_dist,
        hit_y: oy + dy * max_dist,
    }
}

pub fn check_line_of_sight(x1: f32, y1: f32, x2: f32, y2: f32, target_crouched: bool) -> (bool, bool) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let dist = dx.hypot(dy);
    if dist < 0.05 {
        return (true, false);
    }
    let angle = dy.atan2(dx);

    let full_hit = cast_ray(x1, y1, angle, dist, true);
    if full_hit.dist < dist - 0.1 {
        return (false, false);
    }

    let cover_hit = cast_ray(x1, y1, angle, dist, false);
    let has_cover = cover_hit.tile == TILE_COVER && cover_hit.dist < dist - 0.1;
    if has_cover && target_crouched {
        return (false, true);
    }
    (true, has_cover)
}
