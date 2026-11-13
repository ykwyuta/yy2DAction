use crate::bone2d::{make_unit_skeleton, Skeleton2D};
use crate::map::{check_line_of_sight, is_solid};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyState {
    Idle,
    Walk,
    Alert,
    Aim,
    Shoot,
    Recoil,
    Reload,
    Falling,
    Downed,
}

#[derive(Clone, Copy, Debug)]
pub struct ScreenBounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub perp_dist: f32,
}

pub struct Enemy {
    pub x: f32,
    pub y: f32,
    pub home_x: f32,
    pub home_y: f32,
    pub angle: f32,
    pub is_patrol: bool,
    pub hp: i32,
    pub max_hp: i32,
    pub speed: f32,
    pub state: EnemyState,
    pub state_timer: f32,
    pub ammo: i32,
    pub hit_flash: f32,
    pub leg_stun_timer: f32,
    pub patrol_phase: f32,
    pub phase_offset: f32,
    pub skeleton: Skeleton2D,
    pub screen_bounds: Option<ScreenBounds>,
}

impl Enemy {
    pub fn new(x: f32, y: f32, angle: f32, is_patrol: bool, phase_offset: f32) -> Self {
        Self {
            x,
            y,
            home_x: x,
            home_y: y,
            angle,
            is_patrol,
            hp: 100,
            max_hp: 100,
            speed: 0.0,
            state: EnemyState::Idle,
            state_timer: 0.0,
            ammo: 3,
            hit_flash: 0.0,
            leg_stun_timer: 0.0,
            patrol_phase: phase_offset,
            phase_offset,
            skeleton: make_unit_skeleton(),
            screen_bounds: None,
        }
    }

    /// Select which of the 20 frames (0..19) + horizontal flip to display
    pub fn select_frame(&self, player_x: f32, player_y: f32, time: f32) -> (usize, bool, f32) {
        if self.state == EnemyState::Downed {
            return (19, false, 0.0);
        }
        if self.state == EnemyState::Falling {
            return (18, false, 0.0);
        }
        if self.leg_stun_timer > 0.0 {
            return (17, false, 0.0);
        }
        if self.hit_flash > 0.0 {
            return (16, false, 0.0);
        }
        if self.state == EnemyState::Alert {
            return (15, false, 0.0);
        }
        if self.state == EnemyState::Reload {
            return (13, false, (time * 6.0).sin() * 2.0);
        }
        if self.state == EnemyState::Recoil {
            return (12, false, 0.0);
        }
        if self.state == EnemyState::Shoot {
            return (11, false, 0.0);
        }
        if self.state == EnemyState::Aim {
            return (10, false, 0.0);
        }

        let to_player = (player_y - self.y).atan2(player_x - self.x);
        let mut rel = to_player - self.angle;
        while rel > std::f32::consts::PI {
            rel -= std::f32::consts::TAU;
        }
        while rel < -std::f32::consts::PI {
            rel += std::f32::consts::TAU;
        }

        let abs_rel = rel.abs();
        let flip = rel < 0.0;
        let walking = self.speed > 0.05;
        let walk_step = ((time * 6.0) as usize) % 2;
        let bob_y = if walking {
            (time * 6.0 * std::f32::consts::PI).sin().abs() * 6.0
        } else {
            0.0
        };

        if abs_rel < std::f32::consts::PI * 0.22 {
            (if walking { 4 + walk_step } else { 0 }, false, bob_y)
        } else if abs_rel < std::f32::consts::PI * 0.45 {
            (if walking { 6 + walk_step } else { 1 }, flip, bob_y)
        } else if abs_rel < std::f32::consts::PI * 0.75 {
            (if walking { 8 + walk_step } else { 2 }, flip, bob_y)
        } else {
            (3, if walking { walk_step == 1 } else { false }, bob_y)
        }
    }

    /// Update 2D bone hierarchy for secondary motion (breathing, recoil lean, coat sway)
    pub fn update_bones(&mut self, time: f32) {
        let t = time + self.phase_offset;
        let alive = self.hp > 0;
        let breath = if alive { (t * 2.4).sin() * 0.025 } else { 0.0 };
        let walk_tilt = if self.speed > 0.05 {
            (t * 6.0).sin() * 0.035
        } else {
            0.0
        };
        let recoil_tilt = if self.state == EnemyState::Recoil || self.hit_flash > 0.0 {
            -0.08
        } else {
            0.0
        };

        // Bone 0: Root (feet planted)
        self.skeleton.bones[0].rot = walk_tilt * 0.4;
        // Bone 1: Torso (breathing chest expansion + recoil lean)
        self.skeleton.bones[1].scale = [1.0 + breath * 0.4, 1.0 - breath];
        self.skeleton.bones[1].rot = recoil_tilt + walk_tilt * 0.6;
        // Bone 2: Head (subtle counter-balance)
        self.skeleton.bones[2].rot = -recoil_tilt * 0.5 + (t * 1.5).sin() * 0.02;
        // Bone 3: Coat hem (wind sway)
        self.skeleton.bones[3].rot = if alive { (t * 3.1).sin() * 0.05 } else { 0.0 };
        self.skeleton.update();
    }
}

pub fn move_with_collision(x: &mut f32, y: &mut f32, dx: f32, dy: f32, radius: f32) {
    let nx = *x + dx;
    if !is_solid(nx + radius, *y + radius)
        && !is_solid(nx - radius, *y + radius)
        && !is_solid(nx + radius, *y - radius)
        && !is_solid(nx - radius, *y - radius)
    {
        *x = nx;
    }
    let ny = *y + dy;
    if !is_solid(*x + radius, ny + radius)
        && !is_solid(*x - radius, ny + radius)
        && !is_solid(*x + radius, ny - radius)
        && !is_solid(*x - radius, ny - radius)
    {
        *y = ny;
    }
}

pub fn update_enemy_ai(
    e: &mut Enemy,
    player_x: f32,
    player_y: f32,
    player_crouched: bool,
    time: f32,
    dt: f32,
    out_player_damage: &mut i32,
) {
    e.hit_flash = (e.hit_flash - dt).max(0.0);
    e.leg_stun_timer = (e.leg_stun_timer - dt).max(0.0);

    if e.state == EnemyState::Downed {
        e.update_bones(time);
        return;
    }
    if e.state == EnemyState::Falling {
        e.state_timer -= dt;
        if e.state_timer <= 0.0 {
            e.state = EnemyState::Downed;
        }
        e.update_bones(time);
        return;
    }

    if e.leg_stun_timer > 0.0 {
        e.speed = 0.0;
        e.update_bones(time);
        return;
    }

    let dx = player_x - e.x;
    let dy = player_y - e.y;
    let dist = dx.hypot(dy);
    let angle_to_player = dy.atan2(dx);
    let (los_clear, _) = check_line_of_sight(e.x, e.y, player_x, player_y, player_crouched);

    let mut angle_diff = angle_to_player - e.angle;
    while angle_diff > std::f32::consts::PI {
        angle_diff -= std::f32::consts::TAU;
    }
    while angle_diff < -std::f32::consts::PI {
        angle_diff += std::f32::consts::TAU;
    }
    let can_see = los_clear && dist < 11.0 && (angle_diff.abs() < 1.2 || dist < 3.0);

    match e.state {
        EnemyState::Idle | EnemyState::Walk => {
            if e.is_patrol {
                e.patrol_phase += dt * 0.9;
                let tx = e.home_x + e.patrol_phase.cos() * 1.4;
                let ty = e.home_y + e.patrol_phase.sin() * 0.8;
                let mdx = tx - e.x;
                let mdy = ty - e.y;
                let mdist = mdx.hypot(mdy);
                if mdist > 0.05 {
                    e.angle = mdy.atan2(mdx);
                    move_with_collision(
                        &mut e.x,
                        &mut e.y,
                        (mdx / mdist) * 0.9 * dt,
                        (mdy / mdist) * 0.9 * dt,
                        0.24,
                    );
                    e.speed = 0.9;
                    e.state = EnemyState::Walk;
                }
            } else {
                e.speed = 0.0;
                e.angle += (time * 0.7 + e.home_x).sin() * 0.25 * dt;
            }
            if can_see {
                e.state = EnemyState::Alert;
                e.state_timer = 0.45;
                e.speed = 0.0;
            }
        }
        EnemyState::Alert => {
            e.angle = angle_to_player;
            e.state_timer -= dt;
            if e.state_timer <= 0.0 {
                e.state = EnemyState::Aim;
                e.state_timer = 0.75;
            }
        }
        EnemyState::Aim => {
            e.angle = angle_to_player;
            e.state_timer -= dt;
            if !los_clear {
                e.state = EnemyState::Walk;
                e.speed = 0.0;
            } else if e.state_timer <= 0.0 {
                e.state = EnemyState::Shoot;
                e.state_timer = 0.16;
                e.ammo -= 1;
                // Deterministic pseudo-random accuracy based on time/pos
                let roll = (time * 17.3 + e.x * 3.1).sin() * 0.5 + 0.5;
                let hit_chance = (0.85 - dist * 0.05).max(0.25);
                if los_clear && roll < hit_chance {
                    *out_player_damage += 12;
                }
            }
        }
        EnemyState::Shoot => {
            e.state_timer -= dt;
            if e.state_timer <= 0.0 {
                e.state = EnemyState::Recoil;
                e.state_timer = 0.25;
            }
        }
        EnemyState::Recoil => {
            e.state_timer -= dt;
            if e.state_timer <= 0.0 {
                if e.ammo <= 0 {
                    e.state = EnemyState::Reload;
                    e.state_timer = 1.5;
                    e.ammo = 3;
                } else {
                    if los_clear && dist > 3.2 {
                        move_with_collision(
                            &mut e.x,
                            &mut e.y,
                            angle_to_player.cos() * 1.1 * dt,
                            angle_to_player.sin() * 1.1 * dt,
                            0.24,
                        );
                    }
                    e.state = EnemyState::Aim;
                    e.state_timer = 0.65;
                }
            }
        }
        EnemyState::Reload => {
            e.state_timer -= dt;
            if e.state_timer <= 0.0 {
                e.state = EnemyState::Aim;
                e.state_timer = 0.55;
            }
        }
        EnemyState::Falling | EnemyState::Downed => {}
    }

    e.update_bones(time);
}
