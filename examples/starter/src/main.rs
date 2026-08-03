use amigo_engine::prelude::*;

mod data;
mod player;
mod states;

fn main() {
    Engine::build()
        .title("Amigo Starter")
        .virtual_resolution(640, 360)
        .window_size(1280, 720)
        .build()
        // Each state is its own `Game`; LoadingState replaces itself with the
        // menu, which pushes gameplay, which pops back. No hand-rolled state
        // enum — the engine owns the stack.
        .run(states::LoadingState::new());
}
