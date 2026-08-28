//! Snake - Classic single-player game
//!
//! Controls:
//! - Arrow keys or WASD: steer
//! - Space: pause
//! - R: reset game
//! - Q/Esc: quit

use crate::colors::{scheme_color, ColorState};
use crate::help::render_help_spec;
use crate::terminal::Terminal;
use crossterm::event::KeyCode;
use crossterm::style::Color;
use crossterm::terminal::size;
use rand::Rng;
use std::collections::VecDeque;
use std::io;

const MIN_WIDTH: u16 = 20;
const MIN_HEIGHT: u16 = 10;
const MOVE_INTERVAL: f32 = 0.1;
const MAX_STEPS_PER_FRAME: usize = 8;

const HINT: &str = "Arrows/WASD:steer | Space:pause R:reset ?:help";
const MSG_PAUSED: &str = "PAUSED";
const MSG_GAME_OVER: &str = "GAME OVER";
const MSG_RESTART: &str = "Press R to reset";
const MSG_TOO_SMALL: &str = "Terminal too small";

const HELP: crate::help::HelpSpec = crate::help::HelpSpec::colored(
    "SNAKE",
    &[
        crate::help::HelpEntry::new("Arrows/WASD", "Steer"),
        crate::help::HelpEntry::new("Space", "Pause/resume"),
        crate::help::HelpEntry::new("R", "Reset game"),
    ],
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    fn delta(self) -> (i32, i32) {
        match self {
            Self::Up => (0, -1),
            Self::Down => (0, 1),
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
        }
    }

    fn is_opposite(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Up, Self::Down)
                | (Self::Down, Self::Up)
                | (Self::Left, Self::Right)
                | (Self::Right, Self::Left)
        )
    }
}

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

    fn contains(self, position: Position) -> bool {
        (self.min_x..=self.max_x).contains(&position.x)
            && (self.min_y..=self.max_y).contains(&position.y)
    }

    fn cell_count(self) -> usize {
        ((self.max_x - self.min_x + 1) * (self.max_y - self.min_y + 1)) as usize
    }
}

struct Game {
    board: Board,
    snake: VecDeque<Position>,
    direction: Direction,
    next_direction: Direction,
    food: Option<Position>,
    score: u32,
    paused: bool,
    game_over: bool,
}

impl Game {
    fn new<R: Rng + ?Sized>(board: Board, rng: &mut R) -> Self {
        let head = Position {
            x: (board.min_x + board.max_x) / 2,
            y: (board.min_y + board.max_y) / 2,
        };
        let snake = VecDeque::from([
            head,
            Position {
                x: head.x - 1,
                y: head.y,
            },
            Position {
                x: head.x - 2,
                y: head.y,
            },
        ]);
        let mut game = Self {
            board,
            snake,
            direction: Direction::Right,
            next_direction: Direction::Right,
            food: None,
            score: 0,
            paused: false,
            game_over: false,
        };
        game.food = game.spawn_food(rng);
        game
    }

    fn queue_direction(&mut self, direction: Direction) {
        if self.game_over || self.next_direction != self.direction {
            return;
        }
        if !direction.is_opposite(self.direction) {
            self.next_direction = direction;
        }
    }

    fn step<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        if self.game_over {
            return;
        }

        self.direction = self.next_direction;
        let (dx, dy) = self.direction.delta();
        let Some(head) = self.snake.front().copied() else {
            self.game_over = true;
            return;
        };
        let next = Position {
            x: head.x + dx,
            y: head.y + dy,
        };

        if !self.board.contains(next) {
            self.game_over = true;
            return;
        }

        let eating = self.food == Some(next);
        let tail = self.snake.back().copied();
        let hits_body = self.snake.contains(&next) && (eating || tail != Some(next));
        if hits_body {
            self.game_over = true;
            return;
        }

        self.snake.push_front(next);
        if eating {
            self.score += 1;
            self.food = self.spawn_food(rng);
            if self.food.is_none() {
                self.game_over = true;
            }
        } else {
            self.snake.pop_back();
        }
    }

    fn spawn_food<R: Rng + ?Sized>(&self, rng: &mut R) -> Option<Position> {
        let free_cells = self.board.cell_count().saturating_sub(self.snake.len());
        if free_cells == 0 {
            return None;
        }

        let mut target = rng.gen_range(0..free_cells);
        for y in self.board.min_y..=self.board.max_y {
            for x in self.board.min_x..=self.board.max_x {
                let position = Position { x, y };
                if self.snake.contains(&position) {
                    continue;
                }
                if target == 0 {
                    return Some(position);
                }
                target -= 1;
            }
        }
        None
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

    let (body_color, body_bold) = scheme_color(colors.scheme, 2, true);
    let (head_color, head_bold) = scheme_color(colors.scheme, 3, true);
    let (food_color, food_bold) = if colors.is_mono() {
        (Color::Red, true)
    } else {
        scheme_color(colors.scheme, 1, true)
    };

    if let Some(food) = game.food {
        term.set(food.x, food.y, '◆', Some(food_color), food_bold);
    }
    for (index, segment) in game.snake.iter().enumerate() {
        let (character, color, bold) = if index == 0 {
            ('●', head_color, head_bold)
        } else {
            ('█', body_color, body_bold)
        };
        term.set(segment.x, segment.y, character, Some(color), bold);
    }

    let score = format!("Score: {}", game.score);
    term.set_str(centered_x(width, &score), 0, &score, Some(head_color), true);

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
                KeyCode::Char(' ') => {
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
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('W') => {
                    if let Some(game) = game.as_mut() {
                        game.queue_direction(Direction::Up);
                    }
                }
                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('S') => {
                    if let Some(game) = game.as_mut() {
                        game.queue_direction(Direction::Down);
                    }
                }
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('A') => {
                    if let Some(game) = game.as_mut() {
                        game.queue_direction(Direction::Left);
                    }
                }
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('D') => {
                    if let Some(game) = game.as_mut() {
                        game.queue_direction(Direction::Right);
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
                game = Board::from_terminal(width, height).map(|board| Game::new(board, &mut rng));
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
    use super::{Board, Direction, Game, Position};
    use rand::rngs::mock::StepRng;
    use std::collections::VecDeque;

    fn board() -> Board {
        Board::from_terminal(40, 20).expect("test terminal should fit the game")
    }

    fn rng() -> StepRng {
        StepRng::new(0, 0)
    }

    #[test]
    fn snake_moves_one_cell_without_growing() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        let head = game.snake[0];
        let length = game.snake.len();
        game.food = Some(Position { x: 1, y: 2 });

        game.step(&mut rng);

        assert_eq!(
            game.snake[0],
            Position {
                x: head.x + 1,
                y: head.y
            }
        );
        assert_eq!(game.snake.len(), length);
        assert_eq!(game.score, 0);
    }

    #[test]
    fn eating_food_grows_the_snake_and_scores() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        let head = game.snake[0];
        let length = game.snake.len();
        game.food = Some(Position {
            x: head.x + 1,
            y: head.y,
        });

        game.step(&mut rng);

        assert_eq!(game.snake.len(), length + 1);
        assert_eq!(game.score, 1);
        assert!(game.food.is_some());
        assert!(!game
            .snake
            .contains(&game.food.expect("food should respawn")));
    }

    #[test]
    fn opposite_direction_and_extra_turns_are_rejected() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);

        game.queue_direction(Direction::Left);
        assert_eq!(game.next_direction, Direction::Right);

        game.queue_direction(Direction::Up);
        game.queue_direction(Direction::Left);
        assert_eq!(game.next_direction, Direction::Up);
    }

    #[test]
    fn crossing_a_wall_ends_the_game() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.snake = VecDeque::from([Position {
            x: game.board.max_x,
            y: game.board.min_y,
        }]);
        game.direction = Direction::Right;
        game.next_direction = Direction::Right;

        game.step(&mut rng);

        assert!(game.game_over);
    }

    #[test]
    fn hitting_the_snakes_body_ends_the_game() {
        let mut rng = rng();
        let mut game = Game::new(board(), &mut rng);
        game.snake = VecDeque::from([
            Position { x: 3, y: 3 },
            Position { x: 3, y: 4 },
            Position { x: 2, y: 4 },
            Position { x: 2, y: 3 },
            Position { x: 2, y: 2 },
        ]);
        game.direction = Direction::Left;
        game.next_direction = Direction::Left;
        game.food = Some(Position { x: 10, y: 10 });

        game.step(&mut rng);

        assert!(game.game_over);
    }
}
