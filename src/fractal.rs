//! Visualization dispatcher
//!
//! Routes to the appropriate visualization module based on config.

use crate::config::FractalConfig;
use crate::terminal::Terminal;
use rand::prelude::*;
use std::io;

/// Run the visualization
pub fn run(config: FractalConfig) -> io::Result<()> {
    let seed = config.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    let mut rng = StdRng::seed_from_u64(seed);
    let mut term = Terminal::new(true)?;

    term.clear_screen()?;

    match config.fractal_type {
        crate::config::FractalType::Matrix => crate::viz::matrix::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Life => crate::viz::life::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Plasma => crate::viz::plasma::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Fire => crate::viz::fire::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Rain => crate::viz::rain::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Waves => crate::viz::waves::run(&mut term, &config),
        crate::config::FractalType::Cube => crate::viz::cube::run(&mut term, &config),
        crate::config::FractalType::Pipes => crate::viz::pipes::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Donut => crate::viz::donut::run(&mut term, &config),
        crate::config::FractalType::Globe => crate::viz::globe::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Hex => crate::viz::hex::run(&mut term, &config, &mut rng),
        crate::config::FractalType::Keyboard => crate::viz::keyboard::run(&mut term, &config),
        crate::config::FractalType::Invaders => crate::viz::invaders::run(&mut term, &config, &mut rng),
    }
}
