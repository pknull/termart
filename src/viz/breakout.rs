//! Breakout - Classic single-player brick-breaking game
//!
//! Controls:
//! - Left/right arrows or A/D: move paddle
//! - Space: pause
//! - R: reset game
//! - Q/Esc: quit

use crate::colors::{scheme_color, ColorState};
use crate::help::render_help_spec;
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use std::io;

const MIN_WIDTH: u16 = 30;
const MIN_HEIGHT: u16 = 16;
const MOVE_INTERVAL: f32 = 0.04;
const MAX_STEPS_PER_FRAME: usize = 8;
const PADDLE_WIDTH: i32 = 9;
const PADDLE_STEP: i32 = 2;
const INITIAL_LIVES: u8 = 3;
const BRICK_ROWS: i32 = 4;
const BRICK_WIDTH: i32 = 5;
const BRICK_GAP: i32 = 1;
const POINTS_PER_BRICK: u32 = 10;

const HINT: &str = "Left/Right or A/D:move | Space:pause R:reset ?:help";
const MSG_PAUSED: &str = "PAUSED";
const MSG_WIN: &str = "YOU WIN";
const MSG_GAME_OVER: &str = "GAME OVER";
const MSG_RESTART: &str = "Press R to reset";
const MSG_TOO_SMALL: &str = "Terminal too small";

const HELP: crate::help::HelpSpec = crate::help::HelpSpec::colored(
    "BREAKOUT",
    &[
        crate::help::HelpEntry::new("Left/Right or A/D", "Move paddle"),
        crate::help::HelpEntry::new("Space", "Pause/resume"),
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

    fn width(self) -> i32 {
        self.max_x - self.min_x + 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaddleDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
struct Ball {
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
}

#[derive(Clone, Debug)]
struct Brick {
    min_x: i32,
    max_x: i32,
    y: i32,
    row: u8,
    alive: bool,
}

impl Brick {
    fn contains(&self, x: f32, y: f32) -> bool {
        let x = x.round() as i32;
        let y = y.round() as i32;
        self.alive && (self.min_x..=self.max_x).contains(&x) && y == self.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GameState {
    Running,
    Won,
    Lost,
}

struct Game {
    board: Board,
    paddle_x: i32,
    paddle_width: i32,
    ball: Ball,
    bricks: Vec<Brick>,
    score: u32,
    lives: u8,
    paused: bool,
    state: GameState,
}

impl Game {
    fn new(board: Board) -> Self {
        let paddle_width = PADDLE_WIDTH.min(board.width());
        let paddle_x = board.min_x + (board.width() - paddle_width) / 2;
        let bricks = Self::build_bricks(board);
        let mut game = Self {
            board,
            paddle_x,
            paddle_width,
            ball: Ball {
                x: 0.0,
                y: 0.0,
                dx: 0.0,
                dy: 0.0,
            },
            bricks,
            score: 0,
            lives: INITIAL_LIVES,
            paused: false,
            state: GameState::Running,
        };
        game.reset_ball();
        game
    }

    fn build_bricks(board: Board) -> Vec<Brick> {
        let columns = ((board.width() + BRICK_GAP) / (BRICK_WIDTH + BRICK_GAP)).max(1);
        let row_width = columns * BRICK_WIDTH + (columns - 1) * BRICK_GAP;
        let start_x = board.min_x + (board.width() - row_width) / 2;
        let mut bricks = Vec::with_capacity((columns * BRICK_ROWS) as usize);

        for row in 0..BRICK_ROWS {
            for column in 0..columns {
                let min_x = start_x + column * (BRICK_WIDTH + BRICK_GAP);
                bricks.push(Brick {
                    min_x,
                    max_x: min_x + BRICK_WIDTH - 1,
                    y: board.min_y + row + 1,
                    row: row as u8,
                    alive: true,
                });
            }
        }
        bricks
    }

    fn paddle_max_x(&self) -> i32 {
        self.board.max_x - self.paddle_width + 1
    }

    fn move_paddle(&mut self, direction: PaddleDirection) {
        if self.state != GameState::Running {
            return;
        }

        let delta = match direction {
            PaddleDirection::Left => -PADDLE_STEP,
            PaddleDirection::Right => PADDLE_STEP,
        };
        self.paddle_x = (self.paddle_x + delta).clamp(self.board.min_x, self.paddle_max_x());
    }

    fn reset_ball(&mut self) {
        self.ball = Ball {
            x: self.paddle_x as f32 + (self.paddle_width - 1) as f32 / 2.0,
            y: self.board.max_y as f32 - 2.0,
            dx: 0.5,
            dy: -0.8,
        };
    }

    fn lose_life(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.state = GameState::Lost;
        } else {
            self.reset_ball();
        }
    }

    fn step(&mut self) {
        if self.state != GameState::Running {
            return;
        }

        let mut next_x = self.ball.x + self.ball.dx;
        let mut next_y = self.ball.y + self.ball.dy;

        if next_x < self.board.min_x as f32 {
            self.ball.dx = self.ball.dx.abs();
            next_x = self.board.min_x as f32;
        } else if next_x > self.board.max_x as f32 {
            self.ball.dx = -self.ball.dx.abs();
            next_x = self.board.max_x as f32;
        }

        if next_y < self.board.min_y as f32 {
            self.ball.dy = self.ball.dy.abs();
            next_y = self.board.min_y as f32;
        }

        let paddle_y = self.board.max_y as f32;
        let paddle_min_x = self.paddle_x as f32;
        let paddle_max_x = (self.paddle_x + self.paddle_width - 1) as f32;
        if self.ball.dy > 0.0
            && self.ball.y < paddle_y
            && next_y >= paddle_y
            && next_x >= paddle_min_x - 0.5
            && next_x <= paddle_max_x + 0.5
        {
            let paddle_center = (paddle_min_x + paddle_max_x) / 2.0;
            let half_width = ((self.paddle_width - 1) as f32 / 2.0).max(1.0);
            let impact = ((next_x - paddle_center) / half_width).clamp(-1.0, 1.0);
            self.ball.dx = impact * 0.9;
            self.ball.dy = -0.8;
            next_y = paddle_y - 1.0;
        }

        if let Some(index) = self
            .bricks
            .iter()
            .position(|brick| brick.contains(next_x, next_y))
        {
            let brick_y = self.bricks[index].y;
            self.bricks[index].alive = false;
            self.score += POINTS_PER_BRICK;

            if self.ball.y.round() as i32 != brick_y {
                self.ball.dy = -self.ball.dy;
                next_y = self.ball.y + self.ball.dy;
            } else {
                self.ball.dx = -self.ball.dx;
                next_x = self.ball.x + self.ball.dx;
            }

            if self.bricks.iter().all(|brick| !brick.alive) {
                self.state = GameState::Won;
            }
        }

        if next_y > self.board.max_y as f32 {
            self.lose_life();
            return;
        }

        self.ball.x = next_x;
        self.ball.y = next_y;
    }
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

fn render_game(term: &mut Terminal, width: u16, height: u16, game: &Game, colors: ColorState) {
    draw_border(term, game.board);

    for brick in game.bricks.iter().filter(|brick| brick.alive) {
        let (color, bold) = scheme_color(colors.scheme, brick.row.min(3), true);
        for x in brick.min_x..=brick.max_x {
            term.set(x, brick.y, '█', Some(color), bold);
        }
    }

    let (paddle_color, paddle_bold) = scheme_color(colors.scheme, 2, true);
    for x in game.paddle_x..game.paddle_x + game.paddle_width {
        term.set(x, game.board.max_y, '▁', Some(paddle_color), paddle_bold);
    }

    let (ball_color, ball_bold) = scheme_color(colors.scheme, 3, true);
    term.set(
        game.ball.x.round() as i32,
        game.ball.y.round() as i32,
        '●',
        Some(ball_color),
        ball_bold,
    );

    let status = format!("Score: {}  Lives: {}", game.score, game.lives);
    term.set_str(
        centered_x(width, &status),
        0,
        &status,
        Some(ball_color),
        true,
    );

    let center_y = height as i32 / 2;
    let message = match game.state {
        GameState::Won => Some(MSG_WIN),
        GameState::Lost => Some(MSG_GAME_OVER),
        GameState::Running if game.paused => Some(MSG_PAUSED),
        GameState::Running => None,
    };
    if let Some(message) = message {
        term.set_str(
            centered_x(width, message),
            center_y,
            message,
            Some(Color::Yellow),
            true,
        );
    }
    if game.state != GameState::Running {
        term.set_str(
            centered_x(width, MSG_RESTART),
            center_y + 1,
            MSG_RESTART,
            Some(Color::DarkGrey),
            false,
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
    let mut game = Board::from_terminal(width, height).map(Game::new);
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
                KeyCode::Char(' ') => {
                    if let Some(game) = game.as_mut() {
                        if game.state == GameState::Running {
                            game.paused = !game.paused;
                            move_accumulator = 0.0;
                        }
                    }
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    game = Board::from_terminal(width, height).map(Game::new);
                    move_accumulator = 0.0;
                }
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(game) = game.as_mut() {
                        game.move_paddle(PaddleDirection::Left);
                    }
                }
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(game) = game.as_mut() {
                        game.move_paddle(PaddleDirection::Right);
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
                game = Board::from_terminal(width, height).map(Game::new);
                move_accumulator = 0.0;
            }
        }

        if let Some(game) = game.as_mut() {
            if !game.paused && game.state == GameState::Running {
                move_accumulator += time_step;
                let mut steps = 0;
                while move_accumulator >= MOVE_INTERVAL && steps < MAX_STEPS_PER_FRAME {
                    game.step();
                    move_accumulator -= MOVE_INTERVAL;
                    steps += 1;
                    if game.state != GameState::Running {
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
    use super::{Ball, Board, Brick, Game, GameState, PaddleDirection, POINTS_PER_BRICK};

    fn board() -> Board {
        Board::from_terminal(40, 20).expect("test terminal should fit the game")
    }

    fn brick(min_x: i32, y: i32) -> Brick {
        Brick {
            min_x,
            max_x: min_x + 4,
            y,
            row: 0,
            alive: true,
        }
    }

    #[test]
    fn paddle_movement_stays_inside_board() {
        let mut game = Game::new(board());

        for _ in 0..100 {
            game.move_paddle(PaddleDirection::Left);
        }
        assert_eq!(game.paddle_x, game.board.min_x);

        for _ in 0..100 {
            game.move_paddle(PaddleDirection::Right);
        }
        assert_eq!(game.paddle_x, game.paddle_max_x());
    }

    #[test]
    fn ball_bounces_off_side_and_top_walls() {
        let mut game = Game::new(board());
        game.ball = Ball {
            x: game.board.min_x as f32 + 0.2,
            y: 12.0,
            dx: -0.8,
            dy: 0.2,
        };
        game.step();
        assert!(game.ball.dx > 0.0);
        assert_eq!(game.ball.x, game.board.min_x as f32);

        game.ball = Ball {
            x: 20.0,
            y: game.board.min_y as f32 + 0.2,
            dx: 0.2,
            dy: -0.8,
        };
        game.step();
        assert!(game.ball.dy > 0.0);
        assert_eq!(game.ball.y, game.board.min_y as f32);
    }

    #[test]
    fn paddle_bounce_uses_impact_position_for_angle() {
        let mut game = Game::new(board());
        let paddle_right = game.paddle_x + game.paddle_width - 1;
        game.ball = Ball {
            x: paddle_right as f32,
            y: game.board.max_y as f32 - 0.4,
            dx: 0.0,
            dy: 0.8,
        };

        game.step();

        assert!(game.ball.dy < 0.0);
        assert!(game.ball.dx > 0.0);
        assert!(game.ball.y < game.board.max_y as f32);
    }

    #[test]
    fn brick_hit_destroys_brick_and_adds_score() {
        let mut game = Game::new(board());
        game.bricks = vec![brick(10, 5), brick(20, 5)];
        game.ball = Ball {
            x: 12.0,
            y: 6.0,
            dx: 0.0,
            dy: -1.0,
        };

        game.step();

        assert!(!game.bricks[0].alive);
        assert!(game.bricks[1].alive);
        assert_eq!(game.score, POINTS_PER_BRICK);
        assert_eq!(game.state, GameState::Running);
        assert!(game.ball.dy > 0.0);
    }

    #[test]
    fn missing_paddle_costs_a_life_and_resets_ball() {
        let mut game = Game::new(board());
        let initial_lives = game.lives;
        game.ball = Ball {
            x: game.board.min_x as f32,
            y: game.board.max_y as f32,
            dx: 0.0,
            dy: 1.0,
        };

        game.step();

        assert_eq!(game.lives, initial_lives - 1);
        assert_eq!(game.state, GameState::Running);
        assert!(game.ball.y < game.board.max_y as f32);
    }

    #[test]
    fn clearing_last_brick_wins_game() {
        let mut game = Game::new(board());
        game.bricks = vec![brick(10, 5)];
        game.ball = Ball {
            x: 12.0,
            y: 6.0,
            dx: 0.0,
            dy: -1.0,
        };

        game.step();

        assert_eq!(game.state, GameState::Won);
        assert_eq!(game.score, POINTS_PER_BRICK);
    }

    #[test]
    fn losing_last_life_ends_game() {
        let mut game = Game::new(board());
        game.lives = 1;
        game.ball = Ball {
            x: game.board.min_x as f32,
            y: game.board.max_y as f32,
            dx: 0.0,
            dy: 1.0,
        };

        game.step();

        assert_eq!(game.lives, 0);
        assert_eq!(game.state, GameState::Lost);
    }
}
