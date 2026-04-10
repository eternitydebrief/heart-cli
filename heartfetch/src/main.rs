use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
};
use std::io::{stdout, Write};
use std::time::{Duration, Instant};
use std::fs;
use std::process::Command;

fn heart_implicit(x: f64, y: f64, z: f64) -> f64 {
    let x2 = x * x;
    let y2 = y * y;
    let z2 = z * z;
    let z3 = z2 * z;

    let term = x2 + 2.25 * y2 + z2 - 1.0;
    let base = term * term * term - x2 * z3 - 0.045 * y2 * z3;

    let cleft_depth = 0.35;
    let cleft_width = 0.02;
    let z_pos = z.max(0.0);
    let dist2 = x2 + y2;
    let sharpness = 1.0 / (1.0 + dist2 / cleft_width).powi(3);
    let cleft = cleft_depth * sharpness * z_pos * z_pos;

    base + cleft
}

fn heart_gradient(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let eps = 0.015;
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
    let step = 0.08;
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

fn render_heart(rot_h: f64, width: usize, height: usize) -> Vec<Vec<(char, u8, u8, u8)>> {
    let luminance_chars: &[u8] = b".,-~:;=!*#$@";
    let mut buffer: Vec<Vec<(char, u8, u8, u8)>> = vec![vec![(' ', 0, 0, 0); width]; height];

    let (sin_h, cos_h) = rot_h.sin_cos();
    let rot_v = 0.0_f64;
    let (sin_v, cos_v) = rot_v.sin_cos();

    let heart_scale = height as f64 / 1.15;
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;

    let (light_x, light_y, light_z) = (0.15, 0.95, -0.1);

    for sy in 0..height {
        for sx in 0..width {
            let hx_screen = (sx as f64 - cx) / heart_scale;
            let hz_screen = -(sy as f64 - cy) / (heart_scale * 0.5);

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
                let ny_rv = ny * cos_v - nz * sin_v;
                let nz_rv = ny * sin_v + nz * cos_v;
                let nx_screen = nx * cos_h + ny_rv * sin_h;
                let ny_screen = -nx * sin_h + ny_rv * cos_h;
                let nz_screen = nz_rv;

                let dot = nx_screen * light_x + nz_screen * light_y + ny_screen * light_z;
                let diffuse = (dot * 0.5 + 0.5).max(0.0);
                let ambient = 0.15;
                let lum = (ambient + (1.0 - ambient) * diffuse).clamp(0.1, 0.95);
                let lum_idx = ((lum * 11.99) as usize).min(11);
                let c = luminance_chars[lum_idx] as char;

                let r = (5.0 + 195.0 * lum) as u8;
                let g = (10.0 + 210.0 * lum) as u8;
                let b = (30.0 + 225.0 * lum) as u8;

                buffer[sy][sx] = (c, r, g, b);
            }
        }
    }
    buffer
}

fn get_system_info() -> Vec<(String, String)> {
    let mut info = Vec::new();

    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let hostname = fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "localhost".to_string())
        .trim()
        .to_string();
    info.push(("".to_string(), format!("{}@{}", user, hostname)));
    info.push(("".to_string(), "-".repeat(user.len() + hostname.len() + 1)));

    if let Ok(os_release) = fs::read_to_string("/etc/os-release") {
        for line in os_release.lines() {
            if line.starts_with("PRETTY_NAME=") {
                let name = line.trim_start_matches("PRETTY_NAME=").trim_matches('"');
                info.push(("OS".to_string(), name.to_string()));
                break;
            }
        }
    }

    if let Ok(output) = Command::new("uname").arg("-r").output() {
        let kernel = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info.push(("Kernel".to_string(), kernel));
    }

    if let Ok(uptime_str) = fs::read_to_string("/proc/uptime") {
        if let Some(secs_str) = uptime_str.split_whitespace().next() {
            if let Ok(secs) = secs_str.parse::<f64>() {
                let hours = (secs / 3600.0) as u64;
                let mins = ((secs % 3600.0) / 60.0) as u64;
                info.push(("Uptime".to_string(), format!("{} hours, {} mins", hours, mins)));
            }
        }
    }

    if let Ok(shell) = std::env::var("SHELL") {
        let shell_name = shell.rsplit('/').next().unwrap_or(&shell);
        info.push(("Shell".to_string(), shell_name.to_string()));
    }

    if let Ok(term) = std::env::var("TERM") {
        info.push(("Terminal".to_string(), term));
    }

    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.starts_with("model name") {
                if let Some(name) = line.split(':').nth(1) {
                    let name = name.trim()
                        .replace("(R)", "")
                        .replace("(TM)", "")
                        .replace("CPU ", "");
                    let name = if name.len() > 35 {
                        format!("{}...", &name[..32])
                    } else {
                        name
                    };
                    info.push(("CPU".to_string(), name));
                    break;
                }
            }
        }
    }

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        let mut total: u64 = 0;
        let mut available: u64 = 0;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    total = val.parse().unwrap_or(0);
                }
            } else if line.starts_with("MemAvailable:") {
                if let Some(val) = line.split_whitespace().nth(1) {
                    available = val.parse().unwrap_or(0);
                }
            }
        }
        if total > 0 {
            let used = total - available;
            let used_mib = used / 1024;
            let total_mib = total / 1024;
            info.push(("Memory".to_string(), format!("{} MiB / {} MiB", used_mib, total_mib)));
        }
    }

    info
}

fn main() -> std::io::Result<()> {
    let mut stdout = stdout();

    let sys_info = get_system_info();

    let heart_height = sys_info.len();
    let heart_width = heart_height * 2;  // Aspect ratio correction

    execute!(stdout, Hide)?;

    let start = Instant::now();
    let revolution_time = 2.0; // 2 seconds for one revolution

    loop {
        let elapsed = start.elapsed().as_secs_f64();
        let progress = elapsed / revolution_time;

        if progress >= 1.0 {
            break;
        }

        let eased = if progress < 0.5 {
            2.0 * progress * progress
        } else {
            1.0 - (-2.0 * progress + 2.0).powi(2) / 2.0
        };
        let rot_h = eased * std::f64::consts::TAU;
        let heart = render_heart(rot_h, heart_width, heart_height);

        execute!(stdout, MoveTo(0, 0))?;

        for y in 0..heart_height {
            for x in 0..heart_width {
                let (c, r, g, b) = heart[y][x];
                if c != ' ' {
                    print!("\x1b[38;2;{};{};{}m{}", r, g, b, c);
                } else {
                    print!(" ");
                }
            }

            print!("\x1b[0m  "); // Gap between heart and info

            if y < sys_info.len() {
                let (label, value) = &sys_info[y];
                if label.is_empty() {
                    print!("\x1b[38;2;100;150;255m{}\x1b[0m", value);
                } else {
                    print!("\x1b[38;2;100;150;255m{}\x1b[0m: {}", label, value);
                }
            }

            print!("\x1b[K\n");
        }

        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(16)); // ~60fps
    }

    let heart = render_heart(0.0, heart_width, heart_height);
    execute!(stdout, MoveTo(0, 0))?;

    for y in 0..heart_height {
        for x in 0..heart_width {
            let (c, r, g, b) = heart[y][x];
            if c != ' ' {
                print!("\x1b[38;2;{};{};{}m{}", r, g, b, c);
            } else {
                print!(" ");
            }
        }

        print!("\x1b[0m  ");

        if y < sys_info.len() {
            let (label, value) = &sys_info[y];
            if label.is_empty() {
                print!("\x1b[38;2;100;150;255m{}\x1b[0m", value);
            } else {
                print!("\x1b[38;2;100;150;255m{}\x1b[0m: {}", label, value);
            }
        }

        print!("\x1b[K\n");
    }

    stdout.flush()?;
    execute!(stdout, Show)?;
    print!("\x1b[0m");
    stdout.flush()?;

    Ok(())
}
