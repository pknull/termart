// Prevent new unwrap() calls - existing code should migrate to proper error handling
#![warn(clippy::unwrap_used)]

mod config;
mod terminal;
mod colors;
mod help;
mod bonsai;
mod fractal;
mod viz;
mod monitor;
mod weather;
mod pomodoro;
mod fah;
mod settings;
mod net_geo;
mod evdev_util;

use clap::{Parser, Subcommand, Args};
use config::{BonsaiConfig, FractalConfig, FractalKind};
use monitor::{MonitorConfig, MonitorType};
use std::io;
use std::path::PathBuf;

#[derive(Args, Clone)]
struct VizOptions {
    /// Animation speed (seconds per frame)
    #[arg(short, long, default_value = "0.03")]
    time: f32,

    /// Random seed for reproducibility
    #[arg(short, long)]
    seed: Option<u64>,

    /// Show debug info
    #[arg(short, long)]
    debug: bool,
}

#[derive(Args, Clone)]
struct MonitorOptions {
    /// Update interval (seconds)
    #[arg(short, long, default_value = "1.0")]
    time: f32,

    /// Show debug info
    #[arg(short, long)]
    debug: bool,
}

#[derive(Parser)]
#[command(name = "termart")]
#[command(author = "Terminal Art Generator")]
#[command(version = "0.1.0")]
#[command(about = "Terminal-based generative art", long_about = None)]
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

    /// Matrix rain effect
    Matrix {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Conway's Game of Life
    Life {
        #[command(flatten)]
        opts: VizOptions,

        /// Character to use for drawing
        #[arg(short, long, default_value = "#")]
        char: String,
    },

    /// Plasma effect
    Plasma {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Doom-style fire
    Fire {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Rain animation
    Rain {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Ocean waves
    Waves {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Spinning 3D cube
    Cube {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Spinning 4D hypercube (tesseract)
    Hypercube {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Pipes screensaver
    Pipes {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Spinning donut (torus)
    Donut {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Rotating globe with network connections
    Globe {
        #[command(flatten)]
        opts: VizOptions,

        /// Path to GeoLite2-City.mmdb for real network visualization
        #[arg(long)]
        geoip: Option<PathBuf>,

        /// Initial tilt angle in degrees (-90 to 90, default: 8)
        #[arg(long, default_value = "8")]
        tilt: f32,
    },

    /// Hexagonal grid pattern
    Hex {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Keyboard visualization
    Keyboard {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Dygma Raise split keyboard visualization
    Dygma {
        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.03")]
        time: f32,

        /// Serial port path (auto-detect if not specified)
        #[arg(short, long)]
        port: Option<std::path::PathBuf>,

        /// Show debug info
        #[arg(short, long)]
        debug: bool,
    },

    /// Space Invaders style game
    Invaders {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Audio spectrum visualizer (CAVA-style)
    Audio {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Lissajous curves with braille rendering
    Lissajous {
        #[command(flatten)]
        opts: VizOptions,
    },

    /// Clock display with nixie tube effects - alternates between time and date
    Clock {
        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.1")]
        time: f32,

        /// Hide seconds (show only HH:MM)
        #[arg(long)]
        no_seconds: bool,
    },

    /// Sunlight cycle visualization with screen temperature control
    Sunlight {
        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.1")]
        time: f32,

        /// Latitude in degrees (-90 to 90)
        #[arg(long)]
        lat: Option<f64>,

        /// Longitude in degrees (-180 to 180)
        #[arg(long)]
        lon: Option<f64>,

        /// Disable screen gamma adjustment
        #[arg(long)]
        no_gamma: bool,

        /// Demo mode: cycle through day quickly
        #[arg(long)]
        demo: bool,

        /// Demo speed: hours per second (default 2.0 = full day in 12s)
        #[arg(long, default_value = "2.0")]
        demo_speed: f32,

        /// Night color temperature in Kelvin (1900-6500, default 3400 like f.lux)
        #[arg(long)]
        night_temp: Option<u32>,

        /// Night blue gamma (0.0-1.0, overridden by --night-temp)
        #[arg(long)]
        night_blue: Option<f64>,

        /// Night green gamma (0.0-1.0, overridden by --night-temp)
        #[arg(long)]
        night_green: Option<f64>,
    },

    /// Pong - two player game
    Pong {
        /// Game speed (seconds per frame)
        #[arg(short, long, default_value = "0.016")]
        time: f32,
    },

    /// CPU usage monitor
    Cpu {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// Memory usage monitor
    Mem {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// Disk space usage
    Disk {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// Disk I/O rates
    Io {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// Network I/O rates
    Net {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// GPU usage monitor (NVIDIA)
    Gpu {
        #[command(flatten)]
        opts: MonitorOptions,
    },

    /// Process list (top processes by CPU/memory)
    Ps {
        /// Update interval (seconds)
        #[arg(short, long, default_value = "2.0")]
        time: f32,

        /// Max processes to show
        #[arg(short = 'n', long, default_value = "50")]
        count: usize,

        /// Include kernel threads
        #[arg(long)]
        all: bool,
    },

    /// Docker container stats
    Docker {
        /// Update interval (seconds)
        #[arg(short, long, default_value = "2.0")]
        time: f32,
    },

    /// Live weather display with ASCII art
    Weather {
        /// Location (city name, e.g., "London" or "New York")
        #[arg(short, long)]
        location: Option<String>,

        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.1")]
        time: f32,

        /// Demo mode: cycle through all weather conditions
        #[arg(long)]
        demo: bool,

        /// Demo speed: seconds per condition (default 3.0)
        #[arg(long, default_value = "3.0")]
        demo_speed: f32,
    },

    /// Pomodoro timer with ASCII tomato
    Pomodoro {
        /// Work duration in minutes
        #[arg(short, long, default_value = "25")]
        work: u32,

        /// Short break duration in minutes
        #[arg(short, long, default_value = "5")]
        short_break: u32,

        /// Long break duration in minutes
        #[arg(short, long, default_value = "15")]
        long_break: u32,

        /// Pomodoros before long break
        #[arg(short, long, default_value = "4")]
        count: u32,
    },

    /// Folding@Home stats display
    Fah {
        /// FAH username (or set in ~/.config/termart/config.toml)
        #[arg(short, long)]
        user: Option<String>,

        /// Animation speed (seconds per frame)
        #[arg(short, long, default_value = "0.1")]
        time: f32,
    },
}

fn run_viz(kind: FractalKind, opts: VizOptions) -> io::Result<()> {
    let config = FractalConfig {
        kind,
        time_step: opts.time,
        seed: opts.seed,
        debug: opts.debug,
    };
    fractal::run(config)
}

fn run_monitor(mtype: MonitorType, opts: MonitorOptions) -> io::Result<()> {
    let config = MonitorConfig {
        monitor_type: mtype,
        time_step: opts.time,
        debug: opts.debug,
    };
    monitor::run(config)
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
        Commands::Matrix { opts } => run_viz(FractalKind::Matrix, opts)?,
        Commands::Life { opts, char: c } => {
            let draw_char = c.chars().next().unwrap_or('#');
            run_viz(FractalKind::Life { draw_char }, opts)?
        }
        Commands::Plasma { opts } => run_viz(FractalKind::Plasma, opts)?,
        Commands::Fire { opts } => run_viz(FractalKind::Fire, opts)?,
        Commands::Rain { opts } => run_viz(FractalKind::Rain, opts)?,
        Commands::Waves { opts } => run_viz(FractalKind::Waves, opts)?,
        Commands::Cube { opts } => run_viz(FractalKind::Cube, opts)?,
        Commands::Hypercube { opts } => run_viz(FractalKind::Hypercube, opts)?,
        Commands::Pipes { opts } => run_viz(FractalKind::Pipes, opts)?,
        Commands::Donut { opts } => run_viz(FractalKind::Donut, opts)?,
        Commands::Globe { opts, geoip, tilt } => {
            let settings = settings::Settings::load();
            let geoip_db = geoip.or(settings.globe.geoip_db);
            run_viz(FractalKind::Globe { geoip_db, tilt: tilt.to_radians() }, opts)?
        }
        Commands::Hex { opts } => run_viz(FractalKind::Hex, opts)?,
        Commands::Keyboard { opts } => run_viz(FractalKind::Keyboard, opts)?,
        Commands::Dygma { time, port, debug } => {
            let config = viz::dygma::DygmaConfig {
                time_step: time,
                port,
                debug,
            };
            viz::dygma::run(config)?;
        }
        Commands::Invaders { opts } => run_viz(FractalKind::Invaders, opts)?,
        Commands::Audio { opts } => run_viz(FractalKind::Audio, opts)?,
        Commands::Lissajous { opts } => run_viz(FractalKind::Lissajous, opts)?,
        Commands::Clock { time, no_seconds } => {
            let config = viz::clock::ClockConfig {
                time_step: time,
                show_seconds: !no_seconds,
                ..Default::default()
            };
            viz::clock::run(config)?;
        }
        Commands::Sunlight { time, lat, lon, no_gamma, demo, demo_speed, night_temp, night_blue, night_green } => {
            let settings = settings::Settings::load();

            // Location: CLI > config file > NYC default
            let latitude = lat
                .or(settings.sunlight.latitude)
                .unwrap_or(40.7128);
            let longitude = lon
                .or(settings.sunlight.longitude)
                .unwrap_or(-74.0060);

            // Night temperature: --night-temp in Kelvin, or individual --night-blue/--night-green
            // Default is 3400K (f.lux default)
            let default_kelvin = 3400;
            let (_, default_g, default_b) = viz::sunlight::kelvin_to_gamma(default_kelvin);

            let (night_green_val, night_blue_val) = if let Some(kelvin) = night_temp {
                let (_, g, b) = viz::sunlight::kelvin_to_gamma(kelvin);
                (g, b)
            } else {
                (night_green.unwrap_or(default_g), night_blue.unwrap_or(default_b))
            };

            let config = viz::sunlight::SunlightConfig {
                time_step: time,
                latitude,
                longitude,
                adjust_gamma: !no_gamma,
                demo,
                demo_speed,
                night_blue: night_blue_val,
                night_green: night_green_val,
            };
            viz::sunlight::run(config)?;
        }
        Commands::Pong { time } => {
            viz::pong::run(time)?;
        }
        Commands::Cpu { opts } => run_monitor(MonitorType::Cpu, opts)?,
        Commands::Mem { opts } => run_monitor(MonitorType::Mem, opts)?,
        Commands::Disk { opts } => run_monitor(MonitorType::Disk, opts)?,
        Commands::Io { opts } => run_monitor(MonitorType::Io, opts)?,
        Commands::Net { opts } => run_monitor(MonitorType::Net, opts)?,
        Commands::Gpu { opts } => run_monitor(MonitorType::Gpu, opts)?,
        Commands::Ps { time, count, all } => {
            let config = monitor::ps::PsConfig {
                time_step: time,
                max_procs: count,
                show_kernel: all,
            };
            monitor::ps::run(config)?;
        }
        Commands::Docker { time } => {
            let config = monitor::docker::DockerConfig {
                time_step: time,
            };
            monitor::docker::run(config)?;
        }
        Commands::Weather { location, time, demo, demo_speed } => {
            let config = weather::WeatherConfig {
                location,
                time_step: time,
                demo,
                demo_speed,
            };
            weather::run(config)?;
        }
        Commands::Pomodoro { work, short_break, long_break, count } => {
            let config = pomodoro::PomodoroConfig {
                work_mins: work,
                short_break_mins: short_break,
                long_break_mins: long_break,
                pomodoros_until_long: count,
            };
            pomodoro::run(config)?;
        }
        Commands::Fah { user, time } => {
            let settings = settings::Settings::load();

            // Username: CLI > config file
            let username = user.or(settings.fah.username).unwrap_or_else(|| {
                eprintln!("Error: FAH username required. Use --user or set in {}",
                    settings::Settings::config_path().display());
                std::process::exit(1);
            });

            let config = fah::FahConfig {
                username,
                email: settings.fah.email,
                password: settings.fah.password,
                fah_secret: settings.fah.fah_secret,
                fah_sid: settings.fah.fah_sid,
                time_step: time,
            };
            fah::run(config)?;
        }
    }

    Ok(())
}
