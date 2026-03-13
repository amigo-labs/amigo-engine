use amigo_engine::prelude::*;

mod data;
mod game;
mod player;
mod states;

fn main() {
    Engine::build()
        .title("Amigo Starter")
        .virtual_resolution(640, 360)
        .window_size(1280, 720)
        .build()
        .run(game::StarterGame::new());
}
