//! Visualization dispatcher
//!
//! Routes to the appropriate visualization module based on config.

use crate::config::{FractalConfig, FractalKind};
use crate::terminal::Terminal;
use rand::prelude::*;
use std::io;

/// Run the visualization
pub fn run(config: FractalConfig) -> io::Result<()> {
    let seed = config.seed.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    });

    let mut rng = StdRng::seed_from_u64(seed);
    let mut term = Terminal::new(true)?;

    term.clear_screen()?;

    match &config.kind {
        FractalKind::Matrix => crate::viz::matrix::run(&mut term, &config, &mut rng),
        FractalKind::Life { draw_char } => crate::viz::life::run(&mut term, &config, &mut rng, *draw_char),
        FractalKind::Plasma => crate::viz::plasma::run(&mut term, &config, &mut rng),
        FractalKind::Fire => crate::viz::fire::run(&mut term, &config, &mut rng),
        FractalKind::Rain => crate::viz::rain::run(&mut term, &config, &mut rng),
        FractalKind::Waves => crate::viz::waves::run(&mut term, &config),
        FractalKind::Cube => crate::viz::cube::run(&mut term, &config),
        FractalKind::Hypercube => crate::viz::hypercube::run(&mut term, &config),
        FractalKind::Pipes => crate::viz::pipes::run(&mut term, &config, &mut rng),
        FractalKind::Donut => crate::viz::donut::run(&mut term, &config),
        FractalKind::Globe { geoip_db, tilt } => crate::viz::globe::run(&mut term, &config, &mut rng, geoip_db.as_deref(), *tilt),
        FractalKind::Hex => crate::viz::hex::run(&mut term, &config, &mut rng),
        FractalKind::Keyboard => crate::viz::keyboard::run(&mut term, &config),
        FractalKind::Invaders => crate::viz::invaders::run(&mut term, &config, &mut rng),
        FractalKind::Audio => crate::viz::audio::run(&mut term, &config),
        FractalKind::Lissajous => crate::viz::lissajous::run(&mut term, &config),
    }
}
