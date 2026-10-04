use amigo_engine::prelude::*;

mod data;
mod player;
mod states;

fn main() {
    // Embedded so the game has controls wherever it is started from; an
    // input.ron in the working directory still overrides it.
    let bindings = ActionBindings::from_ron(include_str!("../input.ron"))
        .expect("examples/starter/input.ron is valid");

    Engine::build()
        .title("Amigo Starter")
        .input_bindings(bindings)
        .virtual_resolution(640, 360)
        .window_size(1280, 720)
        .build()
        // Each state is its own `Game`; LoadingState replaces itself with the
        // menu, which pushes gameplay, which pops back. No hand-rolled state
        // enum — the engine owns the stack.
        .run(states::LoadingState::new());
}

#[cfg(test)]
mod tests {
    use amigo_engine::prelude::*;

    #[test]
    fn input_ron_parses_and_every_binding_can_fire() {
        let bindings =
            ActionBindings::from_ron(include_str!("../input.ron")).expect("input.ron parses");
        assert_eq!(bindings.unknown_inputs(), Vec::new());
        for action in [
            "move_up",
            "move_down",
            "move_left",
            "move_right",
            "confirm",
            "pause",
        ] {
            assert!(
                !bindings.get_bindings(action).is_empty(),
                "the game reads action '{action}'"
            );
        }
    }
}
