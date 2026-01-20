//! Pong - Classic two-player game
//!
//! Controls:
//! - Player 1 (left):  W/S for up/down, '1' to toggle AI
//! - Player 2 (right): Up/Down arrows or I/K, '2' to toggle AI
//! - Space: pause
//! - R: reset game
//! - Q/Esc: quit

use crate::colors::{scheme_color, ColorState};
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use rand::Rng;
use std::io;

// Game constants
const PADDLE_HEIGHT: i32 = 5;
const PADDLE_HALF: f32 = PADDLE_HEIGHT as f32 / 2.0;
const PADDLE_X_LEFT: i32 = 2;
const BALL_SPEED: f32 = 15.0;
const PADDLE_SPEED: f32 = 35.0;
const AI_SPEED: f32 = 25.0;
const WIN_SCORE: u32 = 11;
const SPIN_FACTOR: f32 = 10.0;
const MAX_VX: f32 = 50.0;
const MAX_VY: f32 = 30.0;

// Collision constants
const PADDLE_COLLISION_WIDTH: f32 = 1.0;
const BALL_NUDGE_DISTANCE: f32 = 0.1;

// Static UI strings
const HINT: &str = "1:P1 AI  W/S:P1 move | 2:P2 AI  ↑/↓:P2 move | Space:pause R:reset ?:help";
const MSG_PAUSED: &str = "PAUSED";
const MSG_P1_WINS: &str = "PLAYER 1 WINS!";
const MSG_P2_WINS: &str = "PLAYER 2 WINS!";
const MSG_RESTART: &str = "Press SPACE to restart";

// Help text
const HELP_TEXT: &str = "\
PONG
─────────────────
W/S      P1 up/down
↑/↓/I/K  P2 up/down
1        Toggle P1 AI
2        Toggle P2 AI
R        Reset game
───────────────────────
 GLOBAL CONTROLS
 Space   Pause/resume
 !-()    Color scheme
 q/Esc   Quit
 ?       Close help
───────────────────────";

#[derive(Clone, Copy, PartialEq)]
enum Winner {
    None,
    Left,
    Right,
}

#[derive(Clone, Copy)]
struct Ball {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

struct Paddle {
    y: f32,
    score: u32,
    ai: bool,
}

struct Game {
    ball: Ball,
    left: Paddle,
    right: Paddle,
    paused: bool,
    game_over: bool,
    winner: Winner,
}

impl Game {
    fn new(h: u16) -> Self {
        let cy = h as f32 / 2.0;
        Self {
            ball: Ball { x: 0.0, y: cy, vx: BALL_SPEED, vy: 5.0 },
            left: Paddle { y: cy, score: 0, ai: true },
            right: Paddle { y: cy, score: 0, ai: true },
            paused: false,
            game_over: false,
            winner: Winner::None,
        }
    }

    #[inline]
    fn reset_ball(&mut self, cx: f32, cy: f32, dir: f32) {
        self.ball.x = cx;
        self.ball.y = cy;
        self.ball.vx = BALL_SPEED * dir;
        self.ball.vy = (rand::thread_rng().gen::<f32>() - 0.5) * 8.0;
    }

    fn reset(&mut self, cx: f32, cy: f32) {
        self.left.y = cy;
        self.right.y = cy;
        self.left.score = 0;
        self.right.score = 0;
        self.game_over = false;
        self.winner = Winner::None;
        self.paused = false;
        self.reset_ball(cx, cy, 1.0);
    }

    #[inline]
    fn move_paddle(paddle_y: &mut f32, dir: f32, speed: f32, dt: f32, min: f32, max: f32) {
        *paddle_y = (*paddle_y + dir * speed * dt).clamp(min, max);
    }

    #[inline]
    fn update_ai(paddle_y: &mut f32, target: f32, dt: f32, min: f32, max: f32) {
        let diff = target - *paddle_y;
        if diff.abs() > 0.5 {
            *paddle_y = (*paddle_y + AI_SPEED * dt * diff.signum()).clamp(min, max);
        }
    }
}

pub fn run(time_step: f32) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let (mut w, mut h) = term.size();
    let mut game = Game::new(h);
    let mut colors = ColorState::new(0);
    let mut show_help = false;

    // Precompute initial values
    let mut cx = w as f32 / 2.0;
    let mut cy = h as f32 / 2.0;
    let mut paddle_x_right = w as i32 - 3;
    let mut paddle_min = PADDLE_HALF + 1.0;
    let mut paddle_max = h as f32 - PADDLE_HALF - 1.0;
    let mut center_x = w as i32 / 2;

    game.ball.x = cx;

    // Input state
    let mut p1_dir: f32 = 0.0;
    let mut p2_dir: f32 = 0.0;

    // Score display buffer
    let mut score_buf = String::with_capacity(16);

    loop {
        // Process all pending input
        while let Ok(Some((code, _))) = term.check_key() {
            if colors.handle_key(code) {
                continue;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char(' ') => {
                    if game.game_over {
                        game.reset(cx, cy);
                    } else {
                        game.paused = !game.paused;
                    }
                }
                KeyCode::Char('r') => game.reset(cx, cy),
                KeyCode::Char('w') | KeyCode::Char('W') => p1_dir = -1.0,
                KeyCode::Char('s') | KeyCode::Char('S') => p1_dir = 1.0,
                KeyCode::Char('1') => game.left.ai = !game.left.ai,
                KeyCode::Up | KeyCode::Char('i') | KeyCode::Char('I') => p2_dir = -1.0,
                KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('K') => p2_dir = 1.0,
                KeyCode::Char('2') => game.right.ai = !game.right.ai,
                KeyCode::Char('?') => show_help = !show_help,
                _ => {}
            }
        }

        // Handle resize
        if let Ok((nw, nh)) = size() {
            if nw != w || nh != h {
                w = nw;
                h = nh;
                term.resize(w, h);
                term.clear_screen()?;
                cx = w as f32 / 2.0;
                cy = h as f32 / 2.0;
                paddle_x_right = w as i32 - 3;
                paddle_min = PADDLE_HALF + 1.0;
                paddle_max = h as f32 - PADDLE_HALF - 1.0;
                center_x = w as i32 / 2;
            }
        }

        // Update game state
        if !game.paused && !game.game_over {
            let dt = time_step;

            // Player input
            if !game.left.ai && p1_dir != 0.0 {
                Game::move_paddle(&mut game.left.y, p1_dir, PADDLE_SPEED, dt, paddle_min, paddle_max);
            }
            if !game.right.ai && p2_dir != 0.0 {
                Game::move_paddle(&mut game.right.y, p2_dir, PADDLE_SPEED, dt, paddle_min, paddle_max);
            }
            p1_dir = 0.0;
            p2_dir = 0.0;

            // AI
            if game.left.ai {
                Game::update_ai(&mut game.left.y, game.ball.y, dt, paddle_min, paddle_max);
            }
            if game.right.ai {
                Game::update_ai(&mut game.right.y, game.ball.y, dt, paddle_min, paddle_max);
            }

            // Move ball
            game.ball.x += game.ball.vx * dt;
            game.ball.y += game.ball.vy * dt;

            // Wall collisions
            let h_bound = h as f32 - 2.0;
            if game.ball.y <= 1.0 {
                game.ball.y = 1.0;
                game.ball.vy = -game.ball.vy;
            } else if game.ball.y >= h_bound {
                game.ball.y = h_bound;
                game.ball.vy = -game.ball.vy;
            }

            // Left paddle collision - symmetric collision zone
            let left_x = PADDLE_X_LEFT as f32;
            if game.ball.x >= left_x - PADDLE_COLLISION_WIDTH && game.ball.x <= left_x + PADDLE_COLLISION_WIDTH {
                let dy = game.ball.y - game.left.y;
                if dy.abs() <= PADDLE_HALF {
                    game.ball.x = left_x + PADDLE_COLLISION_WIDTH + BALL_NUDGE_DISTANCE;
                    game.ball.vx = game.ball.vx.abs() * 1.05;
                    game.ball.vy += (dy / PADDLE_HALF) * SPIN_FACTOR;
                }
            }

            // Right paddle collision - symmetric collision zone
            let right_x = paddle_x_right as f32;
            if game.ball.x >= right_x - PADDLE_COLLISION_WIDTH && game.ball.x <= right_x + PADDLE_COLLISION_WIDTH {
                let dy = game.ball.y - game.right.y;
                if dy.abs() <= PADDLE_HALF {
                    game.ball.x = right_x - PADDLE_COLLISION_WIDTH - BALL_NUDGE_DISTANCE;
                    game.ball.vx = -game.ball.vx.abs() * 1.05;
                    game.ball.vy += (dy / PADDLE_HALF) * SPIN_FACTOR;
                }
            }

            // Clamp velocity
            game.ball.vx = game.ball.vx.clamp(-MAX_VX, MAX_VX);
            game.ball.vy = game.ball.vy.clamp(-MAX_VY, MAX_VY);

            // Scoring
            let w_bound = w as f32 - 1.0;
            if game.ball.x <= 0.0 {
                game.right.score += 1;
                if game.right.score >= WIN_SCORE {
                    game.game_over = true;
                    game.winner = Winner::Right;
                } else {
                    game.reset_ball(cx, cy, 1.0);
                }
            } else if game.ball.x >= w_bound {
                game.left.score += 1;
                if game.left.score >= WIN_SCORE {
                    game.game_over = true;
                    game.winner = Winner::Left;
                } else {
                    game.reset_ball(cx, cy, -1.0);
                }
            }
        }

        // Render
        term.clear();

        // Center line (every other row)
        for y in (0..h).step_by(2) {
            term.set(center_x, y as i32, '│', Some(Color::DarkGrey), false);
        }

        // Colors from scheme
        let (p1_color, p2_color, ball_color) = if colors.is_mono() {
            (Color::Cyan, Color::Magenta, Color::White)
        } else {
            let c2 = scheme_color(colors.scheme, 2, true).0;
            let c3 = scheme_color(colors.scheme, 3, true).0;
            (c2, c3, c3)
        };

        // Score
        score_buf.clear();
        use std::fmt::Write;
        let _ = write!(score_buf, "{}  {}", game.left.score, game.right.score);
        let score_x = center_x - score_buf.len() as i32 / 2;
        term.set_str(score_x, 0, &score_buf, Some(ball_color), true);

        // AI indicators
        let (p1_label, p1_col) = if game.left.ai { ("AI", Color::DarkGrey) } else { ("P1", p1_color) };
        let (p2_label, p2_col) = if game.right.ai { ("AI", Color::DarkGrey) } else { ("P2", p2_color) };
        term.set_str(1, 0, p1_label, Some(p1_col), false);
        term.set_str(w as i32 - 3, 0, p2_label, Some(p2_col), false);

        // Paddles
        let left_col = if game.left.ai { Color::DarkGrey } else { p1_color };
        let right_col = if game.right.ai { Color::DarkGrey } else { p2_color };
        let left_y = game.left.y as i32;
        let right_y = game.right.y as i32;

        for dy in -PADDLE_HEIGHT/2..=PADDLE_HEIGHT/2 {
            let ly = left_y + dy;
            let ry = right_y + dy;
            if ly >= 0 && ly < h as i32 {
                term.set(PADDLE_X_LEFT, ly, '█', Some(left_col), false);
            }
            if ry >= 0 && ry < h as i32 {
                term.set(paddle_x_right, ry, '█', Some(right_col), false);
            }
        }

        // Ball
        let bx = game.ball.x as i32;
        let by = game.ball.y as i32;
        if bx >= 0 && bx < w as i32 && by >= 0 && by < h as i32 {
            term.set(bx, by, '●', Some(ball_color), true);
        }

        // Messages
        let cy_i32 = cy as i32;
        if game.game_over {
            let msg = if game.winner == Winner::Left { MSG_P1_WINS } else { MSG_P2_WINS };
            term.set_str(center_x - msg.len() as i32 / 2, cy_i32, msg, Some(Color::Yellow), true);
            term.set_str(center_x - MSG_RESTART.len() as i32 / 2, cy_i32 + 1, MSG_RESTART, Some(Color::DarkGrey), false);
        } else if game.paused {
            term.set_str(center_x - MSG_PAUSED.len() as i32 / 2, cy_i32, MSG_PAUSED, Some(Color::Yellow), true);
        }

        // Controls hint
        if HINT.len() < w as usize {
            term.set_str(center_x - HINT.len() as i32 / 2, h as i32 - 1, HINT, Some(Color::DarkGrey), false);
        }

        // Help overlay
        if show_help {
            let lines: Vec<&str> = HELP_TEXT.lines().collect();
            let max_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            let box_width = max_width + 4;
            let box_height = lines.len() + 2;
            let start_x = (w as usize).saturating_sub(box_width) / 2;
            let start_y = (h as usize).saturating_sub(box_height) / 2;

            // Top border
            term.set(start_x as i32, start_y as i32, '┌', Some(Color::White), false);
            for x in 1..box_width - 1 {
                term.set((start_x + x) as i32, start_y as i32, '─', Some(Color::White), false);
            }
            term.set((start_x + box_width - 1) as i32, start_y as i32, '┐', Some(Color::White), false);

            // Content rows
            for (i, line) in lines.iter().enumerate() {
                let y = start_y + 1 + i;
                term.set(start_x as i32, y as i32, '│', Some(Color::White), false);
                let padding = max_width.saturating_sub(line.chars().count());
                let padded = format!(" {}{} ", line, " ".repeat(padding));
                for (j, ch) in padded.chars().enumerate() {
                    term.set((start_x + 1 + j) as i32, y as i32, ch, Some(Color::Grey), false);
                }
                term.set((start_x + box_width - 1) as i32, y as i32, '│', Some(Color::White), false);
            }

            // Bottom border
            let bottom_y = start_y + box_height - 1;
            term.set(start_x as i32, bottom_y as i32, '└', Some(Color::White), false);
            for x in 1..box_width - 1 {
                term.set((start_x + x) as i32, bottom_y as i32, '─', Some(Color::White), false);
            }
            term.set((start_x + box_width - 1) as i32, bottom_y as i32, '┘', Some(Color::White), false);
        }

        term.present()?;
        term.sleep(time_step);
    }
}
