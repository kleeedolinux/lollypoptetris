use ggez::{Context, GameResult};
use ggez::graphics::{self, Color, DrawMode, DrawParam, Rect, Text};
use ggez::event::{self, EventHandler};
use ggez::input::keyboard::{KeyCode, KeyInput};
use ggez::audio::{self, SoundSource};
use rand::Rng;
use std::time::Duration;
use std::process::Command;

const CELL_SIZE: f32 = 30.0;
const GRID_WIDTH: usize = 10;
const GRID_HEIGHT: usize = 20;
const PINK: Color = Color::new(1.0, 0.41, 0.71, 1.0);
const YELLOW: Color = Color::new(1.0, 1.0, 0.0, 1.0);

struct Block {
    x: i32,
    y: i32,
    shape: Vec<Vec<bool>>,
    color: Color,
}

struct GameState {
    block: Block,
    grid: Vec<Vec<Option<Color>>>,
    fall_time: Duration,
    last_update: Duration,
    score: u32,
    game_over: bool,
    death_sound: audio::Source,
    combo_sound: audio::Source,
    start_sound: audio::Source,
    freeze_timer: Option<Duration>,
    freeze_start: Option<Duration>,
    death_count: u32,
    jumpscare_shown: bool,
}

impl Block {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let shapes = vec![
            // I
            vec![
                vec![true, true, true, true],
                vec![false, false, false, false],
                vec![false, false, false, false],
                vec![false, false, false, false],
            ],
            // O
            vec![
                vec![true, true],
                vec![true, true],
            ],
            // T
            vec![
                vec![false, true, false],
                vec![true, true, true],
                vec![false, false, false],
            ],
            // L
            vec![
                vec![true, false, false],
                vec![true, true, true],
                vec![false, false, false],
            ],
            // J
            vec![
                vec![false, false, true],
                vec![true, true, true],
                vec![false, false, false],
            ],
            // S
            vec![
                vec![false, true, true],
                vec![true, true, false],
                vec![false, false, false],
            ],
            // Z
            vec![
                vec![true, true, false],
                vec![false, true, true],
                vec![false, false, false],
            ],
        ];

        let shape = shapes[rng.gen_range(0..shapes.len())].clone();
        let color = if rng.gen_bool(0.5) { PINK } else { YELLOW };

        Block {
            x: (GRID_WIDTH as i32 - shape[0].len() as i32) / 2,
            y: 0,
            shape,
            color,
        }
    }

    fn can_move(&self, dx: i32, dy: i32, grid: &Vec<Vec<Option<Color>>>) -> bool {
        for (y, row) in self.shape.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                if cell {
                    let new_x = self.x + x as i32 + dx;
                    let new_y = self.y + y as i32 + dy;

                    if new_x < 0 || new_x >= GRID_WIDTH as i32 || new_y >= GRID_HEIGHT as i32 {
                        return false;
                    }

                    if new_y >= 0 && grid[new_y as usize][new_x as usize].is_some() {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn rotate(&mut self, grid: &Vec<Vec<Option<Color>>>) {
        let rows = self.shape.len();
        let cols = self.shape[0].len();
        let mut new_shape = vec![vec![false; rows]; cols];

        for y in 0..rows {
            for x in 0..cols {
                new_shape[x][rows - 1 - y] = self.shape[y][x];
            }
        }

        let old_shape = self.shape.clone();
        self.shape = new_shape;

        if !self.can_move(0, 0, grid) {
            self.shape = old_shape;
        }
    }
}

impl GameState {
    fn new(ctx: &mut Context) -> GameResult<Self> {
        let death_sound = audio::Source::new(ctx, "/death.ogg")?;
        let combo_sound = audio::Source::new(ctx, "/atk.ogg")?;
        let mut start_sound = audio::Source::new(ctx, "/random.mp3")?;
        start_sound.set_volume(10.0);
        
        Ok(GameState {
            block: Block::new(),
            grid: vec![vec![None; GRID_WIDTH]; GRID_HEIGHT],
            fall_time: Duration::from_secs(1),
            last_update: Duration::from_secs(0),
            score: 0,
            game_over: false,
            death_sound,
            combo_sound,
            start_sound,
            freeze_timer: None,
            freeze_start: None,
            death_count: 0,
            jumpscare_shown: false,
        })
    }

    fn place_block(&mut self) {
        for (y, row) in self.block.shape.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                if cell {
                    let grid_y = (self.block.y + y as i32) as usize;
                    let grid_x = (self.block.x + x as i32) as usize;
                    if grid_y < GRID_HEIGHT {
                        self.grid[grid_y][grid_x] = Some(self.block.color);
                    }
                }
            }
        }
    }

    fn clear_lines(&mut self, ctx: &mut Context) -> GameResult {
        let mut lines_cleared = 0;
        
        for y in (0..GRID_HEIGHT).rev() {
            if self.grid[y].iter().all(|cell| cell.is_some()) {
                self.grid.remove(y);
                self.grid.insert(0, vec![None; GRID_WIDTH]);
                lines_cleared += 1;
                self.combo_sound.play_detached(ctx)?;
            }
        }
        
        if lines_cleared > 0 {
            self.score += lines_cleared * 100;
            self.fall_time = Duration::from_millis((1000.0 * 0.9f32.powi(self.score as i32 / 1000)) as u64);
        }
        Ok(())
    }

    fn check_game_over(&mut self, ctx: &mut Context) -> GameResult {
        if self.grid[0].iter().any(|cell| cell.is_some()) {
            self.game_over = true;
            self.death_count += 1;
            let _ = self.death_sound.play_detached(ctx)?;
            self.freeze_timer = Some(Duration::from_secs(5));
            self.freeze_start = Some(ctx.time.time_since_start());
            let _ = self.start_sound.play_detached(ctx)?;

            if self.death_count == 1 && !self.jumpscare_shown {
                self.jumpscare_shown = true;
                if let Ok(resource_path) = std::env::current_dir() {
                    let image_path = resource_path.join("resource").join("buuh.png");
                    let _ = Command::new("cmd")
                        .args(["/C", "start", "", image_path.to_str().unwrap_or("")])
                        .spawn();
                }
            }
        }
        Ok(())
    }

    fn draw_jumpscare(&mut self) -> GameResult {
        Ok(())
    }
}

impl EventHandler<ggez::GameError> for GameState {
    fn update(&mut self, ctx: &mut Context) -> GameResult {
        if let (Some(freeze_timer), Some(freeze_start)) = (self.freeze_timer, self.freeze_start) {
            let now = ctx.time.time_since_start();
            if now - freeze_start < freeze_timer {
                return Ok(());
            } else {
                self.freeze_timer = None;
                self.freeze_start = None;
                self.game_over = false;
                self.grid = vec![vec![None; GRID_WIDTH]; GRID_HEIGHT];
                self.block = Block::new();
                self.score = 0;
                self.jumpscare_shown = false;
            }
        }

        if self.game_over {
            return Ok(());
        }

        let now = ctx.time.time_since_start();
        if now - self.last_update >= self.fall_time {
            if self.block.can_move(0, 1, &self.grid) {
                self.block.y += 1;
            } else {
                self.place_block();
                self.clear_lines(ctx)?;
                self.check_game_over(ctx)?;
                self.block = Block::new();
            }
            self.last_update = now;
        }
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult {
        let mut canvas = graphics::Canvas::from_frame(ctx, Color::BLACK);
        
        for (y, row) in self.grid.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if let Some(color) = cell {
                    let rect = Rect::new(
                        x as f32 * CELL_SIZE,
                        y as f32 * CELL_SIZE,
                        CELL_SIZE,
                        CELL_SIZE,
                    );
                    let mesh = graphics::Mesh::new_rectangle(
                        ctx,
                        DrawMode::fill(),
                        rect,
                        *color,
                    )?;
                    canvas.draw(&mesh, DrawParam::default());
                }
            }
        }
        
        for (y, row) in self.block.shape.iter().enumerate() {
            for (x, &cell) in row.iter().enumerate() {
                if cell {
                    let rect = Rect::new(
                        (self.block.x + x as i32) as f32 * CELL_SIZE,
                        (self.block.y + y as i32) as f32 * CELL_SIZE,
                        CELL_SIZE,
                        CELL_SIZE,
                    );
                    let mesh = graphics::Mesh::new_rectangle(
                        ctx,
                        DrawMode::fill(),
                        rect,
                        self.block.color,
                    )?;
                    canvas.draw(&mesh, DrawParam::default());
                }
            }
        }
        
        if self.game_over && self.death_count == 1 {
            let screen_width = GRID_WIDTH as f32 * CELL_SIZE;
            let screen_height = GRID_HEIGHT as f32 * CELL_SIZE;
            let text = Text::new("Jogue mais uma vez para liberar um easter egg");
            let text_pos = [
                screen_width / 2.0 - 150.0,
                screen_height / 2.0 + 100.0,
            ];
            canvas.draw(&text, DrawParam::default().dest(text_pos).color(Color::WHITE));
        }
        
        canvas.finish(ctx)?;
        Ok(())
    }

    fn key_down_event(&mut self, _ctx: &mut Context, input: KeyInput, _repeat: bool) -> GameResult {
        if self.freeze_timer.is_some() {
            return Ok(());
        }

        if let Some(keycode) = input.keycode {
            match keycode {
                KeyCode::Left => {
                    if self.block.can_move(-1, 0, &self.grid) {
                        self.block.x -= 1;
                    }
                }
                KeyCode::Right => {
                    if self.block.can_move(1, 0, &self.grid) {
                        self.block.x += 1;
                    }
                }
                KeyCode::Down => {
                    if self.block.can_move(0, 1, &self.grid) {
                        self.block.y += 1;
                    }
                }
                KeyCode::Up => {
                    self.block.rotate(&self.grid);
                }
                KeyCode::Space => {
                    while self.block.can_move(0, 1, &self.grid) {
                        self.block.y += 1;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn main() -> GameResult {
    let cb = ggez::ContextBuilder::new("lollypop", "cascade")
        .window_setup(ggez::conf::WindowSetup::default().title("Lollypop Tetris"))
        .window_mode(ggez::conf::WindowMode::default().dimensions(
            GRID_WIDTH as f32 * CELL_SIZE,
            GRID_HEIGHT as f32 * CELL_SIZE,
        ))
        .add_resource_path("resource");

    let (mut ctx, event_loop) = cb.build()?;
    let state = GameState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}
