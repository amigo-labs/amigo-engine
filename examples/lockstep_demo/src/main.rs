//! Two players, two machines, one game: each player steers a square.
//!
//! Run it twice, on one machine or two:
//!
//! ```sh
//! AMIGO_NET_HOST=7777 cargo run -p amigo_lockstep_demo
//! AMIGO_NET_JOIN=127.0.0.1:7777 cargo run -p amigo_lockstep_demo
//! ```
//!
//! (In a project made with `amigo new`, that is `amigo run --host 7777` and
//! `amigo run --join <host>:7777`.) Without either it runs alone with one
//! square.
//!
//! The game is written once for any number of players: it walks
//! `ctx.players()` and reads each one's `ctx.player_actions(p)`. It moves in
//! fixed point and draws its dash lengths from `ctx.rng`, so both machines
//! compute the same squares; `state_hash` lets the engine check that after
//! every tick (F1 then F8 shows the session).

use amigo_engine::prelude::*;

const VIRTUAL_W: f32 = 320.0;
const VIRTUAL_H: f32 = 180.0;
const SIZE: i32 = 12;

fn player_color(i: usize) -> Color {
    match i {
        0 => Color::new(0.95, 0.45, 0.3, 1.0),
        _ => Color::new(0.3, 0.7, 0.95, 1.0),
    }
}

struct Duel {
    pos: [SimVec2; 2],
    dashes: [u32; 2],
    message: Option<String>,
}

impl Duel {
    fn new() -> Self {
        Self {
            pos: [SimVec2::from_num(80, 90), SimVec2::from_num(240, 90)],
            dashes: [0; 2],
            message: None,
        }
    }
}

impl Game for Duel {
    fn update(&mut self, ctx: &mut GameContext) -> SceneAction {
        for event in ctx.events.read::<NetEvent>() {
            self.message = Some(match event {
                NetEvent::Connected { local } => format!("connected as player {}", local.0 + 1),
                NetEvent::Disconnected { reason } => format!("{reason}; playing alone"),
                NetEvent::Desync { tick } => format!("DESYNC at tick {tick}"),
            });
        }

        let speed = Fix::from_num(1.5);
        let players: Vec<PlayerId> = ctx.players().collect();
        for p in players {
            let actions = ctx.player_actions(p);
            let mut step = SimVec2::ZERO;
            if actions.held("left") {
                step.x -= speed;
            }
            if actions.held("right") {
                step.x += speed;
            }
            if actions.held("up") {
                step.y -= speed;
            }
            if actions.held("down") {
                step.y += speed;
            }
            let dash = actions.pressed("dash");
            let i = p.0 as usize;
            if dash && step != SimVec2::ZERO {
                // Random but identical on both machines.
                step = step * Fix::from_num(ctx.rng.range(6, 14));
                self.dashes[i] += 1;
            }
            let pos = &mut self.pos[i];
            *pos += step;
            pos.x = pos
                .x
                .clamp(Fix::ZERO, Fix::from_num(VIRTUAL_W as i32 - SIZE));
            pos.y = pos
                .y
                .clamp(Fix::ZERO, Fix::from_num(VIRTUAL_H as i32 - SIZE));
        }
        SceneAction::Continue
    }

    fn state_hash(&self, _ctx: &GameContext) -> Option<u64> {
        let mut h = StateHasher::new();
        for (pos, dashes) in self.pos.iter().zip(self.dashes) {
            h.write_i32(pos.x.to_bits());
            h.write_i32(pos.y.to_bits());
            h.write_u32(dashes);
        }
        Some(h.finish_crc64())
    }

    fn draw(&self, ctx: &mut DrawContext) {
        let view = ctx.view_rect();
        for (i, pos) in self.pos.iter().enumerate() {
            let rect = Rect::new(
                pos.x.to_num::<f32>(),
                pos.y.to_num::<f32>(),
                SIZE as f32,
                SIZE as f32,
            );
            ctx.draw_rect(rect, player_color(i));
            ctx.draw_text(
                &format!("P{}", i + 1),
                rect.x + 1.0,
                rect.y - 9.0,
                Color::WHITE,
            );
        }
        let help = "WASD/arrows move, Space dashes";
        let gray = Color::new(0.6, 0.6, 0.6, 1.0);
        ctx.draw_text(help, view.x + 4.0, view.y + view.h - 10.0, gray);
        if let Some(message) = &self.message {
            ctx.draw_text(message, view.x + 4.0, view.y + 4.0, Color::YELLOW);
        }
    }
}

fn main() {
    let mut bindings = ActionBindings::new();
    for (action, keys) in [
        ("left", ["A", "Left"]),
        ("right", ["D", "Right"]),
        ("up", ["W", "Up"]),
        ("down", ["S", "Down"]),
    ] {
        for key in keys {
            bindings.bind_key(action, key);
        }
    }
    bindings.bind_key("dash", "Space");

    Engine::build()
        .title("Amigo Lockstep Demo")
        .virtual_resolution(VIRTUAL_W as u32, VIRTUAL_H as u32)
        .input_bindings(bindings)
        .build()
        .run(Duel::new());
}
