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

/// Types of visualizations available
#[derive(Clone, Copy, PartialEq)]
pub enum FractalType {
    Matrix,   // cmatrix-like falling characters
    Life,     // Conway's Game of Life
    Plasma,   // Animated plasma effect
    Fire,     // Doom-style fire
    Rain,     // Falling rain with splashes
    Waves,    // Animated sine waves
    Cube,     // 3D rotating cube with braille
    Pipes,    // Classic pipes screensaver
    Donut,    // Rotating 3D torus (donut)
    Globe,    // Rotating 3D globe with network activity
    Hex,      // Hexagon grid with wave animations
    Keyboard, // On-screen keyboard with key highlighting
    Invaders, // Space Invaders style game
}

/// Configuration for fractal generation
#[derive(Clone)]
pub struct FractalConfig {
    pub fractal_type: FractalType,
    pub time_step: f32,
    pub seed: Option<u64>,
    pub draw_char: char,
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
