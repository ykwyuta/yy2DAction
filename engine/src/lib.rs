pub mod assets;
pub mod bone2d;
pub mod font;
pub mod map;
pub mod renderer;
pub mod unit;

use std::collections::HashSet;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use bone2d::{make_weapon_skeleton, Skeleton2D};
use map::{check_line_of_sight, is_solid};
use renderer::{GpuRenderer, PlayerRenderState, PopupText};
use unit::{move_with_collision, update_enemy_ai, Enemy, EnemyState};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

fn now_secs() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64()
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now() / 1000.0)
            .unwrap_or(0.0)
    }
}

struct GameState {
    time: f32,
    show_debug: bool,
    game_over: bool,
    victory: bool,

    px: f32,
    py: f32,
    pangle: f32,
    hp: i32,
    ammo: i32,
    max_ammo: i32,
    reserve_ammo: i32,
    crouching: bool,
    crouch_blend: f32,
    aiming: bool,
    recoil: f32,
    reload_timer: f32,
    fire_cooldown: f32,
    bob_phase: f32,
    bob_x: f32,
    bob_y: f32,
    damage_flash: f32,

    weapon_skel: Skeleton2D,
    enemies: Vec<Enemy>,
    popups: Vec<PopupText>,
}

impl GameState {
    fn new() -> Self {
        Self {
            time: 0.0,
            show_debug: true,
            game_over: false,
            victory: false,
            px: 1.8,
            py: 1.8,
            pangle: 0.35,
            hp: 100,
            ammo: 6,
            max_ammo: 6,
            reserve_ammo: 24,
            crouching: false,
            crouch_blend: 0.0,
            aiming: false,
            recoil: 0.0,
            reload_timer: 0.0,
            fire_cooldown: 0.0,
            bob_phase: 0.0,
            bob_x: 0.0,
            bob_y: 0.0,
            damage_flash: 0.0,
            weapon_skel: make_weapon_skeleton(),
            enemies: vec![
                Enemy::new(4.5, 3.5, std::f32::consts::PI * 0.85, true, 0.0),
                Enemy::new(7.5, 2.5, std::f32::consts::PI, false, 1.1),
                Enemy::new(9.5, 5.5, -std::f32::consts::PI * 0.6, true, 2.2),
                Enemy::new(13.5, 3.5, std::f32::consts::PI * 0.5, false, 3.3),
                Enemy::new(12.5, 8.5, std::f32::consts::PI, true, 4.4),
                Enemy::new(6.5, 9.5, -std::f32::consts::PI * 0.25, false, 5.5),
            ],
            popups: Vec::new(),
        }
    }

    fn try_reload(&mut self) {
        if self.game_over
            || self.reload_timer > 0.0
            || self.ammo == self.max_ammo
            || self.reserve_ammo <= 0
        {
            return;
        }
        self.reload_timer = 1.1;
    }

    fn try_shoot(&mut self, screen_w: f32, screen_h: f32) {
        if self.game_over || self.reload_timer > 0.0 || self.fire_cooldown > 0.0 {
            return;
        }
        if self.ammo <= 0 {
            self.try_reload();
            return;
        }

        self.ammo -= 1;
        self.fire_cooldown = 0.28;
        self.recoil = 1.0;

        let cx = screen_w * 0.5;
        let cy = screen_h * 0.5;

        let mut candidates: Vec<(usize, f32)> = self
            .enemies
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if e.hp > 0 {
                    e.screen_bounds.map(|b| (i, b.perp_dist))
                } else {
                    None
                }
            })
            .collect();
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let pad_x = if self.aiming { 6.0 } else { 2.0 };
        for (idx, _) in candidates {
            let e = &mut self.enemies[idx];
            let Some(b) = e.screen_bounds else { continue };
            if cx >= b.x - pad_x && cx <= b.x + b.w + pad_x && cy >= b.y && cy <= b.y + b.h {
                let (clear, _) = check_line_of_sight(self.px, self.py, e.x, e.y, false);
                if !clear {
                    continue;
                }

                let rel_y = (cy - b.y) / b.h;
                let (dmg, mut label, mut color) = if rel_y < 0.24 {
                    (85, "HEADSHOT! -85".to_string(), [0.98, 0.80, 0.08, 1.0])
                } else if rel_y > 0.66 {
                    e.leg_stun_timer = 1.6;
                    (24, "LEG HIT! (STUN)".to_string(), [0.22, 0.74, 0.97, 1.0])
                } else {
                    (34, "-34".to_string(), [0.97, 0.98, 0.99, 1.0])
                };

                e.hp = (e.hp - dmg).max(0);
                e.hit_flash = 0.18;

                let k_ang = (e.y - self.py).atan2(e.x - self.px);
                let kx = e.x + k_ang.cos() * 0.18;
                let ky = e.y + k_ang.sin() * 0.18;
                if !is_solid(kx, ky) {
                    e.x = kx;
                    e.y = ky;
                }

                if e.state == EnemyState::Idle || e.state == EnemyState::Walk {
                    e.state = EnemyState::Alert;
                    e.state_timer = 0.35;
                }
                if e.hp <= 0 {
                    e.state = EnemyState::Falling;
                    e.state_timer = 0.35;
                    label = "DOWN!".to_string();
                    color = [0.98, 0.44, 0.52, 1.0];
                }

                self.popups.push(PopupText {
                    x: cx,
                    y: cy - 20.0,
                    text: label,
                    color,
                    life: 0.75,
                });
                break;
            }
        }
    }

    fn update(&mut self, keys: &HashSet<KeyCode>, dt: f32) {
        if self.game_over {
            return;
        }
        self.time += dt;

        let crouch_held = keys.contains(&KeyCode::ShiftLeft) || keys.contains(&KeyCode::ShiftRight);
        let is_crouched = self.crouching || crouch_held;
        let target_crouch = if is_crouched { 1.0 } else { 0.0 };
        self.crouch_blend += (target_crouch - self.crouch_blend) * (dt * 12.0).min(1.0);

        if keys.contains(&KeyCode::KeyF) {
            self.aiming = true;
        }

        let turn_speed = 2.3;
        if keys.contains(&KeyCode::ArrowLeft) || keys.contains(&KeyCode::KeyQ) {
            self.pangle -= turn_speed * dt;
        }
        if keys.contains(&KeyCode::ArrowRight) || keys.contains(&KeyCode::KeyE) {
            self.pangle += turn_speed * dt;
        }

        let mut move_f = 0.0_f32;
        let mut move_s = 0.0_f32;
        if keys.contains(&KeyCode::KeyW) || keys.contains(&KeyCode::ArrowUp) {
            move_f += 1.0;
        }
        if keys.contains(&KeyCode::KeyS) || keys.contains(&KeyCode::ArrowDown) {
            move_f -= 1.0;
        }
        if keys.contains(&KeyCode::KeyD) {
            move_s += 1.0;
        }
        if keys.contains(&KeyCode::KeyA) {
            move_s -= 1.0;
        }

        let speed = if is_crouched {
            1.7
        } else if self.aiming {
            2.0
        } else {
            3.2
        } * dt;

        let mag = move_f.hypot(move_s);
        if mag > 0.0 {
            let nf = move_f / mag;
            let ns = move_s / mag;
            let (sin, cos) = self.pangle.sin_cos();
            let dx = (cos * nf - sin * ns) * speed;
            let dy = (sin * nf + cos * ns) * speed;
            move_with_collision(&mut self.px, &mut self.py, dx, dy, 0.24);

            self.bob_phase += dt * if is_crouched { 6.0 } else { 10.0 };
            self.bob_x = self.bob_phase.cos() * 6.0;
            self.bob_y = (self.bob_phase * 2.0).sin() * 5.0;
        } else {
            self.bob_x *= 0.85;
            self.bob_y *= 0.85;
        }

        self.recoil = (self.recoil - dt * 5.5).max(0.0);
        self.fire_cooldown = (self.fire_cooldown - dt).max(0.0);
        self.damage_flash = (self.damage_flash - dt * 2.5).max(0.0);

        if self.reload_timer > 0.0 {
            self.reload_timer -= dt;
            if self.reload_timer <= 0.0 {
                let needed = self.max_ammo - self.ammo;
                let take = needed.min(self.reserve_ammo);
                self.ammo += take;
                self.reserve_ammo -= take;
            }
        }

        // Update 2-Bone Weapon Skeleton
        self.weapon_skel.bones[0].rot = (self.time * 1.8).sin() * 0.015;
        self.weapon_skel.bones[1].rot = self.recoil * 0.18;
        self.weapon_skel.bones[1].pos[0] = self.weapon_skel.bones[1].bind_pos[0] + self.recoil * 0.08;
        self.weapon_skel.update();

        let player_crouched = self.crouch_blend > 0.6;
        let mut incoming_dmg = 0;
        let mut alive_count = 0;
        for e in &mut self.enemies {
            update_enemy_ai(
                e,
                self.px,
                self.py,
                player_crouched,
                self.time,
                dt,
                &mut incoming_dmg,
            );
            if e.hp > 0 {
                alive_count += 1;
            }
        }

        if incoming_dmg > 0 {
            self.hp = (self.hp - incoming_dmg).max(0);
            self.damage_flash = 1.0;
            if self.hp <= 0 {
                self.game_over = true;
            }
        }
        if alive_count == 0 && !self.victory {
            self.victory = true;
            self.game_over = true;
        }

        self.popups.retain_mut(|p| {
            p.y -= 28.0 * dt;
            p.life -= dt;
            p.life > 0.0
        });
    }
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<GpuRenderer>,
    game: GameState,
    keys: HashSet<KeyCode>,
    last_time: f64,
    pointer_locked: bool,
    #[cfg(target_arch = "wasm32")]
    pending_renderer: Arc<std::sync::Mutex<Option<GpuRenderer>>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            game: GameState::new(),
            keys: HashSet::new(),
            last_time: now_secs(),
            pointer_locked: false,
            #[cfg(target_arch = "wasm32")]
            pending_renderer: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("2D 1st-Person Tactical Engine (Rust + wgpu + 20-Frame Units)")
            .with_inner_size(LogicalSize::new(960, 540));

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            if let Some(canvas) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("game-canvas"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            {
                attrs = attrs.with_canvas(Some(canvas));
            }
        }

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        #[cfg(target_arch = "wasm32")]
        {
            let win_clone = window.clone();
            let slot = self.pending_renderer.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let r = GpuRenderer::new(win_clone.clone()).await;
                *slot.lock().unwrap() = Some(r);
                win_clone.request_redraw();
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let r = pollster::block_on(GpuRenderer::new(window.clone()));
            self.renderer = Some(r);
        }

        self.window = Some(window);
        self.last_time = now_secs();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            self.keys.insert(code);
                            if code == KeyCode::KeyR {
                                self.game.try_reload();
                            } else if code == KeyCode::KeyV {
                                self.game.show_debug = !self.game.show_debug;
                            } else if code == KeyCode::KeyC {
                                self.game.crouching = !self.game.crouching;
                            } else if code == KeyCode::Space {
                                let (w, h) = self
                                    .renderer
                                    .as_ref()
                                    .map(|r| (r.width as f32, r.height as f32))
                                    .unwrap_or((960.0, 540.0));
                                self.game.try_shoot(w, h);
                            } else if code == KeyCode::Enter && self.game.game_over {
                                self.game = GameState::new();
                            } else if code == KeyCode::Escape {
                                self.pointer_locked = false;
                                if let Some(win) = &self.window {
                                    let _ = win.set_cursor_grab(CursorGrabMode::None);
                                    win.set_cursor_visible(true);
                                }
                            }
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                            if code == KeyCode::KeyF {
                                self.game.aiming = false;
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button == MouseButton::Left && state == ElementState::Pressed {
                    if !self.pointer_locked {
                        if let Some(win) = &self.window {
                            if win
                                .set_cursor_grab(CursorGrabMode::Locked)
                                .or_else(|_| win.set_cursor_grab(CursorGrabMode::Confined))
                                .is_ok()
                            {
                                win.set_cursor_visible(false);
                                self.pointer_locked = true;
                            }
                        }
                    }
                    let (w, h) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.width as f32, r.height as f32))
                        .unwrap_or((960.0, 540.0));
                    self.game.try_shoot(w, h);
                } else if button == MouseButton::Right {
                    self.game.aiming = state == ElementState::Pressed;
                }
            }
            WindowEvent::RedrawRequested => {
                #[cfg(target_arch = "wasm32")]
                if self.renderer.is_none() {
                    if let Some(r) = self.pending_renderer.lock().unwrap().take() {
                        self.renderer = Some(r);
                    }
                }

                let now = now_secs();
                let dt = ((now - self.last_time) as f32).clamp(0.001, 0.05);
                self.last_time = now;

                self.game.update(&self.keys, dt);

                if let Some(r) = &mut self.renderer {
                    let banner = if self.game.victory {
                        Some("ALL UNITS CLEARED!")
                    } else if self.game.game_over {
                        Some("DEFEATED")
                    } else {
                        None
                    };
                    let p_state = PlayerRenderState {
                        x: self.game.px,
                        y: self.game.py,
                        angle: self.game.pangle,
                        hp: self.game.hp,
                        ammo: self.game.ammo,
                        reserve_ammo: self.game.reserve_ammo,
                        crouch_blend: self.game.crouch_blend,
                        aiming: self.game.aiming,
                        recoil: self.game.recoil,
                        reloading: self.game.reload_timer > 0.0,
                        bob_x: self.game.bob_x,
                        bob_y: self.game.bob_y,
                        damage_flash: self.game.damage_flash,
                    };
                    r.render(
                        &p_state,
                        &mut self.game.enemies,
                        &self.game.weapon_skel,
                        &self.game.popups,
                        self.game.time,
                        self.game.show_debug,
                        banner,
                    );
                }

                if let Some(win) = &self.window {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, _) } = event {
            if self.pointer_locked && !self.game.game_over {
                let sens = if self.game.aiming { 0.0016 } else { 0.0026 };
                self.game.pangle += dx as f32 * sens;
            }
        }
    }
}

pub fn run() {
    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    run();
}
