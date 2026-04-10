use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};

const GRAVITY: f64 = 25.0;

#[derive(PartialEq, Clone, Copy)]
enum GameMode {
    Sandbox,
    Juggler,
    TargetZones,
    HeartGolf,
}

const BOUNCE_DAMPING: f64 = 0.6;
const FRICTION: f64 = 0.995;
const DEFAULT_HEART_SIZE: f64 = 24.0;

struct Physics {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot_h: f64,      // horizontal rotation (around vertical axis)
    rot_v: f64,      // vertical rotation (around horizontal axis)
    rot_h_vel: f64,
    rot_v_vel: f64,
}


fn ray_hits_heart(hx_screen: f64, hz_screen: f64, sin_h: f64, cos_h: f64, sin_v: f64, cos_v: f64) -> bool {
    let cam_dist = 4.0;
    let ox = hx_screen * cos_h - cam_dist * sin_h;
    let oy = -hx_screen * sin_h - cam_dist * cos_h;
    let oz = hz_screen * cos_v;
    let oy2 = oy * cos_v + hz_screen * sin_v;
    let oz2 = -oy * sin_v + oz;

    let dx = sin_h;
    let dy = cos_h * cos_v;
    let dz = -cos_h * sin_v;

    let mut t = 0.0_f64;
    let max_t = 4.0;
    let step = 0.15;
    let mut prev_val = heart_implicit(ox, oy2, oz2);

    while t < max_t {
        t += step;
        let val = heart_implicit(ox + dx * t, oy2 + dy * t, oz2 + dz * t);
        if prev_val * val < 0.0 { return true; }
        prev_val = val;
    }
    false
}

fn find_extent(dir_x: f64, dir_z: f64, sin_h: f64, cos_h: f64, sin_v: f64, cos_v: f64) -> f64 {
    let mut lo = 0.0_f64;
    let mut hi = 2.0_f64;

    for _ in 0..6 {
        let mid = (lo + hi) * 0.5;
        if ray_hits_heart(dir_x * mid, dir_z * mid, sin_h, cos_h, sin_v, cos_v) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn compute_rotated_extents(rot_h: f64, rot_v: f64, heart_scale: f64) -> (f64, f64, f64, f64) {
    let (sin_h, cos_h) = rot_h.sin_cos();
    let (sin_v, cos_v) = rot_v.sin_cos();

    let mut max_right = 0.0_f64;
    let mut max_left = 0.0_f64;
    let mut max_up = 0.0_f64;
    let mut max_down = 0.0_f64;

    for i in 0..8 {
        let angle = (i as f64 / 8.0) * std::f64::consts::TAU;
        let (s, c) = angle.sin_cos();
        let extent = find_extent(c, s, sin_h, cos_h, sin_v, cos_v);
        let ext_x = c * extent;
        let ext_z = s * extent;

        if ext_x > 0.0 { max_right = max_right.max(ext_x); }
        if ext_x < 0.0 { max_left = max_left.max(-ext_x); }
        if ext_z > 0.0 { max_up = max_up.max(ext_z); }
        if ext_z < 0.0 { max_down = max_down.max(-ext_z); }
    }

    max_right = max_right.max(find_extent(1.0, 0.0, sin_h, cos_h, sin_v, cos_v));
    max_left = max_left.max(find_extent(-1.0, 0.0, sin_h, cos_h, sin_v, cos_v));
    max_up = max_up.max(find_extent(0.0, 1.0, sin_h, cos_h, sin_v, cos_v));
    max_down = max_down.max(find_extent(0.0, -1.0, sin_h, cos_h, sin_v, cos_v));

    (
        max_left * heart_scale,
        max_right * heart_scale,
        max_up * heart_scale * 0.5,
        max_down * heart_scale * 0.5,
    )
}

fn is_point_in_heart(px: f64, py: f64, heart_x: f64, heart_y: f64, heart_size: f64) -> bool {
    let scale = heart_size / 32.0;
    let local_x = (px - heart_x) / scale;
    let local_y = (py - heart_y) / (scale * 0.5);
    let nx = local_x / 16.0;
    let ny = local_y / 17.0;
    let x2 = nx * nx;
    let term = x2 + ny * ny - 1.0;
    term * term * term - x2 * ny * ny * ny < 0.0
}

fn heart_implicit(x: f64, y: f64, z: f64) -> f64 {
    let x2 = x * x;
    let y2 = y * y;
    let z2 = z * z;
    let z3 = z2 * z;

    let term = x2 + 2.25 * y2 + z2 - 1.0;
    let base = term * term * term - x2 * z3 - 0.045 * y2 * z3;

    let cleft_depth = 0.35;
    let cleft_width = 0.02;  // Very narrow for V-shape
    let z_pos = z.max(0.0);
    let dist2 = x2 + y2;
    let sharpness = 1.0 / (1.0 + dist2 / cleft_width).powi(3);
    let cleft = cleft_depth * sharpness * z_pos * z_pos;

    base + cleft
}

fn heart_gradient(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let eps = 0.015;  // Fixed epsilon for speed

    let gx = heart_implicit(x + eps, y, z) - heart_implicit(x - eps, y, z);
    let gy = heart_implicit(x, y + eps, z) - heart_implicit(x, y - eps, z);
    let gz = heart_implicit(x, y, z + eps) - heart_implicit(x, y, z - eps);

    let len = (gx * gx + gy * gy + gz * gz).sqrt();

    if len < 0.0001 {
        let rad = (x * x + y * y).sqrt().max(0.001);
        (x / rad, y / rad, 0.0)
    } else {
        (gx / len, gy / len, gz / len)
    }
}

fn ray_march_heart(ox: f64, oy: f64, oz: f64, dx: f64, dy: f64, dz: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let mut t = 0.0_f64;
    let max_t = 4.0;
    let step = 0.08;  // Larger steps for speed

    let mut prev_val = heart_implicit(ox, oy, oz);

    while t < max_t {
        t += step;
        let x = ox + dx * t;
        let y = oy + dy * t;
        let z = oz + dz * t;

        let val = heart_implicit(x, y, z);

        if prev_val * val < 0.0 {
            let mut t0 = t - step;
            let mut t1 = t;
            let mut pv = prev_val;
            for _ in 0..3 {
                let tm = (t0 + t1) * 0.5;
                let vm = heart_implicit(ox + dx * tm, oy + dy * tm, oz + dz * tm);
                if pv * vm < 0.0 { t1 = tm; } else { t0 = tm; pv = vm; }
            }

            let tf = (t0 + t1) * 0.5;
            let hx = ox + dx * tf;
            let hy = oy + dy * tf;
            let hz = oz + dz * tf;
            let (nx, ny, nz) = heart_gradient(hx, hy, hz);
            return Some((hx, hy, hz, nx, ny, nz));
        }
        prev_val = val;
    }
    None
}

fn main() -> std::io::Result<()> {
    let mut stdout = stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Hide)?;
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    print!("\x1b[?7l");
    stdout.flush()?;

    let (mut width, mut height) = terminal::size()?;

    let mut hearts: Vec<Physics> = vec![Physics {
        x: width as f64 / 2.0,
        y: height as f64 / 2.0,
        vx: 0.0,
        vy: 0.0,
        rot_h: 0.0,
        rot_v: 0.0,
        rot_h_vel: 0.0,
        rot_v_vel: 0.0,
    }];

    let mut prev_mouse_x: f64 = 0.0;
    let mut prev_mouse_y: f64 = 0.0;
    let mut mouse_initialized = false;
    let mut blackhole_mode = false;
    let mut whitehole_mode = false;
    let mut gravity_dir: u8 = 0;  // 0=down, 1=left, 2=up, 3=right
    let mut heart_size = DEFAULT_HEART_SIZE;
    let mut last_frame = Instant::now();

    let mut game_mode = GameMode::Sandbox;
    let mut score: u32 = 0;
    let mut game_time: f64 = 0.0;
    let mut last_spawn = Instant::now();
    let mut golf_touches: u32 = 0;
    let mut golf_hole: u32 = 0;

    let mut target: Option<(f64, f64, f64)> = None;
    let mut targets_hit: u32 = 0;
    let mut golf_touching: bool = false;  // Track if currently touching heart
    let mut animation_time: f64 = 0.0;  // For 3D target animation

    let luminance_chars: &[u8] = b".,-~:;=!*#$@";

    'main: loop {
        let now = Instant::now();
        let dt = now.duration_since(last_frame).as_secs_f64();
        last_frame = now;
        animation_time += dt;

        while event::poll(Duration::from_millis(0))? {
            match event::read()? {
                Event::Key(KeyEvent { code: KeyCode::Esc, .. }) => break 'main,
                Event::Key(KeyEvent { code: KeyCode::Char('q'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    if game_mode == GameMode::Sandbox {
                        hearts.push(Physics {
                            x: prev_mouse_x,
                            y: prev_mouse_y,
                            vx: (rand_f64() - 0.5) * 20.0,
                            vy: (rand_f64() - 0.5) * 20.0,
                            rot_h: rand_f64() * std::f64::consts::TAU,
                            rot_v: rand_f64() * std::f64::consts::TAU,
                            rot_h_vel: (rand_f64() - 0.5) * 2.0,
                            rot_v_vel: (rand_f64() - 0.5) * 2.0,
                        });
                    }
                },
                Event::Key(KeyEvent { code: KeyCode::Up, .. }) => { for h in &mut hearts { h.vy -= 80.0; } },
                Event::Key(KeyEvent { code: KeyCode::Down, .. }) => { for h in &mut hearts { h.vy += 80.0; } },
                Event::Key(KeyEvent { code: KeyCode::Left, .. }) => { for h in &mut hearts { h.vx -= 80.0; } },
                Event::Key(KeyEvent { code: KeyCode::Right, .. }) => { for h in &mut hearts { h.vx += 80.0; } },
                Event::Key(KeyEvent { code: KeyCode::Char('w'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    if game_mode == GameMode::Sandbox {
                        blackhole_mode = !blackhole_mode;
                        whitehole_mode = false;
                    }
                },
                Event::Key(KeyEvent { code: KeyCode::Char('e'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    if game_mode == GameMode::Sandbox || game_mode == GameMode::TargetZones {
                        whitehole_mode = !whitehole_mode;
                        blackhole_mode = false;
                    }
                },
                Event::Key(KeyEvent { code: KeyCode::Char('r'), kind: crossterm::event::KeyEventKind::Press, .. }) => { gravity_dir = (gravity_dir + 1) % 4; },
                Event::Key(KeyEvent { code: KeyCode::Char('0'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    game_mode = GameMode::Sandbox;
                    blackhole_mode = false;
                    whitehole_mode = false;
                },
                Event::Key(KeyEvent { code: KeyCode::Char('1'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    game_mode = GameMode::Juggler;
                    blackhole_mode = false;
                    whitehole_mode = false;
                    score = 0;
                    game_time = 0.0;
                    hearts.clear();
                    hearts.push(Physics {
                        x: width as f64 / 2.0,
                        y: height as f64 / 3.0,
                        vx: 0.0, vy: 0.0,
                        rot_h: 0.0, rot_v: 0.0,
                        rot_h_vel: 0.0, rot_v_vel: 0.0,
                    });
                    last_spawn = Instant::now();
                    gravity_dir = 0;  // Reset gravity to down
                },
                Event::Key(KeyEvent { code: KeyCode::Char('2'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    game_mode = GameMode::TargetZones;
                    blackhole_mode = false;
                    whitehole_mode = false;
                    targets_hit = 0;
                    target = None;
                    hearts.clear();
                    hearts.push(Physics {
                        x: width as f64 / 2.0,
                        y: height as f64 / 2.0,
                        vx: 0.0, vy: 0.0,
                        rot_h: 0.0, rot_v: 0.0,
                        rot_h_vel: 0.0, rot_v_vel: 0.0,
                    });
                    gravity_dir = 0;
                },
                Event::Key(KeyEvent { code: KeyCode::Char('3'), kind: crossterm::event::KeyEventKind::Press, .. }) => {
                    game_mode = GameMode::HeartGolf;
                    blackhole_mode = false;
                    whitehole_mode = false;
                    golf_touches = 0;
                    golf_hole = 1;
                    score = 0;
                    target = None;
                    hearts.clear();
                    hearts.push(Physics {
                        x: width as f64 / 4.0,
                        y: height as f64 / 2.0,
                        vx: 0.0, vy: 0.0,
                        rot_h: 0.0, rot_v: 0.0,
                        rot_h_vel: 0.0, rot_v_vel: 0.0,
                    });
                    gravity_dir = 0;
                },
                Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollUp, .. }) => { heart_size = (heart_size + 2.0).min(60.0); },
                Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, .. }) => { heart_size = (heart_size - 2.0).max(8.0); },
                Event::Mouse(MouseEvent { kind: MouseEventKind::Moved, column, row, .. }) |
                Event::Mouse(MouseEvent { kind: MouseEventKind::Drag(_), column, row, .. }) => {
                    let mx = column as f64;
                    let my = row as f64;

                    if mouse_initialized {
                        let dx = mx - prev_mouse_x;
                        let dy = my - prev_mouse_y;

                        if !blackhole_mode && !whitehole_mode {
                            let speed = (dx * dx + dy * dy).sqrt();
                            let mut any_hit = false;
                            for heart in &mut hearts {
                                let mut hit = is_point_in_heart(mx, my, heart.x, heart.y, heart_size);

                                if !hit && speed > 1.0 {
                                    let steps = (speed * 2.0) as i32;
                                    for s in 1..=steps {
                                        let t = s as f64 / steps as f64;
                                        let check_x = prev_mouse_x + dx * t;
                                        let check_y = prev_mouse_y + dy * t;
                                        if is_point_in_heart(check_x, check_y, heart.x, heart.y, heart_size) {
                                            hit = true;
                                            break;
                                        }
                                    }
                                }

                                if hit {
                                    any_hit = true;
                                    heart.vx += dx * 12.0;
                                    heart.vy += dy * 12.0;
                                    heart.rot_h_vel += dx * 0.5;
                                    heart.rot_v_vel += dy * 0.5;
                                }
                            }
                            if game_mode == GameMode::HeartGolf {
                                if any_hit && !golf_touching {
                                    golf_touches += 1;
                                }
                                golf_touching = any_hit;
                            }
                        }
                    }
                    prev_mouse_x = mx;
                    prev_mouse_y = my;
                    mouse_initialized = true;
                }
                Event::Resize(w, h) => {
                    width = w;
                    height = h;
                    if game_mode == GameMode::TargetZones || game_mode == GameMode::HeartGolf {
                        target = None;
                    }
                }
                _ => {}
            }
        }

        let heart_scale = heart_size * 0.55;
        for physics in &mut hearts {
            match gravity_dir {
                0 => physics.vy += GRAVITY * dt,  // down
                1 => physics.vx -= GRAVITY * dt,  // left
                2 => physics.vy -= GRAVITY * dt,  // up
                3 => physics.vx += GRAVITY * dt,  // right
                _ => {}
            }
            physics.vx *= FRICTION;
            physics.vy *= FRICTION;
            physics.x += physics.vx * dt;
            physics.y += physics.vy * dt;

            let rot_friction = 0.96;
            physics.rot_h_vel *= rot_friction;
            physics.rot_v_vel *= rot_friction;
            physics.rot_h += physics.rot_h_vel * dt;
            physics.rot_v += physics.rot_v_vel * dt;

            let (left_ext, right_ext, up_ext, down_ext) = compute_rotated_extents(physics.rot_h, physics.rot_v, heart_scale);

            if physics.x - left_ext < 1.0 {
                physics.x = 1.0 + left_ext;
                physics.vx = -physics.vx * BOUNCE_DAMPING;
                physics.rot_h_vel += 0.5;
            }
            let right_limit = if game_mode != GameMode::Sandbox {
                width as f64 - 27.0  // 25 char panel + 2 for border
            } else {
                width as f64 - 2.0
            };
            if physics.x + right_ext > right_limit {
                physics.x = right_limit - right_ext;
                physics.vx = -physics.vx * BOUNCE_DAMPING;
                physics.rot_h_vel -= 0.5;
            }
            if physics.y - up_ext < 1.0 {
                physics.y = 1.0 + up_ext;
                physics.vy = -physics.vy * BOUNCE_DAMPING;
            }
            if physics.y + down_ext > height as f64 - 7.0 {
                physics.y = height as f64 - 7.0 - down_ext;
                physics.vy = -physics.vy * BOUNCE_DAMPING;
            }

            if mouse_initialized {
                let dx = physics.x - prev_mouse_x;
                let dy = (physics.y - prev_mouse_y) * 2.0;
                let dist = (dx * dx + dy * dy).sqrt();

                if blackhole_mode {
                    if dist > 0.1 {
                        let pull_strength = 4000.0 / (dist + 0.5);
                        let pull_x = -dx / dist;
                        let pull_y = -dy / dist / 2.0;
                        physics.vx += pull_x * pull_strength * dt;
                        physics.vy += pull_y * pull_strength * dt;
                        physics.x += pull_x * 120.0 * dt;
                        physics.y += pull_y * 120.0 * dt;
                        physics.rot_h_vel += pull_y * 0.5;
                        physics.rot_v_vel -= pull_x * 0.5;
                    }
                    let max_spin = 2.0;
                    physics.rot_h_vel = physics.rot_h_vel.clamp(-max_spin, max_spin);
                    physics.rot_v_vel = physics.rot_v_vel.clamp(-max_spin, max_spin);
                } else if whitehole_mode {
                    if dist > 0.1 {
                        let push_strength = 5000.0 / (dist + 0.5);
                        let push_x = dx / dist;
                        let push_y = dy / dist / 2.0;
                        physics.vx += push_x * push_strength * dt;
                        physics.vy += push_y * push_strength * dt;
                        physics.x += push_x * 150.0 * dt;
                        physics.y += push_y * 150.0 * dt;
                        physics.rot_h_vel += push_y * 0.3;
                        physics.rot_v_vel -= push_x * 0.3;
                    }
                    let max_spin = 2.0;
                    physics.rot_h_vel = physics.rot_h_vel.clamp(-max_spin, max_spin);
                    physics.rot_v_vel = physics.rot_v_vel.clamp(-max_spin, max_spin);
                } else {
                    let cursor_radius = 2.0;
                    let min_dist = heart_size * 0.4 + cursor_radius;
                    if dist < min_dist && dist > 0.01 {
                        let push_x = dx / dist;
                        let push_y = dy / dist / 2.0;
                        let overlap = min_dist - dist;
                        physics.x += push_x * overlap * 1.1;
                        physics.y += push_y * overlap * 1.1;
                        let dot = physics.vx * push_x + physics.vy * push_y;
                        if dot < 0.0 {
                            physics.vx -= 2.0 * dot * push_x * BOUNCE_DAMPING;
                            physics.vy -= 2.0 * dot * push_y * BOUNCE_DAMPING;
                        }
                        physics.rot_h_vel += push_x * 0.3;
                        physics.rot_v_vel += push_y * 0.3;
                    }
                }
            }
        }

        let collision_radius = heart_size * 0.4;
        let cell_size = collision_radius * 2.5;
        let grid_w = (width as f64 / cell_size).ceil() as usize + 1;
        let grid_h = (height as f64 / cell_size).ceil() as usize + 1;

        let mut grid: Vec<Vec<usize>> = vec![Vec::new(); grid_w * grid_h];
        for (i, h) in hearts.iter().enumerate() {
            let gx = ((h.x / cell_size) as usize).min(grid_w - 1);
            let gy = ((h.y / cell_size) as usize).min(grid_h - 1);
            grid[gy * grid_w + gx].push(i);
        }

        for i in 0..hearts.len() {
            let gx = ((hearts[i].x / cell_size) as usize).min(grid_w - 1);
            let gy = ((hearts[i].y / cell_size) as usize).min(grid_h - 1);

            for dy in 0..=1i32 {
                for dx in 0..=1i32 {
                    let nx = (gx as i32 + dx) as usize;
                    let ny = (gy as i32 + dy) as usize;
                    if nx >= grid_w || ny >= grid_h { continue; }

                    for &j in &grid[ny * grid_w + nx] {
                        if j <= i { continue; }

                        let dx = hearts[j].x - hearts[i].x;
                        let dy = (hearts[j].y - hearts[i].y) * 2.0;
                        let dist = (dx * dx + dy * dy).sqrt();
                        let min_dist = collision_radius * 2.0;

                        if dist < min_dist && dist > 0.01 {
                            let overlap = min_dist - dist;
                            let nx = dx / dist;
                            let ny = dy / dist / 2.0;

                            hearts[i].x -= nx * overlap * 0.5;
                            hearts[i].y -= ny * overlap * 0.5;
                            hearts[j].x += nx * overlap * 0.5;
                            hearts[j].y += ny * overlap * 0.5;

                            let rel_vx = hearts[j].vx - hearts[i].vx;
                            let rel_vy = hearts[j].vy - hearts[i].vy;
                            let rel_dot = rel_vx * nx + rel_vy * ny * 2.0;

                            if rel_dot < 0.0 {
                                let impulse = rel_dot * BOUNCE_DAMPING;
                                hearts[i].vx += impulse * nx;
                                hearts[i].vy += impulse * ny;
                                hearts[j].vx -= impulse * nx;
                                hearts[j].vy -= impulse * ny;
                                hearts[i].rot_h_vel += ny * 0.3;
                                hearts[j].rot_h_vel -= ny * 0.3;
                            }
                        }
                    }
                }
            }
        }

        let game_area_width = width as f64 - 25.0;  // Reserve 25 chars for right panel

        match game_mode {
            GameMode::Sandbox => {},
            GameMode::Juggler => {
                game_time += dt;
                score = (game_time * 10.0) as u32;  // Score = deciseconds survived

                if last_spawn.elapsed().as_secs_f64() > 5.0 && hearts.len() < 20 {
                    hearts.push(Physics {
                        x: rand_f64() * (game_area_width - 40.0) + 20.0,
                        y: 5.0,
                        vx: (rand_f64() - 0.5) * 30.0,
                        vy: rand_f64() * 10.0,
                        rot_h: rand_f64() * std::f64::consts::TAU,
                        rot_v: rand_f64() * std::f64::consts::TAU,
                        rot_h_vel: (rand_f64() - 0.5) * 2.0,
                        rot_v_vel: (rand_f64() - 0.5) * 2.0,
                    });
                    last_spawn = Instant::now();
                }

                let floor_y = height as f64 - 7.0;  // Same as bottom wall collision
                let right_wall = game_area_width - 2.0;
                for heart in &hearts {
                    let touched_floor = match gravity_dir {
                        0 => heart.y >= floor_y - heart_size * 0.3,  // Near bottom
                        1 => heart.x <= 2.0 + heart_size * 0.3,      // Near left
                        2 => heart.y <= 2.0 + heart_size * 0.3,      // Near top
                        3 => heart.x >= right_wall - heart_size * 0.3, // Near right
                        _ => false,
                    };
                    if touched_floor {
                        game_mode = GameMode::Sandbox;
                        break;
                    }
                }
            },
            GameMode::TargetZones => {
                if target.is_none() {
                    let tx = rand_f64() * (game_area_width - 20.0) + 10.0;
                    let ty = rand_f64() * (height as f64 - 20.0) + 10.0;
                    target = Some((tx, ty, 5.0));
                }

                if let Some((tx, ty, tr)) = target {
                    for heart in &hearts {
                        let dx = heart.x - tx;
                        let dy = heart.y - ty;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < tr + heart_size * 0.3 {
                            targets_hit += 1;
                            score = targets_hit * 100;
                            target = None;
                            break;
                        }
                    }
                }
            },
            GameMode::HeartGolf => {
                if target.is_none() {
                    let tx = rand_f64() * (game_area_width - 20.0) + 10.0;
                    let ty = rand_f64() * (height as f64 - 20.0) + 10.0;
                    target = Some((tx, ty, 4.0));
                }

                if let Some((tx, ty, tr)) = target {
                    if let Some(heart) = hearts.first() {
                        let dx = heart.x - tx;
                        let dy = heart.y - ty;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist < tr + heart_size * 0.3 {
                            score += 1000_u32.saturating_sub(golf_touches * 50);  // Fewer touches = more points
                            golf_hole += 1;
                            golf_touches = 0;
                            target = None;
                            if let Some(h) = hearts.first_mut() {
                                h.x = rand_f64() * (game_area_width - 40.0) + 20.0;
                                h.y = height as f64 / 2.0;
                                h.vx = 0.0;
                                h.vy = 0.0;
                            }
                        }
                    }
                }
            },
        }

        let w = width as usize;
        let h = height as usize;
        let mut output: Vec<u8> = vec![b' '; w * h];
        let mut zbuffer: Vec<f64> = vec![f64::NEG_INFINITY; w * h];
        let mut lum_buffer: Vec<f64> = vec![0.0; w * h];

        let panel_top = h.saturating_sub(5);
        let panel_x = w.saturating_sub(25);

        let heart_scale = heart_size * 0.55;

        for physics in &hearts {
            let (sin_h, cos_h) = physics.rot_h.sin_cos();
            let (sin_v, cos_v) = physics.rot_v.sin_cos();

            let ldx = (prev_mouse_x - physics.x) / heart_size;
            let ldy = (prev_mouse_y - physics.y) / heart_size;
            let ldz = 1.0;
            let ll = (ldx*ldx + ldy*ldy + ldz*ldz).sqrt().max(0.01);
            let (light_x, light_y, light_z) = (ldx/ll, -ldy/ll, ldz/ll);

            let min_sx = ((physics.x - heart_size * 1.2) as i32).max(1);
            let max_sx = ((physics.x + heart_size * 1.2) as i32).min(w as i32 - 1);
            let min_sy = ((physics.y - heart_size * 1.2) as i32).max(1);
            let max_sy = ((physics.y + heart_size * 1.2) as i32).min(h as i32 - 1);

            for sy in min_sy..max_sy {
                for sx in min_sx..max_sx {
                    let hx_screen = (sx as f64 - physics.x) / heart_scale;
                    let hz_screen = -(sy as f64 - physics.y) / (heart_scale * 0.5);

                    let cam_dist = 4.0;
                    let ox = hx_screen * cos_h - cam_dist * sin_h;
                    let oy = -hx_screen * sin_h - cam_dist * cos_h;
                    let oz = hz_screen * cos_v;
                    let oy2 = oy * cos_v + hz_screen * sin_v;
                    let oz2 = -oy * sin_v + oz;

                    let dx = sin_h;
                    let dy = cos_h * cos_v;
                    let dz = -cos_h * sin_v;

                    if let Some((_hx, _hy, _hz, nx, ny, nz)) = ray_march_heart(ox, oy2, oz2, dx, dy, dz) {
                        let depth = oy2 * cos_h + ox * sin_h;
                        let ny_rv = ny * cos_v - nz * sin_v;
                        let nz_rv = ny * sin_v + nz * cos_v;
                        let nx_screen = nx * cos_h + ny_rv * sin_h;
                        let ny_screen = -nx * sin_h + ny_rv * cos_h;
                        let nz_screen = nz_rv;

                        let idx = sy as usize * w + sx as usize;
                        if depth > zbuffer[idx] {
                            zbuffer[idx] = depth;
                            let dot = nx_screen * light_x + nz_screen * light_y + ny_screen * light_z;
                            let diffuse = (dot * 0.5 + 0.5).max(0.0);
                            let ambient = 0.15;
                            let raw_lum: f64 = ambient + (1.0 - ambient) * diffuse;
                            let lum = raw_lum.clamp(0.1, 0.95);
                            let lum_idx = ((lum * 11.99) as usize).min(11);
                            output[idx] = luminance_chars[lum_idx];
                            lum_buffer[idx] = lum;
                        }
                    }
                }
            }
        }


        let right_border = if game_mode != GameMode::Sandbox && panel_x > 1 { panel_x } else { w - 1 };

        for x in 0..=right_border {
            let ch = if x == 0 || x == right_border { b'+' } else { b'-' };
            output[x] = ch;
            lum_buffer[x] = if gravity_dir == 2 { -1.5 } else { -1.0 };
        }
        if game_mode != GameMode::Sandbox && panel_x > 1 {
        }

        for y in 1..panel_top {
            output[y * w] = b'|';
            lum_buffer[y * w] = if gravity_dir == 1 { -1.5 } else { -1.0 };

            if game_mode != GameMode::Sandbox && panel_x > 1 {
            } else {
                output[y * w + w - 1] = b'|';
                lum_buffer[y * w + w - 1] = if gravity_dir == 3 { -1.5 } else { -1.0 };
            }
        }

        if let Some((tx, ty, tr)) = target {
            let rings: [(f64, f64); 2] = [
                (1.0, 1.2),    // outer ring: full radius, speed 1.2
                (0.5, -2.0),   // inner ring: half radius, speed -2.0 (opposite direction)
            ];

            for (ring_ratio, speed) in rings {
                let ring_radius = tr * ring_ratio;
                let rot_angle = animation_time * speed;
                let (sin_r, cos_r) = rot_angle.sin_cos();

                let num_points = 80;
                for i in 0..num_points {
                    let theta = (i as f64 / num_points as f64) * std::f64::consts::TAU;
                    let (sin_t, cos_t) = theta.sin_cos();

                    let x3d = ring_radius * cos_t;
                    let y3d = 0.0;
                    let z3d = ring_radius * sin_t;

                    let x_rot = x3d;
                    let y_rot = y3d * cos_r - z3d * sin_r;
                    let z_rot = y3d * sin_r + z3d * cos_r;

                    let sx = tx + x_rot * 2.0;
                    let sy = ty + y_rot;

                    for thick_offset in -1i32..=1 {
                        let draw_x = (sx + thick_offset as f64).round() as i32;
                        let draw_y = sy.round() as i32;

                        if draw_x >= 1 && draw_x < w as i32 - 26 && draw_y >= 1 && draw_y < panel_top as i32 {
                            let idx = draw_y as usize * w + draw_x as usize;
                            let depth = z_rot;
                            if depth > zbuffer[idx] {
                                zbuffer[idx] = depth;
                                let brightness = (z_rot / ring_radius + 1.0) * 0.5; // 0 to 1
                                let lum_idx = (brightness * 11.99) as usize;
                                output[idx] = luminance_chars[lum_idx.min(11)];
                                lum_buffer[idx] = -6.0 - brightness * 0.5; // yellow/orange target colors
                            }
                        }
                    }
                }
            }
        }

        if game_mode != GameMode::Sandbox && panel_x > 1 {
            for y in 1..panel_top {
                let idx = y * w + panel_x;
                output[idx] = b'|';
                lum_buffer[idx] = -1.0;
            }
            output[panel_x] = b'+';
            lum_buffer[panel_x] = -1.0;
            let idx = panel_top * w + panel_x;
            output[idx] = b'+';
            lum_buffer[idx] = -1.0;

            for x in (panel_x + 1)..(w - 1) {
                output[x] = b'-';
                lum_buffer[x] = -1.0;
            }

            let mode_name = match game_mode {
                GameMode::Sandbox => "SANDBOX",
                GameMode::Juggler => "JUGGLER",
                GameMode::TargetZones => "TARGETS",
                GameMode::HeartGolf => "GOLF",
            };
            let mode_str = format!(" {} ", mode_name);
            for (i, ch) in mode_str.bytes().enumerate() {
                if panel_x + 2 + i < w - 1 {
                    let idx = 2 * w + panel_x + 2 + i;
                    output[idx] = ch;
                    lum_buffer[idx] = -8.5;
                }
            }

            let score_str = format!(" Score: {} ", score);
            for (i, ch) in score_str.bytes().enumerate() {
                if panel_x + 2 + i < w - 1 {
                    let idx = 4 * w + panel_x + 2 + i;
                    output[idx] = ch;
                    lum_buffer[idx] = -8.5;
                }
            }

            let info_str = match game_mode {
                GameMode::Juggler => format!(" Time: {:.1}s ", game_time),
                GameMode::TargetZones => format!(" Hits: {} ", targets_hit),
                GameMode::HeartGolf => format!(" Hole: {} Hits: {} ", golf_hole, golf_touches),
                _ => String::new(),
            };
            for (i, ch) in info_str.bytes().enumerate() {
                if panel_x + 2 + i < w - 1 {
                    let idx = 6 * w + panel_x + 2 + i;
                    output[idx] = ch;
                    lum_buffer[idx] = -8.5;
                }
            }
        }

        if mouse_initialized {
            let cx = prev_mouse_x.round() as i32;
            let cy = prev_mouse_y.round() as i32;
            if cx >= 1 && cx < w as i32 - 1 && cy >= 1 && cy < h as i32 - 1 {
                let idx = cy as usize * w + cx as usize;
                output[idx] = b'O';
                lum_buffer[idx] = -5.0;  // mark as cursor (white)
            }
        }

        if blackhole_mode {
            let cx = prev_mouse_x.round() as i32;
            let cy = prev_mouse_y.round() as i32;

            for dy in -1i32..=1 {
                for dx in -2i32..=2 {
                    let sx = cx + dx;
                    let sy = cy + dy;
                    if sx >= 1 && sx < w as i32 - 1 && sy >= 1 && sy < h as i32 - 1 {
                        let dist = ((dx as f64 / 2.0).powi(2) + (dy as f64).powi(2)).sqrt();
                        if dist <= 1.2 {
                            let idx = sy as usize * w + sx as usize;
                            output[idx] = b' ';
                            lum_buffer[idx] = -10.0;
                        }
                    }
                }
            }
        }

        if whitehole_mode {
            let cx = prev_mouse_x.round() as i32;
            let cy = prev_mouse_y.round() as i32;

            for dy in -2i32..=2 {
                for dx in -5i32..=5 {
                    let sx = cx + dx;
                    let sy = cy + dy;
                    if sx >= 1 && sx < w as i32 - 1 && sy >= 1 && sy < h as i32 - 1 {
                        let dist = ((dx as f64 / 2.0).powi(2) + (dy as f64).powi(2)).sqrt();
                        if dist <= 2.5 {
                            let idx = sy as usize * w + sx as usize;
                            let intensity = 1.0 - dist / 2.5;  // 1.0 at center, 0.0 at edge
                            let lum_idx = (intensity * 11.99) as usize;
                            output[idx] = luminance_chars[lum_idx.min(11)];
                            lum_buffer[idx] = -11.0 - (1.0 - intensity);  // -11 to -12
                        }
                    }
                }
            }
        }

        let right_edge = if game_mode != GameMode::Sandbox && panel_x > 1 { panel_x } else { w - 1 };
        for x in 0..w {
            let idx = panel_top * w + x;
            let ch = if x == 0 || x == w - 1 || x == right_edge { b'+' } else { b'-' };
            output[idx] = ch;
            if x <= right_edge {
                lum_buffer[idx] = if gravity_dir == 0 { -1.5 } else { -1.0 };
            } else {
                lum_buffer[idx] = -1.0;  // gray for panel area
            }
        }

        for y in (panel_top + 1)..h {
            for x in 1..(w - 1) {
                let idx = y * w + x;
                output[idx] = b' ';
                lum_buffer[idx] = 0.0;
            }
        }

        let controls1 = "0: Sandbox | 1: Juggler | 2: Targets | 3: Golf | R: gravity | Q: spawn | ESC: quit";
        let controls2 = "W: black hole | E: white hole | Arrows: push | Scroll: resize";

        let ctrl1_row = panel_top + 1;
        let ctrl2_row = panel_top + 2;

        for (i, ch) in controls1.bytes().enumerate() {
            if 2 + i < w - 1 {
                let idx = ctrl1_row * w + 2 + i;
                output[idx] = ch;
                lum_buffer[idx] = -8.5;
            }
        }

        for (i, ch) in controls2.bytes().enumerate() {
            if 2 + i < w - 1 {
                let idx = ctrl2_row * w + 2 + i;
                output[idx] = ch;
                lum_buffer[idx] = -8.5;
            }
        }

        for y in panel_top..h {
            output[y * w] = b'|';
            output[y * w + w - 1] = b'|';
            lum_buffer[y * w] = -1.0;
            lum_buffer[y * w + w - 1] = -1.0;
        }
        for x in 0..w {
            let idx = (h - 1) * w + x;
            output[idx] = if x == 0 || x == w - 1 { b'+' } else { b'-' };
            lum_buffer[idx] = -1.0;
        }

        let mut result = String::with_capacity(w * h * 20);
        result.push_str("\x1b[H");

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let c = output[idx] as char;
                let lum = lum_buffer[idx];

                if lum < -10.5 {
                    let intensity = (-(lum + 11.0)).clamp(0.0, 1.0);  // 0 = bright, 1 = dark
                    let v = (255.0 * (1.0 - intensity * 0.9)) as u8;
                    result.push_str(&format!("\x1b[38;2;{};{};{}m{}", v, v, v, c));
                } else if lum < -9.0 {
                    result.push_str(&format!("\x1b[38;2;{};{};{}m{}", 30, 10, 50, c));
                } else if lum < -8.0 {
                    if lum < -8.3 {
                        result.push_str("\x1b[38;2;150;150;150m");  // light gray label
                    } else {
                        result.push_str("\x1b[38;2;50;50;50m");  // dark gray empty bar
                    }
                    result.push(c);
                } else if lum < -5.5 {
                    let ratio = (-(lum + 6.0)).clamp(0.0, 1.0);
                    let (r, g, b) = if ratio < 0.33 {
                        let t = ratio / 0.33;
                        (80 + (t * 100.0) as u8, 160, 60)
                    } else if ratio < 0.66 {
                        let t = (ratio - 0.33) / 0.33;
                        (180 + (t * 40.0) as u8, (160.0 - t * 60.0) as u8, 50)
                    } else {
                        let t = (ratio - 0.66) / 0.34;
                        (220, (100.0 - t * 40.0) as u8, (50.0 + t * 30.0) as u8)
                    };
                    result.push_str(&format!("\x1b[38;2;{};{};{}m{}", r, g, b, c));
                } else if lum < -4.0 {
                    result.push_str("\x1b[38;2;255;255;255m");
                    result.push(c);
                } else if lum < 0.0 {
                    if lum < -1.2 {
                        result.push_str("\x1b[38;2;100;120;160m");  // desaturated blue gravity wall
                    } else {
                        result.push_str("\x1b[38;2;80;80;80m");  // gray normal wall
                    }
                    result.push(c);
                } else if c != ' ' {
                    let brightness = lum.clamp(0.0, 1.0);
                    let mut r = 5.0 + 195.0 * brightness;    // 5 -> 200
                    let mut g = 10.0 + 210.0 * brightness;   // 10 -> 220
                    let mut b = 30.0 + 225.0 * brightness;   // 30 -> 255

                    if blackhole_mode {
                        let dx = (x as f64 - prev_mouse_x) / 2.0;
                        let dy = y as f64 - prev_mouse_y;
                        let dist_to_cursor = (dx * dx + dy * dy).sqrt();
                        let dark_factor = (1.0 - dist_to_cursor / 10.0).max(0.0);
                        r = r * (1.0 - dark_factor * 0.9);
                        g = g * (1.0 - dark_factor * 0.9);
                        b = b * (1.0 - dark_factor * 0.9);
                    }

                    if whitehole_mode {
                        let dx = (x as f64 - prev_mouse_x) / 2.0;
                        let dy = y as f64 - prev_mouse_y;
                        let dist_to_cursor = (dx * dx + dy * dy).sqrt();
                        let bright_factor = (1.0 - dist_to_cursor / 10.0).max(0.0);
                        r = r + (255.0 - r) * bright_factor;
                        g = g + (255.0 - g) * bright_factor;
                        b = b + (255.0 - b) * bright_factor;
                    }

                    result.push_str(&format!("\x1b[38;2;{};{};{}m{}", r as u8, g as u8, b as u8, c));
                } else {
                    result.push(' ');
                }
            }
            if y < h - 1 {
                result.push_str("\x1b[0m\r\n");
            }
        }
        result.push_str("\x1b[0m");

        print!("{}", result);
        stdout.flush()?;

        std::thread::sleep(Duration::from_micros(6944));
    }

    print!("\x1b[?7h");
    execute!(stdout, crossterm::event::DisableMouseCapture)?;
    execute!(stdout, Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

fn rand_f64() -> f64 {
    use std::time::SystemTime;
    static mut SEED: u64 = 0;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as u64;
    unsafe {
        SEED = SEED.wrapping_mul(6364136223846793005).wrapping_add(nanos ^ 1442695040888963407);
        (SEED >> 33) as f64 / (1u64 << 31) as f64
    }
}
