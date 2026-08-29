//! Asteroids - Classic single-player game
//!
//! Controls:
//! - Left/Right arrows or A/D: rotate
//! - Up arrow or W: thrust
//! - Space: fire
//! - P: pause
//! - R: reset game
//! - Q/Esc: quit

use crate::colors::{scheme_color, ColorState};
use crate::help::render_help_spec;
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use rand::Rng;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::io;

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 14;
const MOVE_INTERVAL: f32 = 0.03;
const MAX_STEPS_PER_FRAME: usize = 8;

const ROTATE_STEP: f32 = PI / 12.0;
const THRUST_ACCEL: f32 = 0.06;
const FRICTION: f32 = 0.985;
const MAX_SPEED: f32 = 0.9;
const SHIP_RADIUS: f32 = 1.0;
const SHIP_NOSE: f32 = 1.5;
const BULLET_SPEED: f32 = 1.1;
const BULLET_LIFETIME: u32 = 45;
const BULLET_PADDING: f32 = 0.4;
const MAX_BULLETS: usize = 4;
const INITIAL_LIVES: u8 = 3;
const INVULN_TICKS: u32 = 66;
const WAVE_BASE_COUNT: usize = 3;
const MAX_WAVE_ASTEROIDS: usize = 11;
const WAVE_SPEED_STEP: f32 = 0.08;
const MAX_WAVE_SPEED_FACTOR: f32 = 1.6;
const SPEED_JITTER_MIN: f32 = 0.8;
const SPEED_JITTER_MAX: f32 = 1.2;
const SAFE_SPAWN_DIST: f32 = 14.0;
const SPAWN_ATTEMPTS: usize = 24;

const HINT: &str = "Left/Right:rotate Up:thrust Space:fire | P:pause R:reset ?:help";
const MSG_PAUSED: &str = "PAUSED";
const MSG_GAME_OVER: &str = "GAME OVER";
const MSG_RESTART: &str = "Press R to reset";
const MSG_TOO_SMALL: &str = "Terminal too small";

const HELP: crate::help::HelpSpec = crate::help::HelpSpec::colored(
    "ASTEROIDS",
    &[
        crate::help::HelpEntry::new("Left/Right or A/D", "Rotate ship"),
        crate::help::HelpEntry::new("Up or W", "Thrust"),
        crate::help::HelpEntry::new("Space", "Fire"),
        crate::help::HelpEntry::new("P", "Pause/resume"),
        crate::help::HelpEntry::new("R", "Reset game"),
    ],
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Board {
    min_x: i32,
    max_x: i32,
    min_y: i32,
    max_y: i32,
}

impl Board {
    fn from_terminal(width: u16, height: u16) -> Option<Self> {
        if width < MIN_WIDTH || height < MIN_HEIGHT {
            return None;
        }

        Some(Self {
            min_x: 1,
            max_x: width as i32 - 2,
            min_y: 2,
            max_y: height as i32 - 3,
        })
    }

    fn field_width(self) -> f32 {
        (self.max_x - self.min_x + 1) as f32
    }

    fn field_height(self) -> f32 {
        (self.max_y - self.min_y + 1) as f32
    }

    fn center(self) -> (f32, f32) {
        (
            (self.min_x + self.max_x) as f32 / 2.0,
            (self.min_y + self.max_y) as f32 / 2.0,
        )
    }

    /// Wrap a float position toroidally into the field. The wrapped range is
    /// offset by half a cell so rounding always lands on a cell inside the
    /// board.
    fn wrap(self, x: f32, y: f32) -> (f32, f32) {
        (
            wrap_coord(x, self.min_x, self.field_width()),
            wrap_coord(y, self.min_y, self.field_height()),
        )
    }

    /// Shortest toroidal separation squared, measured in column units. Rows
    /// count double because terminal cells are roughly twice as tall as they
    /// are wide.
    fn separation_squared(self, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        let dx = wrapped_delta(ax, bx, self.field_width());
        let dy = wrapped_delta(ay, by, self.field_height()) * 2.0;
        dx * dx + dy * dy
    }
}

fn wrap_coord(value: f32, min: i32, span: f32) -> f32 {
    let origin = min as f32 - 0.5;
    (value - origin).rem_euclid(span) + origin
}

fn wrapped_delta(a: f32, b: f32, span: f32) -> f32 {
    let half = span / 2.0;
    (a - b + half).rem_euclid(span) - half
}

/// Per-step displacement for one column unit along a math-convention heading
/// (0 = right, pi/2 = up on screen). Rows move at half rate to compensate for
/// the terminal cell aspect ratio.
fn heading_vector(heading: f32) -> (f32, f32) {
    (heading.cos(), -heading.sin() * 0.5)
}

fn wave_speed_factor(wave: u32) -> f32 {
    (1.0 + WAVE_SPEED_STEP * wave.saturating_sub(1) as f32).min(MAX_WAVE_SPEED_FACTOR)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotation {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AsteroidSize {
    Large,
    Medium,
    Small,
}

impl AsteroidSize {
    fn radius(self) -> f32 {
        match self {
            Self::Large => 2.2,
            Self::Medium => 1.4,
            Self::Small => 0.8,
        }
    }

    fn score(self) -> u32 {
        match self {
            Self::Large => 20,
            Self::Medium => 50,
            Self::Small => 100,
        }
    }

    fn base_speed(self) -> f32 {
        match self {
            Self::Large => 0.15,
            Self::Medium => 0.22,
            Self::Small => 0.30,
        }
    }

    fn split(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Ship {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    heading: f32,
}

impl Ship {
    fn spawn(board: Board) -> Self {
        let (x, y) = board.center();
        Self {
            x,
            y,
            dx: 0.0,
            dy: 0.0,
            heading: FRAC_PI_2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Asteroid {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    size: AsteroidSize,
}

#[derive(Clone, Copy, Debug)]
struct Bullet {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    ticks_left: u32,
}

struct Game {
    board: Board,
    ship: Ship,
    asteroids: Vec<Asteroid>,
    bullets: Vec<Bullet>,
    score: u32,
    lives: u8,
    wave: u32,
    invuln_ticks: u32,
    paused: bool,
    game_over: bool,
}

impl Game {
    fn new<R: Rng + ?Sized>(board: Board, rng: &mut R) -> Self {
        let mut game = Self {
            board,
            ship: Ship::spawn(board),
            asteroids: Vec::new(),
            bullets: Vec::new(),
            score: 0,
            lives: INITIAL_LIVES,
            wave: 1,
            invuln_ticks: 0,
            paused: false,
            game_over: false,
        };
        game.spawn_wave(rng);
        game
    }

    fn rotate(&mut self, rotation: Rotation) {
        if self.paused || self.game_over {
            return;
        }
        let step = match rotation {
            Rotation::Left => ROTATE_STEP,
            Rotation::Right => -ROTATE_STEP,
        };
        self.ship.heading = (self.ship.heading + step).rem_euclid(TAU);
    }

    fn thrust(&mut self) {
        if self.paused || self.game_over {
            return;
        }
        let (hx, hy) = heading_vector(self.ship.heading);
        self.ship.dx += hx * THRUST_ACCEL;
        self.ship.dy += hy * THRUST_ACCEL;

        let speed = (self.ship.dx * self.ship.dx + (2.0 * self.ship.dy).powi(2)).sqrt();
        if speed > MAX_SPEED {
            let scale = MAX_SPEED / speed;
            self.ship.dx *= scale;
            self.ship.dy *= scale;
        }
    }

    fn fire(&mut self) {
        if self.paused || self.game_over || self.bullets.len() >= MAX_BULLETS {
            return;
        }
        let (hx, hy) = heading_vector(self.ship.heading);
        let (x, y) = self
            .board
            .wrap(self.ship.x + hx * SHIP_NOSE, self.ship.y + hy * SHIP_NOSE);
        self.bullets.push(Bullet {
            x,
            y,
            dx: self.ship.dx + hx * BULLET_SPEED,
            dy: self.ship.dy + hy * BULLET_SPEED,
            ticks_left: BULLET_LIFETIME,
        });
    }

    fn step<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        if self.game_over {
            return;
        }
        if self.invuln_ticks > 0 {
            self.invuln_ticks -= 1;
        }

        let board = self.board;

        self.ship.dx *= FRICTION;
        self.ship.dy *= FRICTION;
        let (x, y) = board.wrap(self.ship.x + self.ship.dx, self.ship.y + self.ship.dy);
        self.ship.x = x;
        self.ship.y = y;

        for asteroid in &mut self.asteroids {
            let (x, y) = board.wrap(asteroid.x + asteroid.dx, asteroid.y + asteroid.dy);
            asteroid.x = x;
            asteroid.y = y;
        }

        for bullet in &mut self.bullets {
            let (x, y) = board.wrap(bullet.x + bullet.dx, bullet.y + bullet.dy);
            bullet.x = x;
            bullet.y = y;
            bullet.ticks_left = bullet.ticks_left.saturating_sub(1);
        }
        self.bullets.retain(|bullet| bullet.ticks_left > 0);

        let mut bullet_index = 0;
        while bullet_index < self.bullets.len() {
            let bullet = self.bullets[bullet_index];
            let hit = self.asteroids.iter().position(|asteroid| {
                board.separation_squared(bullet.x, bullet.y, asteroid.x, asteroid.y)
                    <= (asteroid.size.radius() + BULLET_PADDING).powi(2)
            });
            if let Some(index) = hit {
                let asteroid = self.asteroids.swap_remove(index);
                self.score += asteroid.size.score();
                self.split_asteroid(asteroid, rng);
                self.bullets.swap_remove(bullet_index);
            } else {
                bullet_index += 1;
            }
        }

        if self.invuln_ticks == 0 {
            let ship_hit = self.asteroids.iter().any(|asteroid| {
                board.separation_squared(self.ship.x, self.ship.y, asteroid.x, asteroid.y)
                    <= (asteroid.size.radius() + SHIP_RADIUS).powi(2)
            });
            if ship_hit {
                self.destroy_ship();
                if self.game_over {
                    return;
                }
            }
        }

        if self.asteroids.is_empty() {
            self.wave += 1;
            self.spawn_wave(rng);
        }
    }

    fn split_asteroid<R: Rng + ?Sized>(&mut self, parent: Asteroid, rng: &mut R) {
        let Some(child_size) = parent.size.split() else {
            return;
        };
        for _ in 0..2 {
            let (dx, dy) = self.random_velocity(child_size, rng);
            self.asteroids.push(Asteroid {
                x: parent.x,
                y: parent.y,
                dx,
                dy,
                size: child_size,
            });
        }
    }

    fn destroy_ship(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.game_over = true;
            return;
        }
        self.ship = Ship::spawn(self.board);
        self.invuln_ticks = INVULN_TICKS;
    }

    fn spawn_wave<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        let count = (WAVE_BASE_COUNT + self.wave as usize).min(MAX_WAVE_ASTEROIDS);
        for _ in 0..count {
            let (x, y) = self.spawn_position(rng);
            let (dx, dy) = self.random_velocity(AsteroidSize::Large, rng);
            self.asteroids.push(Asteroid {
                x,
                y,
                dx,
                dy,
                size: AsteroidSize::Large,
            });
        }
    }

    fn random_velocity<R: Rng + ?Sized>(&self, size: AsteroidSize, rng: &mut R) -> (f32, f32) {
        let angle = rng.gen_range(0.0..TAU);
        let jitter = rng.gen_range(SPEED_JITTER_MIN..SPEED_JITTER_MAX);
        let speed = size.base_speed() * wave_speed_factor(self.wave) * jitter;
        let (dx, dy) = heading_vector(angle);
        (dx * speed, dy * speed)
    }

    /// Pick a wave spawn position away from the ship, falling back to the
    /// farthest attempt when the field is too cramped to satisfy the safe
    /// distance.
    fn spawn_position<R: Rng + ?Sized>(&self, rng: &mut R) -> (f32, f32) {
        let safe = self.safe_spawn_distance();
        let mut best = self.board.center();
        let mut best_separation = -1.0;
        for _ in 0..SPAWN_ATTEMPTS {
            let x = rng.gen_range(self.board.min_x as f32..self.board.max_x as f32 + 1.0);
            let y = rng.gen_range(self.board.min_y as f32..self.board.max_y as f32 + 1.0);
            let separation = self
                .board
                .separation_squared(x, y, self.ship.x, self.ship.y)
                .sqrt();
            if separation >= safe {
                return (x, y);
            }
            if separation > best_separation {
                best_separation = separation;
                best = (x, y);
            }
        }
        best
    }

    fn safe_spawn_distance(&self) -> f32 {
        let width_limit = self.board.field_width() / 2.0 - 1.0;
        let height_limit = self.board.field_height() - 1.0;
        SAFE_SPAWN_DIST.min(width_limit).min(height_limit).max(0.0)
    }

    /// Re-fit the current game to a new terminal size, wrapping every object
    /// into the new field instead of restarting the game.
    fn apply_board(&mut self, board: Board) {
        self.board = board;
        let (x, y) = board.wrap(self.ship.x, self.ship.y);
        self.ship.x = x;
        self.ship.y = y;
        for asteroid in &mut self.asteroids {
            let (x, y) = board.wrap(asteroid.x, asteroid.y);
            asteroid.x = x;
            asteroid.y = y;
        }
        for bullet in &mut self.bullets {
            let (x, y) = board.wrap(bullet.x, bullet.y);
            bullet.x = x;
            bullet.y = y;
        }
    }
}

fn ship_glyph(heading: f32) -> char {
    const GLYPHS: [char; 8] = ['→', '↗', '↑', '↖', '←', '↙', '↓', '↘'];
    let octant = (heading.rem_euclid(TAU) / (TAU / 8.0)).round() as usize % 8;
    GLYPHS[octant]
}

fn centered_x(width: u16, text: &str) -> i32 {
    ((width as i32 - text.chars().count() as i32) / 2).max(0)
}

fn draw_border(term: &mut Terminal, board: Board) {
    let left = board.min_x - 1;
    let right = board.max_x + 1;
    let top = board.min_y - 1;
    let bottom = board.max_y + 1;

    term.set(left, top, '┌', Some(Color::DarkGrey), false);
    term.set(right, top, '┐', Some(Color::DarkGrey), false);
    term.set(left, bottom, '└', Some(Color::DarkGrey), false);
    term.set(right, bottom, '┘', Some(Color::DarkGrey), false);
    for x in board.min_x..=board.max_x {
        term.set(x, top, '─', Some(Color::DarkGrey), false);
        term.set(x, bottom, '─', Some(Color::DarkGrey), false);
    }
    for y in board.min_y..=board.max_y {
        term.set(left, y, '│', Some(Color::DarkGrey), false);
        term.set(right, y, '│', Some(Color::DarkGrey), false);
    }
}

/// Draw one cell, wrapping integer coordinates toroidally into the board so
/// glyph clusters straddling an edge reappear on the opposite side.
fn set_wrapped(
    term: &mut Terminal,
    board: Board,
    x: i32,
    y: i32,
    ch: char,
    color: Option<Color>,
    bold: bool,
) {
    let width = board.max_x - board.min_x + 1;
    let height = board.max_y - board.min_y + 1;
    let x = board.min_x + (x - board.min_x).rem_euclid(width);
    let y = board.min_y + (y - board.min_y).rem_euclid(height);
    term.set(x, y, ch, color, bold);
}

fn render_game(term: &mut Terminal, width: u16, height: u16, game: &Game, colors: ColorState) {
    draw_border(term, game.board);

    let (asteroid_color, asteroid_bold) = scheme_color(colors.scheme, 1, true);
    let (bullet_color, bullet_bold) = scheme_color(colors.scheme, 2, true);
    let (ship_color, ship_bold) = scheme_color(colors.scheme, 3, true);

    for asteroid in &game.asteroids {
        let x = asteroid.x.round() as i32;
        let y = asteroid.y.round() as i32;
        match asteroid.size {
            AsteroidSize::Large => {
                set_wrapped(term, game.board, x - 1, y, '(', Some(asteroid_color), false);
                set_wrapped(
                    term,
                    game.board,
                    x,
                    y,
                    'O',
                    Some(asteroid_color),
                    asteroid_bold,
                );
                set_wrapped(term, game.board, x + 1, y, ')', Some(asteroid_color), false);
            }
            AsteroidSize::Medium => set_wrapped(
                term,
                game.board,
                x,
                y,
                'O',
                Some(asteroid_color),
                asteroid_bold,
            ),
            AsteroidSize::Small => set_wrapped(
                term,
                game.board,
                x,
                y,
                'o',
                Some(asteroid_color),
                asteroid_bold,
            ),
        }
    }

    for bullet in &game.bullets {
        set_wrapped(
            term,
            game.board,
            bullet.x.round() as i32,
            bullet.y.round() as i32,
            '·',
            Some(bullet_color),
            bullet_bold,
        );
    }

    let ship_visible = !game.game_over && (game.invuln_ticks == 0 || game.invuln_ticks % 8 < 4);
    if ship_visible {
        set_wrapped(
            term,
            game.board,
            game.ship.x.round() as i32,
            game.ship.y.round() as i32,
            ship_glyph(game.ship.heading),
            Some(ship_color),
            ship_bold,
        );
    }

    let status = format!(
        "Score: {}  Lives: {}  Wave: {}",
        game.score, game.lives, game.wave
    );
    term.set_str(
        centered_x(width, &status),
        0,
        &status,
        Some(ship_color),
        true,
    );

    let center_y = height as i32 / 2;
    if game.game_over {
        term.set_str(
            centered_x(width, MSG_GAME_OVER),
            center_y,
            MSG_GAME_OVER,
            Some(Color::Yellow),
            true,
        );
        term.set_str(
            centered_x(width, MSG_RESTART),
            center_y + 1,
            MSG_RESTART,
            Some(Color::DarkGrey),
            false,
        );
    } else if game.paused {
        term.set_str(
            centered_x(width, MSG_PAUSED),
            center_y,
            MSG_PAUSED,
            Some(Color::Yellow),
            true,
        );
    }

    if HINT.chars().count() < width as usize {
        term.set_str(
            centered_x(width, HINT),
            height as i32 - 1,
            HINT,
            Some(Color::DarkGrey),
            false,
        );
    }
}

pub fn run(time_step: f32) -> io::Result<()> {
    let mut term = Terminal::new(true)?;
    let (mut width, mut height) = term.size();
    let mut rng = rand::thread_rng();
    let mut game = Board::from_terminal(width, height).map(|board| Game::new(board, &mut rng));
    let mut colors = ColorState::new(0);
    let mut show_help = false;
    let mut move_accumulator = 0.0;

    loop {
        while let Ok(Some((code, modifiers))) = term.check_key() {
            if colors.handle_key(code, modifiers) {
                continue;
            }
            match code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('p') | KeyCode::Char('P') => {
                    if let Some(game) = game.as_mut() {
                        if !game.game_over {
                            game.paused = !game.paused;
                            move_accumulator = 0.0;
                        }
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    game =
                        Board::from_terminal(width, height).map(|board| Game::new(board, &mut rng));
                    move_accumulator = 0.0;
                }
                KeyCode::Char(' ') => {
                    if let Some(game) = game.as_mut() {
                        game.fire();
                    }
                }
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(game) = game.as_mut() {
                        game.rotate(Rotation::Left);
                    }
                }
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(game) = game.as_mut() {
                        game.rotate(Rotation::Right);
                    }
                }
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                    if let Some(game) = game.as_mut() {
                        game.thrust();
                    }
                }
                KeyCode::Char('?') => show_help = !show_help,
                _ => {}
            }
        }

        if let Ok((new_width, new_height)) = size() {
            if new_width != width || new_height != height {
                width = new_width;
                height = new_height;
                term.resize(width, height);
                term.clear_screen()?;
                game = match (Board::from_terminal(width, height), game.take()) {
                    (Some(board), Some(mut active)) => {
                        active.apply_board(board);
                        Some(active)
                    }
                    (Some(board), None) => Some(Game::new(board, &mut rng)),
                    (None, _) => None,
                };
                move_accumulator = 0.0;
            }
        }

        if let Some(game) = game.as_mut() {
            if !game.paused && !game.game_over {
                move_accumulator += time_step;
                let mut steps = 0;
                while move_accumulator >= MOVE_INTERVAL && steps < MAX_STEPS_PER_FRAME {
                    game.step(&mut rng);
                    move_accumulator -= MOVE_INTERVAL;
                    steps += 1;
                    if game.game_over {
                        break;
                    }
                }
                if steps == MAX_STEPS_PER_FRAME {
                    move_accumulator = 0.0;
                }
            }
        } else {
            move_accumulator = 0.0;
        }

        term.clear();
        if let Some(game) = game.as_ref() {
            render_game(&mut term, width, height, game, colors);
        } else {
            term.set_str(
                centered_x(width, MSG_TOO_SMALL),
                height as i32 / 2,
                MSG_TOO_SMALL,
                Some(Color::Yellow),
                true,
            );
        }

        if show_help {
            render_help_spec(&mut term, width, height, &HELP);
        }

        term.present()?;
        term.sleep(time_step);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Asteroid, AsteroidSize, Board, Bullet, Game, Rotation, INVULN_TICKS, MAX_BULLETS,
        MAX_WAVE_ASTEROIDS, WAVE_BASE_COUNT,
    };
    use rand::rngs::mock::StepRng;

    fn board() -> Board {
        Board::from_terminal(80, 24).expect("test terminal should fit the game")
    }

    fn rng() -> StepRng {
        StepRng::new(0, 0)
    }

    fn asteroid(x: f32, y: f32, dx: f32, dy: f32, size: AsteroidSize) -> Asteroid {
        Asteroid { x, y, dx, dy, size }
    }

    fn bullet(x: f32, y: f32, ticks_left: u32) -> Bullet {
        Bullet {
            x,
            y,
            dx: 0.0,
            dy: 0.0,
            ticks_left,
        }
    }

    fn speed(dx: f32, dy: f32) -> f32 {
        (dx * dx + (2.0 * dy).powi(2)).sqrt()
    }

    #[test]
    fn thrust_adds_velocity_and_friction_decays_it() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();

        game.thrust();
        assert!(game.ship.dy < 0.0, "thrust along the up heading rises");

        let before = speed(game.ship.dx, game.ship.dy);
        game.step(&mut rng);
        game.asteroids.clear();
        let after = speed(game.ship.dx, game.ship.dy);
        assert!(after < before, "friction decays speed");
        assert!(after > 0.0, "inertia keeps the ship drifting");
    }

    #[test]
    fn ship_wraps_across_the_field_edge() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.ship.x = game.board.max_x as f32;
        game.ship.dx = 1.0;

        game.step(&mut rng);

        assert!(game.ship.x < game.board.min_x as f32 + 1.0);
    }

    #[test]
    fn bullets_are_capped_and_expire() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();

        for _ in 0..MAX_BULLETS + 2 {
            game.fire();
        }
        assert_eq!(game.bullets.len(), MAX_BULLETS);

        game.paused = true;
        game.fire();
        assert_eq!(game.bullets.len(), MAX_BULLETS);
        game.paused = false;

        game.bullets.clear();
        game.bullets.push(bullet(10.0, 5.0, 1));
        game.bullets.push(bullet(20.0, 5.0, 5));
        game.step(&mut rng);
        game.asteroids.clear();
        assert_eq!(game.bullets.len(), 1);
    }

    #[test]
    fn shooting_large_asteroid_splits_into_two_medium() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.asteroids
            .push(asteroid(10.0, 5.0, 0.0, 0.0, AsteroidSize::Large));
        game.bullets.push(bullet(10.0, 5.0, 10));

        game.step(&mut rng);

        assert_eq!(game.asteroids.len(), 2);
        assert!(game
            .asteroids
            .iter()
            .all(|a| a.size == AsteroidSize::Medium));
        assert_eq!(game.score, 20);
        assert!(game.bullets.is_empty());
    }

    #[test]
    fn shooting_medium_spawns_two_faster_small() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        let parent = asteroid(10.0, 5.0, 0.1, 0.0, AsteroidSize::Medium);
        game.asteroids.push(parent);
        game.bullets.push(bullet(10.0, 5.0, 10));

        game.step(&mut rng);

        assert_eq!(game.asteroids.len(), 2);
        for child in &game.asteroids {
            assert_eq!(child.size, AsteroidSize::Small);
            assert!(speed(child.dx, child.dy) > speed(parent.dx, parent.dy));
        }
        assert_eq!(game.score, 50);
    }

    #[test]
    fn shooting_small_destroys_it_and_scores_most() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.asteroids
            .push(asteroid(10.0, 5.0, 0.0, 0.0, AsteroidSize::Small));
        game.asteroids
            .push(asteroid(30.0, 8.0, 0.0, 0.0, AsteroidSize::Large));
        game.bullets.push(bullet(10.0, 5.0, 10));

        game.step(&mut rng);

        assert_eq!(game.asteroids.len(), 1);
        assert_eq!(game.asteroids[0].size, AsteroidSize::Large);
        assert_eq!(game.score, 100);
    }

    #[test]
    fn ship_collision_costs_a_life_and_grants_spawn_safety() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.asteroids.push(asteroid(
            game.ship.x,
            game.ship.y,
            0.0,
            0.0,
            AsteroidSize::Small,
        ));

        game.step(&mut rng);

        assert_eq!(game.lives, 2);
        assert_eq!(game.invuln_ticks, INVULN_TICKS);
        assert!(!game.game_over);

        game.step(&mut rng);
        assert_eq!(game.lives, 2, "spawn safety ignores the overlap");
    }

    #[test]
    fn losing_last_life_ends_the_game() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.lives = 1;
        game.asteroids.push(asteroid(
            game.ship.x,
            game.ship.y,
            0.0,
            0.0,
            AsteroidSize::Small,
        ));

        game.step(&mut rng);

        assert_eq!(game.lives, 0);
        assert!(game.game_over);
    }

    #[test]
    fn clearing_the_field_spawns_a_larger_wave() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        assert_eq!(game.asteroids.len(), WAVE_BASE_COUNT + 1);
        game.asteroids.clear();

        game.step(&mut rng);

        assert_eq!(game.wave, 2);
        assert_eq!(
            game.asteroids.len(),
            (WAVE_BASE_COUNT + 2).min(MAX_WAVE_ASTEROIDS)
        );
        assert!(game.asteroids.iter().all(|a| a.size == AsteroidSize::Large));
    }

    #[test]
    fn resize_wraps_objects_into_the_new_field() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.asteroids.clear();
        game.asteroids
            .push(asteroid(77.0, 20.0, 0.0, 0.0, AsteroidSize::Large));
        game.bullets.push(bullet(77.0, 20.0, 10));

        let smaller = Board::from_terminal(50, 18).expect("smaller terminal should fit the game");
        game.apply_board(smaller);

        let in_bounds = |x: f32, y: f32| {
            x >= smaller.min_x as f32 - 0.5
                && x < smaller.max_x as f32 + 0.5
                && y >= smaller.min_y as f32 - 0.5
                && y < smaller.max_y as f32 + 0.5
        };
        assert!(in_bounds(game.ship.x, game.ship.y));
        assert!(in_bounds(game.asteroids[0].x, game.asteroids[0].y));
        assert!(in_bounds(game.bullets[0].x, game.bullets[0].y));
    }

    #[test]
    fn rotation_ignored_while_paused() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        let heading = game.ship.heading;

        game.paused = true;
        game.rotate(Rotation::Left);
        assert_eq!(game.ship.heading, heading);

        game.paused = false;
        game.rotate(Rotation::Left);
        assert!(game.ship.heading > heading);
    }
}
