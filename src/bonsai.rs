use crate::config::{BonsaiConfig, BranchType, Counters};
use crate::help::show_help_modal;
use crate::terminal::{colors, Terminal};
use crossterm::event::KeyCode;
use crossterm::style::Color;
use rand::prelude::*;
use std::io;

const HELP: &str = "\
BONSAI
─────────────────
q/Esc  Quit
?      Close help";

/// A branch segment to be processed
struct BranchTask {
    x: i32,
    y: i32,
    branch_type: BranchType,
    life: i32,
    shoot_cooldown: i32,
}

/// Run the bonsai tree generator
pub fn run(config: BonsaiConfig) -> io::Result<()> {
    let seed = config.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) // Fallback seed for misconfigured system clocks
    });

    if config.print {
        run_print_mode(&config, seed)?;
    } else {
        run_interactive(&config, seed)?;
    }

    Ok(())
}

fn run_print_mode(config: &BonsaiConfig, initial_seed: u64) -> io::Result<()> {
    let mut seed = initial_seed;

    loop {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut term = Terminal::new(false)?;
        let mut counters = Counters::default();

        let (width, height) = term.size();
        let start_x = width as i32 / 2;
        let start_y = height as i32 - get_base_height(config.base_type) - 1;

        // Draw base
        draw_base(&mut term, config.base_type);

        // Grow tree (not live)
        grow_tree(&mut term, config, &mut counters, &mut rng, start_x, start_y, false)?;

        // Print to stdout
        term.print_to_stdout();

        if !config.infinite {
            break;
        }

        std::thread::sleep(std::time::Duration::from_secs_f64(config.time_wait));
        seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    Ok(())
}

fn run_interactive(config: &BonsaiConfig, initial_seed: u64) -> io::Result<()> {
    let mut seed = initial_seed;

    loop {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut term = Terminal::new(true)?;
        let mut counters = Counters::default();

        term.clear_screen()?;

        let (width, height) = term.size();
        let start_x = width as i32 / 2;
        let start_y = height as i32 - get_base_height(config.base_type) - 1;

        // Draw base
        draw_base(&mut term, config.base_type);
        if config.live {
            term.render()?;
        }

        // Draw message if provided
        if let Some(ref msg) = config.message {
            draw_message(&mut term, msg);
            if config.live {
                term.render()?;
            }
        }

        // Grow tree
        let interrupted = grow_tree(&mut term, config, &mut counters, &mut rng, start_x, start_y, config.live)?;

        if interrupted {
            break;
        }

        // Final render if not live
        if !config.live {
            term.render()?;
        }

        if !config.infinite {
            // Wait for keypress to exit
            loop {
                if let Some(code) = term.wait_key(100)? {
                    match code {
                        KeyCode::Char('?') => {
                            if show_help_modal(&mut term, HELP)? {
                                break;
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        _ => {}
                    }
                }
            }
            break;
        }

        // Infinite mode: wait between trees
        let wait_ms = (config.time_wait * 1000.0) as u64;
        if let Some(code) = term.wait_key(wait_ms)? {
            match code {
                KeyCode::Char('?') => {
                    if show_help_modal(&mut term, HELP)? {
                        break;
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => break,
                _ => {}
            }
        }

        seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    Ok(())
}

fn get_base_height(base_type: u8) -> i32 {
    match base_type {
        1 => 4,
        2 => 3,
        _ => 0,
    }
}

fn draw_base(term: &mut Terminal, base_type: u8) {
    let (width, height) = term.size();
    let center_x = width as i32 / 2;
    let base_y = height as i32;

    match base_type {
        1 => {
            // Large decorative pot
            let pot = [
                ":___________./~~~\\.___________:",
                " \\                           / ",
                "  \\_________________________/ ",
                "  (_)                     (_)",
            ];
            let pot_width = 31;
            let start_x = center_x - pot_width / 2;

            for (i, line) in pot.iter().enumerate() {
                let y = base_y - 4 + i as i32;
                for (j, ch) in line.chars().enumerate() {
                    let color = match ch {
                        '~' => Some(colors::LEAF_LIGHT),
                        '.' | ':' | '_' | '\\' | '/' | '(' | ')' => Some(colors::POT),
                        _ => None,
                    };
                    if ch != ' ' {
                        term.set(start_x + j as i32, y, ch, color, false);
                    }
                }
            }
        }
        2 => {
            // Small simple pot
            let pot = [
                "(---./~~~\\.---)",
                " (           ) ",
                "  (_________) ",
            ];
            let pot_width = 15;
            let start_x = center_x - pot_width / 2;

            for (i, line) in pot.iter().enumerate() {
                let y = base_y - 3 + i as i32;
                for (j, ch) in line.chars().enumerate() {
                    let color = match ch {
                        '~' => Some(colors::LEAF_LIGHT),
                        _ => Some(colors::POT),
                    };
                    if ch != ' ' {
                        term.set(start_x + j as i32, y, ch, color, false);
                    }
                }
            }
        }
        _ => {}
    }
}

fn draw_message(term: &mut Terminal, message: &str) {
    let (width, height) = term.size();
    let max_box_width = (width as f32 * 0.25) as usize;
    let box_width = max_box_width.max(20).min(message.len() + 4);

    // Word wrap the message
    let wrapped = word_wrap(message, box_width - 4);
    let box_height = wrapped.len() + 2;

    let box_x = (width as f32 * 0.7) as i32;
    let box_y = (height as f32 * 0.7) as i32 - box_height as i32 / 2;

    // Draw border
    let border_color = Some(Color::White);

    // Top border
    term.set(box_x, box_y, '+', border_color, false);
    for i in 1..box_width as i32 - 1 {
        term.set(box_x + i, box_y, '-', border_color, false);
    }
    term.set(box_x + box_width as i32 - 1, box_y, '+', border_color, false);

    // Content
    for (i, line) in wrapped.iter().enumerate() {
        let y = box_y + 1 + i as i32;
        term.set(box_x, y, '|', border_color, false);
        term.set_str(box_x + 2, y, line, Some(Color::White), false);
        term.set(box_x + box_width as i32 - 1, y, '|', border_color, false);
    }

    // Bottom border
    let bottom_y = box_y + box_height as i32 - 1;
    term.set(box_x, bottom_y, '+', border_color, false);
    for i in 1..box_width as i32 - 1 {
        term.set(box_x + i, bottom_y, '-', border_color, false);
    }
    term.set(box_x + box_width as i32 - 1, bottom_y, '+', border_color, false);
}

fn word_wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line = word.to_string();
        } else if current_line.len() + 1 + word.len() <= width {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Grow tree using iterative approach with explicit stack
fn grow_tree(
    term: &mut Terminal,
    config: &BonsaiConfig,
    counters: &mut Counters,
    rng: &mut StdRng,
    start_x: i32,
    start_y: i32,
    live: bool,
) -> io::Result<bool> {
    let (_, height) = term.size();
    let max_y = height as i32 - 2;

    // Use a stack instead of recursion
    let mut stack: Vec<BranchTask> = Vec::with_capacity(256);

    // Start with the trunk
    stack.push(BranchTask {
        x: start_x,
        y: start_y,
        branch_type: BranchType::Trunk,
        life: config.life_start as i32,
        shoot_cooldown: config.multiplier as i32,
    });

    while let Some(mut task) = stack.pop() {
        counters.branches += 1;

        // Process this branch segment by segment
        while task.life > 0 {
            // Check for interrupt
            if live {
                if let Some((code, _)) = term.check_key()? {
                    match code {
                        KeyCode::Char('?') => {
                            if show_help_modal(term, HELP)? {
                                return Ok(true);
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
                        _ => {}
                    }
                }
            }

            // Calculate movement deltas
            let (dx, dy) = get_deltas(rng, task.branch_type, task.life, config.multiplier as i32);

            // Boundary check
            let new_y = task.y + dy;
            let actual_dy = if dy > 0 && new_y > max_y { 0 } else { dy };

            task.x += dx;
            task.y += actual_dy;

            // Choose character and color
            let (branch_char, color, bold) = choose_char_and_color(
                rng,
                config,
                task.branch_type,
                dx,
                actual_dy,
            );

            // Draw
            term.set(task.x, task.y, branch_char, Some(color), bold);
            if live {
                term.present()?;
                term.sleep(config.time_step);
            }

            // Branching logic - add new branches to the stack
            if task.life < 3 && task.branch_type != BranchType::Dead {
                // Create leaf clusters (limit to avoid explosion)
                let leaf_count = rng.gen_range(1..3);
                for _ in 0..leaf_count {
                    let leaf_dx: i32 = rng.gen_range(-2..3);
                    let leaf_dy: i32 = rng.gen_range(-1..2);
                    stack.push(BranchTask {
                        x: task.x + leaf_dx,
                        y: task.y + leaf_dy,
                        branch_type: BranchType::Dead,
                        life: 1, // Very short life for dead branches
                        shoot_cooldown: 0,
                    });
                }
            } else if task.branch_type == BranchType::Trunk {
                if task.life < (config.multiplier as i32 + 2) {
                    // Transition to dying - add dying branch to stack
                    stack.push(BranchTask {
                        x: task.x,
                        y: task.y,
                        branch_type: BranchType::Dying,
                        life: task.life.min(8), // Cap dying branch life
                        shoot_cooldown: 0,
                    });
                } else if rng.gen_range(0..3) == 0 || task.life % config.multiplier as i32 == 0 {
                    // Branch into new trunk or shoots
                    if rng.gen_range(0..8) == 0 && stack.len() < 200 {
                        let life_mod: i32 = rng.gen_range(-2..3);
                        stack.push(BranchTask {
                            x: task.x,
                            y: task.y,
                            branch_type: BranchType::Trunk,
                            life: (task.life + life_mod).min(config.life_start as i32),
                            shoot_cooldown: config.multiplier as i32,
                        });
                    } else if task.shoot_cooldown <= 0 && stack.len() < 200 {
                        counters.shoots += 1;
                        counters.shoot_counter += 1;

                        let shoot_life = (task.life / 2 + config.multiplier as i32 / 2).min(15);
                        let shoot_type = if counters.shoot_counter % 2 == 0 {
                            BranchType::ShootLeft
                        } else {
                            BranchType::ShootRight
                        };

                        stack.push(BranchTask {
                            x: task.x,
                            y: task.y,
                            branch_type: shoot_type,
                            life: shoot_life,
                            shoot_cooldown: config.multiplier as i32,
                        });

                        task.shoot_cooldown = config.multiplier as i32 * 2;
                    }
                }
            } else if (task.branch_type == BranchType::ShootLeft || task.branch_type == BranchType::ShootRight)
                && task.life < 4
            {
                // Shoots create dying branches at the end
                stack.push(BranchTask {
                    x: task.x,
                    y: task.y,
                    branch_type: BranchType::Dying,
                    life: 4,
                    shoot_cooldown: 0,
                });
            }

            task.shoot_cooldown -= 1;
            task.life -= 1;
        }
    }

    Ok(false)
}

fn get_deltas(rng: &mut StdRng, branch_type: BranchType, life: i32, multiplier: i32) -> (i32, i32) {
    let dice: i32 = rng.gen_range(0..10);

    match branch_type {
        BranchType::Trunk => {
            if life < 4 {
                // Dying trunk: random horizontal
                let dx = match dice {
                    0..=2 => -1,
                    3..=6 => 0,
                    _ => 1,
                };
                (dx, 0)
            } else if life < multiplier * 3 {
                // Young trunk: tends upward with spread
                let dx = match dice {
                    0 => -2,
                    1..=3 => -1,
                    4..=5 => 0,
                    6..=8 => 1,
                    _ => 2,
                };
                let dy = if dice > 2 { -1 } else { 0 };
                (dx, dy)
            } else {
                // Mature trunk: more vertical
                let dx = match dice {
                    0..=1 => -1,
                    2..=7 => 0,
                    _ => 1,
                };
                let dy = -1;
                (dx, dy)
            }
        }
        BranchType::ShootLeft => {
            let dx = match dice {
                0..=1 => -2,
                2..=5 => -1,
                6..=8 => 0,
                _ => 1,
            };
            let dy = match dice {
                0..=1 => 1,
                2..=5 => 0,
                _ => -1,
            };
            (dx, dy)
        }
        BranchType::ShootRight => {
            let dx = match dice {
                0..=1 => 2,
                2..=5 => 1,
                6..=8 => 0,
                _ => -1,
            };
            let dy = match dice {
                0..=1 => 1,
                2..=5 => 0,
                _ => -1,
            };
            (dx, dy)
        }
        BranchType::Dying => {
            let dx = rng.gen_range(-2..3);
            let dy = match dice {
                0..=3 => 0,
                4..=7 => -1,
                _ => 1,
            };
            (dx, dy)
        }
        BranchType::Dead => {
            let dx = rng.gen_range(-1..2);
            let dy = rng.gen_range(-1..2);
            (dx, dy)
        }
    }
}

fn choose_char_and_color(
    rng: &mut StdRng,
    config: &BonsaiConfig,
    branch_type: BranchType,
    dx: i32,
    dy: i32,
) -> (char, Color, bool) {
    match branch_type {
        BranchType::Trunk | BranchType::ShootLeft | BranchType::ShootRight => {
            let ch = match (dx.signum(), dy.signum()) {
                (_, 0) => '~',
                (-1, -1) | (1, 1) => '\\',
                (1, -1) | (-1, 1) => '/',
                (0, _) => '|',
                _ => '|',
            };
            let bold = rng.gen_bool(0.5);
            let color = if bold {
                colors::WOOD_LIGHT
            } else {
                colors::WOOD_DARK
            };
            (ch, color, bold)
        }
        BranchType::Dying => {
            let leaf = &config.leaves[rng.gen_range(0..config.leaves.len())];
            let ch = leaf.chars().next().unwrap_or('&');
            let bold = rng.gen_bool(0.1);
            (ch, colors::LEAF_LIGHT, bold)
        }
        BranchType::Dead => {
            let leaf = &config.leaves[rng.gen_range(0..config.leaves.len())];
            let ch = leaf.chars().next().unwrap_or('&');
            let bold = rng.gen_bool(0.33);
            let color = if bold {
                colors::LEAF_LIGHT
            } else {
                colors::LEAF_DARK
            };
            (ch, color, bold)
        }
    }
}
