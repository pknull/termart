mod config;
mod terminal;
mod bonsai;
mod fractal;

use clap::{Parser, Subcommand};
use config::{BonsaiConfig, FractalConfig, FractalType};
use std::io;

#[derive(Parser)]
#[command(name = "termart")]
#[command(author = "Terminal Art Generator")]
#[command(version = "0.1.0")]
#[command(about = "Terminal-based generative art: bonsai trees and fractals", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a bonsai tree
    Bonsai {
        /// Show live growth animation
        #[arg(short, long)]
        live: bool,

        /// Keep generating trees infinitely
        #[arg(short, long)]
        infinite: bool,

        /// Print tree to stdout (no interactive display)
        #[arg(short, long)]
        print: bool,

        /// Animation step delay in seconds
        #[arg(short, long, default_value = "0.03")]
        time: f32,

        /// Wait time between trees in infinite mode (seconds)
        #[arg(short, long, default_value = "4.0")]
        wait: f64,

        /// Initial branch life (0-200, higher = bigger tree)
        #[arg(short = 'L', long, default_value = "32")]
        life: u32,

        /// Branch multiplier (0-20, higher = bushier)
        #[arg(short = 'M', long, default_value = "5")]
        multiplier: u32,

        /// Random seed for reproducibility
        #[arg(short, long)]
        seed: Option<u64>,

        /// Base/pot type (0=none, 1=large pot, 2=small pot)
        #[arg(short, long, default_value = "1")]
        base: u8,

        /// Leaf characters (comma-separated)
        #[arg(short = 'c', long, default_value = "&")]
        leaf: String,

        /// Message to display next to tree
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Run an animated visualization (matrix, life, plasma, fire, rain, waves, cube, pipes, donut)
    Viz {
        /// Type of visualization: matrix, life, plasma, fire, rain, waves, cube, pipes, donut
        #[arg(short = 'T', long, default_value = "matrix")]
        viz_type: String,

        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.03")]
        time: f32,

        /// Random seed for reproducibility
        #[arg(short, long)]
        seed: Option<u64>,

        /// Character to use for drawing (life mode)
        #[arg(short, long, default_value = "#")]
        char: String,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bonsai {
            live,
            infinite,
            print,
            time,
            wait,
            life,
            multiplier,
            seed,
            base,
            leaf,
            message,
        } => {
            let leaves: Vec<String> = leaf.split(',').map(|s| s.to_string()).collect();
            let config = BonsaiConfig {
                live,
                infinite,
                print,
                time_step: time,
                time_wait: wait,
                life_start: life.min(200),
                multiplier: multiplier.min(20),
                seed,
                base_type: base.min(2),
                leaves,
                message,
            };
            bonsai::run(config)?;
        }
        Commands::Viz {
            viz_type,
            time,
            seed,
            char: draw_char,
        } => {
            let ftype = match viz_type.to_lowercase().as_str() {
                "matrix" | "cmatrix" => FractalType::Matrix,
                "life" | "gol" | "gameoflife" => FractalType::Life,
                "plasma" => FractalType::Plasma,
                "fire" | "doom" => FractalType::Fire,
                "rain" => FractalType::Rain,
                "waves" | "wave" | "ocean" => FractalType::Waves,
                "cube" | "3d" => FractalType::Cube,
                "pipes" | "pipe" => FractalType::Pipes,
                "donut" | "torus" | "doughnut" => FractalType::Donut,
                _ => {
                    eprintln!("Unknown viz type: {}. Using matrix.", viz_type);
                    eprintln!("Available: matrix, life, plasma, fire, rain, waves, cube, pipes, donut");
                    FractalType::Matrix
                }
            };
            let config = FractalConfig {
                fractal_type: ftype,
                live: true,  // Always live for visualizations
                time_step: time,
                depth: 0,    // Not used
                seed,
                draw_char: draw_char.chars().next().unwrap_or('#'),
                infinite: false,
                time_wait: 0.0,
            };
            fractal::run(config)?;
        }
    }

    Ok(())
}
