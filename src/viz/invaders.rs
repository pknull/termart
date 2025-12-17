//! Space Invaders style game - simple ASCII version

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use crate::viz::VizState;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use rand::prelude::*;
use std::io;

// Game constants
const ALIEN_GRID_ROWS: usize = 5;
const ALIEN_SPACING: usize = 3;

// Animated alien characters (frame 0, frame 1) for each type
const ALIEN_CHARS: [[char; 2]; 3] = [
    ['◆', '◇'],  // Type 0 (top row) - diamond
    ['▼', '▽'],  // Type 1 (middle rows) - triangle
    ['●', '○'],  // Type 2 (bottom rows) - circle
];

const PLAYER_CHAR: char = '▲';
const BULLET_CHAR: char = '│';
const ALIEN_BULLET_CHAR: char = '╪';
const SHIELD_CHARS: [char; 3] = ['█', '▓', '░'];  // Health 3, 2, 1

// Speeds (units per frame)
const PLAYER_MOVE: f32 = 1.5;
const BULLET_SPEED: f32 = 0.8;
const ALIEN_BULLET_SPEED: f32 = 0.3;
const FIRE_COOLDOWN: f32 = 0.2;
const AI_FIRE_COOLDOWN: f32 = 0.15;
const ALIEN_FIRE_INTERVAL: f32 = 0.8;
const AUTO_RESTART_DELAY: f32 = 2.0;

// Static UI strings
const HINT: &str = "←/→:Move SPACE:Fire A:AI R:Reset Q:Quit";
const MSG_GAME_OVER: &str = "GAME OVER - Press R to restart";

#[derive(Clone, Copy, PartialEq)]
enum GameState {
    Playing,
    GameOver,
}

struct Alien {
    x: f32,
    y: f32,
    alive: bool,
    alien_type: usize,
}

struct Bullet {
    x: f32,
    y: f32,
    active: bool,
    is_player: bool,
}

struct Shield {
    x: i32,
    y: i32,
    health: u8,
}

struct Game {
    player_x: f32,
    player_lives: u8,
    score: u32,
    high_score: u32,
    aliens: Vec<Alien>,
    alien_dir: f32,
    alien_speed: f32,
    alien_frame: usize,
    alien_move_timer: f32,
    bullets: Vec<Bullet>,
    shields: Vec<Shield>,
    game_state: GameState,
    wave: u32,
    alien_fire_timer: f32,
}

#[inline]
fn calc_alien_cols(w: usize) -> usize {
    // Original: 11 cols on 80-char terminal (11*3=33 pixels, ~41% of width)
    let grid_width = (w as f32 * 0.41) as usize;
    (grid_width / ALIEN_SPACING).clamp(5, 30)
}

fn create_aliens(w: usize, _h: usize) -> Vec<Alien> {
    let cols = calc_alien_cols(w);
    let grid_width = cols as f32 * ALIEN_SPACING as f32;
    let start_x = ((w as f32 - grid_width) / 2.0).max(2.0);
    let start_y = 3.0;

    let mut aliens = Vec::with_capacity(ALIEN_GRID_ROWS * cols);
    for row in 0..ALIEN_GRID_ROWS {
        let alien_type = if row == 0 { 0 } else if row < 3 { 1 } else { 2 };
        for col in 0..cols {
            aliens.push(Alien {
                x: start_x + col as f32 * ALIEN_SPACING as f32,
                y: start_y + row as f32 * 2.0,
                alive: true,
                alien_type,
            });
        }
    }
    aliens
}

fn create_shields(w: usize, h: usize) -> Vec<Shield> {
    let shield_y = h as i32 - 5;
    let num_shields = 4;
    let spacing = w / (num_shields + 1);
    let mut shields = Vec::new();

    for i in 0..num_shields {
        let base_x = (spacing * (i + 1)) as i32 - 2;
        // Create a small bunker shape
        //  ████
        //  █  █
        for dx in 0..4 {
            shields.push(Shield {
                x: base_x + dx,
                y: shield_y,
                health: 3,
            });
        }
        // Bottom row with gap
        shields.push(Shield { x: base_x, y: shield_y + 1, health: 3 });
        shields.push(Shield { x: base_x + 3, y: shield_y + 1, health: 3 });
    }
    shields
}

pub fn run(term: &mut Terminal, config: &FractalConfig, rng: &mut StdRng) -> io::Result<()> {
    let mut state = VizState::new(config.time_step);

    let (init_w, init_h) = term.size();
    let mut w = init_w as usize;
    let mut h = init_h as usize;

    let mut game = Game {
        player_x: w as f32 / 2.0,
        player_lives: 3,
        score: 0,
        high_score: 0,
        aliens: create_aliens(w, h),
        alien_dir: 1.0,
        alien_speed: 0.5,
        alien_frame: 0,
        alien_move_timer: 0.0,
        bullets: Vec::new(),
        shields: create_shields(w, h),
        game_state: GameState::Playing,
        wave: 1,
        alien_fire_timer: 0.0,
    };

    let mut player_fire_cooldown = 0.0f32;
    let mut auto_play = true;  // Start with AI on
    let mut game_over_timer = 0.0f32;

    // Reusable string buffers
    let mut score_buf = String::with_capacity(48);
    let mut restart_buf = String::with_capacity(24);

    // Cached layout values
    let mut alien_cols = calc_alien_cols(w);
    let mut player_y_i32 = (h - 2) as i32;

    loop {
        // Check for terminal resize
        let (new_w, new_h) = crossterm::terminal::size().unwrap_or((w as u16, h as u16));
        if new_w as usize != w || new_h as usize != h {
            w = new_w as usize;
            h = new_h as usize;
            term.resize(new_w, new_h);
            term.clear_screen()?;
            game.aliens = create_aliens(w, h);
            game.shields = create_shields(w, h);
            game.player_x = w as f32 / 2.0;
            alien_cols = calc_alien_cols(w);
            player_y_i32 = (h - 2) as i32;
        }


        // Handle input
        if let Some((code, mods)) = term.check_key()? {
            match code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Char(' ') if game.game_state == GameState::Playing => {
                    if player_fire_cooldown <= 0.0 {
                        game.bullets.push(Bullet {
                            x: game.player_x,
                            y: (h - 2) as f32,
                            active: true,
                            is_player: true,
                        });
                        player_fire_cooldown = FIRE_COOLDOWN;
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if game.game_state == GameState::Playing {
                        game.player_x = (game.player_x - PLAYER_MOVE).max(1.0);
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if game.game_state == GameState::Playing {
                        game.player_x = (game.player_x + PLAYER_MOVE).min((w - 2) as f32);
                    }
                }
                KeyCode::Char('r') => {
                    game = Game {
                        player_x: w as f32 / 2.0,
                        player_lives: 3,
                        score: 0,
                        high_score: game.high_score,
                        aliens: create_aliens(w, h),
                        alien_dir: 1.0,
                        alien_speed: 0.5,
                        alien_frame: 0,
                        alien_move_timer: 0.0,
                        bullets: Vec::new(),
                        shields: create_shields(w, h),
                        game_state: GameState::Playing,
                        wave: 1,
                        alien_fire_timer: 0.0,
                    };
                }
                KeyCode::Char('a') => {
                    auto_play = !auto_play;
                }
                _ => {
                    if state.handle_key(code, mods) {
                        break;
                    }
                }
            }
        }

        let dt = state.speed;
        player_fire_cooldown = (player_fire_cooldown - dt).max(0.0);

        // Auto-play AI - predict bullet landing positions
        if auto_play && game.game_state == GameState::Playing {
            let player_y = (h - 2) as f32;

            // Predict where each bullet will land when it reaches player's row
            // Track both impact position AND urgency (how soon)
            let mut danger_zones: Vec<(f32, f32)> = Vec::new();  // (x, urgency)

            for bullet in &game.bullets {
                if bullet.active && !bullet.is_player && bullet.y < player_y {
                    // Bullets fall straight down
                    let impact_x = bullet.x;

                    // How far away is the bullet?
                    let dist = player_y - bullet.y;

                    // Urgency: closer = more urgent (higher value)
                    let urgency = 1.0 / (dist + 1.0);

                    // Track all bullets above us
                    danger_zones.push((impact_x, urgency));
                }
            }

            // Check if a position is safe
            // Use wider margin (3.0) for close bullets, narrower for distant
            let is_safe = |x: f32| -> bool {
                for &(danger_x, urgency) in &danger_zones {
                    // Close bullets need wider berth
                    let margin = if urgency > 0.1 { 3.5 } else { 2.5 };
                    if (x - danger_x).abs() < margin {
                        return false;
                    }
                }
                true
            };

            // Current position safe?
            let current_safe = is_safe(game.player_x);

            // Check potential escape positions - move further (6 pixels)
            let left_pos = (game.player_x - 6.0).max(1.0);
            let right_pos = (game.player_x + 6.0).min((w - 2) as f32);
            let left_safe = is_safe(left_pos);
            let right_safe = is_safe(right_pos);

            // Find best target: only consider aliens COMING TOWARD us
            let mut best_intercept: Option<f32> = None;
            let mut best_alien_y = 0.0f32;

            for alien in &game.aliens {
                if alien.alive {
                    let dx = alien.x - game.player_x;

                    // Is this alien coming toward us or moving away?
                    // alien_dir > 0 means moving right, < 0 means moving left
                    let alien_coming_toward = (dx > 0.0 && game.alien_dir < 0.0)  // alien is right, moving left (toward us)
                                           || (dx < 0.0 && game.alien_dir > 0.0)  // alien is left, moving right (toward us)
                                           || dx.abs() < 3.0;  // already above us

                    // Only target aliens coming toward us (or very close)
                    if alien_coming_toward {
                        // Prioritize lower aliens (more dangerous)
                        if alien.y > best_alien_y || best_intercept.is_none() {
                            // Predict where alien will be when bullet reaches it
                            let dist = player_y - alien.y;
                            let frames_to_hit = dist / BULLET_SPEED;

                            let alive_count = game.aliens.iter().filter(|a| a.alive).count();
                            let speed_mult = 1.0 + (55 - alive_count.min(55)) as f32 * 0.02;
                            let move_interval = game.alien_speed / speed_mult;
                            let alien_moves = (frames_to_hit * dt / move_interval) as i32;

                            let predicted_x = alien.x + (alien_moves as f32 * 2.0 * game.alien_dir);
                            let predicted_x = predicted_x.clamp(1.0, (w - 2) as f32);

                            best_alien_y = alien.y;
                            best_intercept = Some(predicted_x);
                        }
                    }
                }
            }

            // If no alien coming toward us, move ahead to where they'll turn around
            let fallback_position: Option<f32> = if best_intercept.is_none() {
                // Aliens moving right (dir > 0) → go right to meet them at the edge
                // Aliens moving left (dir < 0) → go left to meet them
                // Find the leading edge of the alien formation
                let mut leading_x = if game.alien_dir > 0.0 { 0.0f32 } else { w as f32 };
                for alien in &game.aliens {
                    if alien.alive {
                        if game.alien_dir > 0.0 && alien.x > leading_x {
                            leading_x = alien.x;
                        } else if game.alien_dir < 0.0 && alien.x < leading_x {
                            leading_x = alien.x;
                        }
                    }
                }
                // Position slightly ahead of where aliens are heading
                Some(leading_x + game.alien_dir * 5.0)
            } else {
                None
            };

            // Movement decision
            if !current_safe {
                // Dodging bullets takes priority
                if left_safe && game.player_x > 5.0 {
                    game.player_x = left_pos;
                } else if right_safe && game.player_x < (w - 6) as f32 {
                    game.player_x = right_pos;
                } else if left_safe {
                    game.player_x = left_pos;
                } else if right_safe {
                    game.player_x = right_pos;
                }
            } else if let Some(intercept_x) = best_intercept {
                // Alien coming toward us - intercept
                let dx = intercept_x - game.player_x;
                if dx.abs() > 1.5 {
                    let step = if dx > 0.0 { 1.0 } else { -1.0 };
                    let new_x = (game.player_x + step).clamp(1.0, (w - 2) as f32);
                    let mid_x = game.player_x + step * 0.5;
                    if is_safe(new_x) && is_safe(mid_x) {
                        game.player_x = new_x;
                    }
                }
            } else if let Some(ahead_x) = fallback_position {
                // No alien coming - move ahead to meet them when they turn
                let ahead_x = ahead_x.clamp(1.0, (w - 2) as f32);
                let dx = ahead_x - game.player_x;
                if dx.abs() > 2.0 {
                    let step = if dx > 0.0 { 1.5 } else { -1.5 };
                    let new_x = (game.player_x + step).clamp(1.0, (w - 2) as f32);
                    if is_safe(new_x) {
                        game.player_x = new_x;
                    }
                }
            }

            // Fire when at intercept position
            if player_fire_cooldown <= 0.0 {
                if let Some(intercept_x) = best_intercept {
                    if (intercept_x - game.player_x).abs() < 1.5 {
                        game.bullets.push(Bullet {
                            x: game.player_x,
                            y: player_y,
                            active: true,
                            is_player: true,
                        });
                        player_fire_cooldown = AI_FIRE_COOLDOWN;
                    }
                }
            }
        }

        // Auto-restart on game over (with delay)
        if auto_play && game.game_state == GameState::GameOver {
            game_over_timer += dt;
            if game_over_timer >= AUTO_RESTART_DELAY {
                game_over_timer = 0.0;
                game = Game {
                    player_x: w as f32 / 2.0,
                    player_lives: 3,
                    score: 0,
                    high_score: game.high_score,
                    aliens: create_aliens(w, h),
                    alien_dir: 1.0,
                    alien_speed: 0.5,
                    alien_frame: 0,
                    alien_move_timer: 0.0,
                    bullets: Vec::new(),
                    shields: create_shields(w, h),
                    game_state: GameState::Playing,
                    wave: 1,
                    alien_fire_timer: 0.0,
                };
            }
        } else {
            game_over_timer = 0.0;
        }

        // Game logic (only when playing)
        if game.game_state == GameState::Playing {
            // Move aliens
            game.alien_move_timer += dt;
            let alive_count = game.aliens.iter().filter(|a| a.alive).count();
            let speed_mult = 1.0 + (55 - alive_count.min(55)) as f32 * 0.02;
            let move_interval = game.alien_speed / speed_mult;

            if game.alien_move_timer >= move_interval {
                game.alien_move_timer = 0.0;
                game.alien_frame = 1 - game.alien_frame;  // Toggle animation

                // Check edges
                let mut hit_edge = false;
                for alien in &game.aliens {
                    if alien.alive {
                        let next_x = alien.x + game.alien_dir * 2.0;
                        if next_x < 1.0 || next_x >= (w - 1) as f32 {
                            hit_edge = true;
                            break;
                        }
                    }
                }

                if hit_edge {
                    game.alien_dir = -game.alien_dir;
                    for alien in &mut game.aliens {
                        alien.y += 1.0;
                    }
                } else {
                    for alien in &mut game.aliens {
                        alien.x += game.alien_dir * 2.0;
                    }
                }
            }

            // Alien firing
            game.alien_fire_timer += dt;
            if game.alien_fire_timer >= ALIEN_FIRE_INTERVAL {
                game.alien_fire_timer = 0.0;
                let alive_aliens: Vec<_> = game.aliens.iter().filter(|a| a.alive).collect();
                if !alive_aliens.is_empty() && rng.gen_bool(0.4) {
                    let shooter = alive_aliens[rng.gen_range(0..alive_aliens.len())];
                    game.bullets.push(Bullet {
                        x: shooter.x,
                        y: shooter.y + 1.0,
                        active: true,
                        is_player: false,
                    });
                }
            }

            // Move bullets
            for bullet in &mut game.bullets {
                if bullet.active {
                    if bullet.is_player {
                        bullet.y -= BULLET_SPEED;
                        if bullet.y < 0.0 {
                            bullet.active = false;
                        }
                    } else {
                        bullet.y += ALIEN_BULLET_SPEED;
                        if bullet.y >= h as f32 {
                            bullet.active = false;
                        }
                    }
                }
            }

            // Collision: player bullets vs aliens
            for bullet in &mut game.bullets {
                if bullet.active && bullet.is_player {
                    for alien in &mut game.aliens {
                        if alien.alive {
                            let hit = (bullet.x - alien.x).abs() < 1.5
                                   && (bullet.y - alien.y).abs() < 1.0;
                            if hit {
                                alien.alive = false;
                                bullet.active = false;
                                game.score += match alien.alien_type {
                                    0 => 30,
                                    1 => 20,
                                    _ => 10,
                                };
                                if game.score > game.high_score {
                                    game.high_score = game.score;
                                }
                                break;
                            }
                        }
                    }
                }
            }

            // Collision: bullets vs shields - DELETE the shield piece on hit
            for bullet in &mut game.bullets {
                if bullet.active {
                    let bx = bullet.x.round() as i32;
                    let by = bullet.y.round() as i32;
                    for shield in &mut game.shields {
                        if shield.health > 0 && shield.x == bx && shield.y == by {
                            // Delete this shield piece entirely
                            shield.health = 0;
                            bullet.active = false;
                            break;
                        }
                    }
                }
            }

            // Collision: alien bullets vs player
            let player_y = (h - 2) as f32;
            for bullet in &mut game.bullets {
                if bullet.active && !bullet.is_player {
                    let hit = (bullet.x - game.player_x).abs() < 1.0
                           && (bullet.y - player_y).abs() < 1.0;
                    if hit {
                        bullet.active = false;
                        game.player_lives -= 1;
                        if game.player_lives == 0 {
                            game.game_state = GameState::GameOver;
                        }
                    }
                }
            }

            // Check for aliens reaching bottom
            for alien in &game.aliens {
                if alien.alive && alien.y >= (h - 4) as f32 {
                    game.game_state = GameState::GameOver;
                    break;
                }
            }

            // Check for victory (next wave)
            if game.aliens.iter().all(|a| !a.alive) {
                game.wave += 1;
                game.aliens = create_aliens(w, h);
                game.alien_speed = (game.alien_speed * 0.9).max(0.2);
            }

            game.bullets.retain(|b| b.active);
        }

        // Render
        term.clear();

        // Draw score and lives - use reusable buffer
        score_buf.clear();
        use std::fmt::Write;
        if auto_play {
            let _ = write!(score_buf, "SCORE:{:05} HI:{:05} WAVE:{} [AI] LIVES:",
                game.score, game.high_score, game.wave);
        } else {
            let _ = write!(score_buf, "SCORE:{:05} HI:{:05} WAVE:{} LIVES:",
                game.score, game.high_score, game.wave);
        }
        term.set_str(0, 0, &score_buf, Some(Color::White), false);
        for i in 0..game.player_lives {
            term.set((score_buf.len() + i as usize) as i32, 0, '♥', Some(Color::Red), true);
        }

        // Draw aliens with animation (alternating rows)
        // Cache cols outside loop
        for (i, alien) in game.aliens.iter().enumerate() {
            if alien.alive {
                let color = match alien.alien_type {
                    0 => Color::Magenta,
                    1 => Color::Cyan,
                    _ => Color::Green,
                };
                let row = i / alien_cols;
                let frame = (game.alien_frame + row) % 2;
                let ch = ALIEN_CHARS[alien.alien_type][frame];
                term.set(alien.x as i32, alien.y as i32, ch, Some(color), true);
            }
        }

        // Draw shields with degradation
        for shield in &game.shields {
            if shield.health > 0 {
                let ch = SHIELD_CHARS[3 - shield.health as usize];
                term.set(shield.x, shield.y, ch, Some(Color::Yellow), false);
            }
        }

        // Draw bullets
        for bullet in &game.bullets {
            if bullet.active {
                let (ch, color) = if bullet.is_player {
                    (BULLET_CHAR, Color::White)
                } else {
                    (ALIEN_BULLET_CHAR, Color::Red)
                };
                term.set(bullet.x as i32, bullet.y as i32, ch, Some(color), true);
            }
        }

        // Draw player
        term.set(game.player_x as i32, player_y_i32, PLAYER_CHAR, Some(Color::Green), true);

        // Draw game over or controls
        if game.game_state == GameState::GameOver {
            let x = (w as i32 - MSG_GAME_OVER.len() as i32) / 2;
            term.set_str(x, h as i32 / 2, MSG_GAME_OVER, Some(Color::Red), true);
            if auto_play {
                restart_buf.clear();
                let _ = write!(restart_buf, "Restarting in {:.0}s...", AUTO_RESTART_DELAY - game_over_timer);
                let x2 = (w as i32 - restart_buf.len() as i32) / 2;
                term.set_str(x2, h as i32 / 2 + 1, &restart_buf, Some(Color::DarkGrey), false);
            }
        } else {
            term.set_str(0, (h - 1) as i32, HINT, Some(Color::DarkGrey), false);
        }

        term.render()?;
        std::thread::sleep(std::time::Duration::from_secs_f32(state.speed));
    }

    Ok(())
}
