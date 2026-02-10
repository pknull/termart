/// Configuration for bonsai tree generation
#[derive(Clone)]
pub struct BonsaiConfig {
    pub live: bool,
    pub infinite: bool,
    pub print: bool,
    pub time_step: f32,
    pub time_wait: f64,
    pub life_start: u32,
    pub multiplier: u32,
    pub seed: Option<u64>,
    pub base_type: u8,
    pub leaves: Vec<String>,
    pub message: Option<String>,
}

/// Per-visualizer specific configuration
#[derive(Clone)]
pub enum FractalKind {
    Matrix,
    Life { draw_char: char },
    Plasma,
    Fire,
    Rain,
    Waves,
    Cube,
    Hypercube,
    Pipes,
    Donut,
    Globe { geoip_db: Option<std::path::PathBuf>, tilt: f32 },
    Hex,
    Keyboard,
    Invaders,
    Audio,
    Lissajous,
    TuiCover,
    TuiControl,
}

/// Configuration for fractal generation
#[derive(Clone)]
pub struct FractalConfig {
    pub kind: FractalKind,
    pub time_step: f32,
    pub seed: Option<u64>,
    pub debug: bool,
}

/// Branch types for bonsai tree
#[derive(Clone, Copy, PartialEq)]
pub enum BranchType {
    Trunk,
    ShootLeft,
    ShootRight,
    Dying,
    Dead,
}

/// Counters for tracking generation progress
#[derive(Default)]
pub struct Counters {
    pub branches: u32,
    pub shoots: u32,
    pub shoot_counter: i32,
}
