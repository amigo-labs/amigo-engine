use amigo_engine::prelude::*;

const COLS: usize = 30;
const ROWS: usize = 17;
const TILE: f32 = 16.0;

struct Grid {
    walkable: Vec<Vec<bool>>,
}

impl Grid {
    fn new() -> Self {
        Self {
            walkable: vec![vec![true; COLS]; ROWS],
        }
    }

    fn toggle(&mut self, x: usize, y: usize) {
        if x < COLS && y < ROWS {
            self.walkable[y][x] = !self.walkable[y][x];
        }
    }
}

impl Walkable for Grid {
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < COLS
            && (y as usize) < ROWS
            && self.walkable[y as usize][x as usize]
    }
}

struct PathfindingDemo {
    grid: Grid,
    start: Option<IVec2>,
    goal: Option<IVec2>,
    path: Option<Vec<IVec2>>,
    flow: Option<FlowField>,
    show_flow: bool,
    paint_mode: bool,
}

impl PathfindingDemo {
    fn new() -> Self {
        Self {
            grid: Grid::new(),
            start: None,
            goal: None,
            path: None,
            flow: None,
            show_flow: false,
            paint_mode: false,
        }
    }

    fn recompute(&mut self) {
        self.path = None;
        self.flow = None;
        if let (Some(s), Some(g)) = (self.start, self.goal) {
            let req = PathRequest::new(s, g);
            self.path = find_path(&req, &self.grid);
            self.flow = Some(FlowField::compute(g, COLS as u32, ROWS as u32, &self.grid));
        }
    }

    fn mouse_to_cell(pos: RenderVec2) -> Option<(usize, usize)> {
        let cx = (pos.x / TILE) as i32;
        let cy = (pos.y / TILE) as i32;
        if cx >= 0 && cy >= 0 && (cx as usize) < COLS && (cy as usize) < ROWS {
            Some((cx as usize, cy as usize))
        } else {
            None
        }
    }
}

impl Game for PathfindingDemo {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        if ctx.input.pressed(KeyCode::KeyF) {
            self.show_flow = !self.show_flow;
        }
        if ctx.input.pressed(KeyCode::Space) {
            self.paint_mode = !self.paint_mode;
        }

        let pos = ctx.input.mouse_world_pos();
        if let Some((cx, cy)) = Self::mouse_to_cell(pos) {
            if self.paint_mode {
                if ctx.input.mouse_pressed(MouseButton::Left) {
                    self.grid.toggle(cx, cy);
                    self.recompute();
                }
            } else {
                if ctx.input.mouse_pressed(MouseButton::Left) {
                    self.start = Some(IVec2::new(cx as i32, cy as i32));
                    self.recompute();
                }
                if ctx.input.mouse_pressed(MouseButton::Right) {
                    self.goal = Some(IVec2::new(cx as i32, cy as i32));
                    self.recompute();
                }
            }
        }

        SceneAction::Continue
    }

    fn draw(&self, ctx: &mut DrawContext) {
        // Draw grid cells
        for y in 0..ROWS {
            for x in 0..COLS {
                let color = if !self.grid.walkable[y][x] {
                    Color::new(0.15, 0.15, 0.18, 1.0)
                } else {
                    Color::new(0.25, 0.25, 0.30, 1.0)
                };
                let r = Rect::new(
                    x as f32 * TILE + 0.5,
                    y as f32 * TILE + 0.5,
                    TILE - 1.0,
                    TILE - 1.0,
                );
                ctx.draw_rect(r, color);
            }
        }

        // Draw path
        if let Some(ref path) = self.path {
            for p in path {
                let r = Rect::new(
                    p.x as f32 * TILE + 2.0,
                    p.y as f32 * TILE + 2.0,
                    TILE - 4.0,
                    TILE - 4.0,
                );
                ctx.draw_rect(r, Color::new(1.0, 0.9, 0.2, 0.7));
            }
        }

        // Draw flow field arrows
        if self.show_flow {
            if let Some(ref flow) = self.flow {
                for y in 0..ROWS as i32 {
                    for x in 0..COLS as i32 {
                        let (dx, dy) = flow.direction_at(x, y);
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let cx = x as f32 * TILE + TILE * 0.5;
                        let cy = y as f32 * TILE + TILE * 0.5;
                        let ax = cx + dx as f32 * 4.0;
                        let ay = cy + dy as f32 * 4.0;
                        let r = Rect::new(ax - 1.0, ay - 1.0, 2.0, 2.0);
                        ctx.draw_rect(r, Color::new(0.4, 0.7, 1.0, 0.6));
                    }
                }
            }
        }

        // Draw start and goal markers
        if let Some(s) = self.start {
            let r = Rect::new(
                s.x as f32 * TILE + 1.0,
                s.y as f32 * TILE + 1.0,
                TILE - 2.0,
                TILE - 2.0,
            );
            ctx.draw_rect(r, Color::new(0.2, 0.9, 0.3, 1.0));
        }
        if let Some(g) = self.goal {
            let r = Rect::new(
                g.x as f32 * TILE + 1.0,
                g.y as f32 * TILE + 1.0,
                TILE - 2.0,
                TILE - 2.0,
            );
            ctx.draw_rect(r, Color::new(0.9, 0.2, 0.2, 1.0));
        }

        // HUD
        let mode = if self.paint_mode { "PAINT" } else { "NAV" };
        let hud = format!("LMB=Start  RMB=Goal  F=FlowField  Space=Paint [{}]", mode);
        ctx.draw_text(&hud, 4.0, 2.0, Color::WHITE);
    }
}

fn main() {
    let game = PathfindingDemo::new();
    Engine::build()
        .title("Pathfinding Demo")
        .virtual_resolution(480, 272)
        .build()
        .run(game);
}
