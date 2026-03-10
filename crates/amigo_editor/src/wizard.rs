use amigo_core::game_preset::{
    project_templates, GameProject, ProjectTemplate, SceneDef, ScenePreset, SceneTransition,
    TransitionTrigger,
};

// ---------------------------------------------------------------------------
// New Project Wizard — step-by-step UI for creating a game project
// ---------------------------------------------------------------------------

/// Wizard steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WizardStep {
    /// Choose a project template.
    ChooseTemplate,
    /// Name the project and set resolution.
    ProjectSettings,
    /// Add additional scenes (optional).
    AddScenes,
    /// Review and confirm.
    Review,
    /// Wizard completed — project is ready.
    Done,
    /// Wizard was cancelled.
    Cancelled,
}

/// The new-project wizard state machine.
pub struct NewProjectWizard {
    pub step: WizardStep,
    pub templates: Vec<ProjectTemplate>,
    pub selected_template: usize,
    pub project_name: String,
    pub custom_width: u32,
    pub custom_height: u32,
    /// Extra scenes the user wants to add.
    pub extra_scenes: Vec<ExtraSceneEntry>,
    /// The resulting project (set on Done).
    pub result: Option<GameProject>,
    /// Cursor for text input.
    pub name_cursor: usize,
    /// Which extra scene is being edited (-1 = none).
    pub editing_scene: Option<usize>,
    /// Preset selected for a new extra scene.
    pub new_scene_preset: usize,
}

/// An extra scene the user adds in step 3.
#[derive(Clone, Debug)]
pub struct ExtraSceneEntry {
    pub name: String,
    pub preset: ScenePreset,
}

impl NewProjectWizard {
    pub fn new() -> Self {
        let templates = project_templates();
        Self {
            step: WizardStep::ChooseTemplate,
            templates,
            selected_template: 0,
            project_name: "My Game".to_string(),
            custom_width: 320,
            custom_height: 180,
            extra_scenes: Vec::new(),
            result: None,
            name_cursor: 7, // "My Game".len()
            editing_scene: None,
            new_scene_preset: 0,
        }
    }

    /// Get the currently selected template.
    pub fn current_template(&self) -> &ProjectTemplate {
        &self.templates[self.selected_template]
    }

    /// Move to the next step.
    pub fn next(&mut self) {
        self.step = match self.step {
            WizardStep::ChooseTemplate => {
                // Apply template resolution
                let t = &self.templates[self.selected_template];
                self.custom_width = t.resolution.0;
                self.custom_height = t.resolution.1;
                WizardStep::ProjectSettings
            }
            WizardStep::ProjectSettings => WizardStep::AddScenes,
            WizardStep::AddScenes => WizardStep::Review,
            WizardStep::Review => {
                self.finalize();
                WizardStep::Done
            }
            WizardStep::Done | WizardStep::Cancelled => self.step,
        };
    }

    /// Move to the previous step.
    pub fn back(&mut self) {
        self.step = match self.step {
            WizardStep::ChooseTemplate => WizardStep::ChooseTemplate,
            WizardStep::ProjectSettings => WizardStep::ChooseTemplate,
            WizardStep::AddScenes => WizardStep::ProjectSettings,
            WizardStep::Review => WizardStep::AddScenes,
            WizardStep::Done | WizardStep::Cancelled => self.step,
        };
    }

    /// Cancel the wizard.
    pub fn cancel(&mut self) {
        self.step = WizardStep::Cancelled;
    }

    /// Select a template by index.
    pub fn select_template(&mut self, index: usize) {
        if index < self.templates.len() {
            self.selected_template = index;
        }
    }

    /// Add an extra scene.
    pub fn add_extra_scene(&mut self, name: String, preset: ScenePreset) {
        self.extra_scenes.push(ExtraSceneEntry { name, preset });
    }

    /// Remove an extra scene by index.
    pub fn remove_extra_scene(&mut self, index: usize) {
        if index < self.extra_scenes.len() {
            self.extra_scenes.remove(index);
        }
    }

    /// Type a character into the project name.
    pub fn type_char(&mut self, ch: char) {
        if self.project_name.len() < 40 && ch.is_ascii_graphic() || ch == ' ' {
            self.project_name.push(ch);
        }
    }

    /// Delete last character from the project name.
    pub fn backspace(&mut self) {
        self.project_name.pop();
    }

    /// Build the final project from wizard state.
    fn finalize(&mut self) {
        let template = &self.templates[self.selected_template];
        let mut project = template.create_project(&self.project_name);
        project.virtual_width = self.custom_width;
        project.virtual_height = self.custom_height;

        // Add extra scenes
        for (i, extra) in self.extra_scenes.iter().enumerate() {
            let id = format!("scene_{}", i);
            let scene = SceneDef::new(&id, &extra.name, extra.preset);
            project.add_scene(scene);
        }

        // For Turn-Based RPG template, add a battle scene linked from gameplay
        if template.primary_preset == ScenePreset::TopDown
            && template.name == "Turn-Based RPG"
        {
            project.add_scene(SceneDef::new("battle", "Battle", ScenePreset::TurnBased));
            if let Some(gameplay) = project.find_scene_mut("gameplay") {
                gameplay.transitions.push(SceneTransition {
                    target: "battle".to_string(),
                    trigger: TransitionTrigger::Event {
                        event_name: "enemy_contact".to_string(),
                    },
                    effect: "fade".to_string(),
                });
            }
        }

        self.result = Some(project);
    }

    /// Get step number for progress display (1-based).
    pub fn step_number(&self) -> u32 {
        match self.step {
            WizardStep::ChooseTemplate => 1,
            WizardStep::ProjectSettings => 2,
            WizardStep::AddScenes => 3,
            WizardStep::Review => 4,
            WizardStep::Done | WizardStep::Cancelled => 4,
        }
    }

    /// Total number of steps.
    pub fn total_steps(&self) -> u32 {
        4
    }

    /// Is the wizard active (not done or cancelled)?
    pub fn is_active(&self) -> bool {
        !matches!(self.step, WizardStep::Done | WizardStep::Cancelled)
    }

    /// Summary lines for the review step.
    pub fn review_summary(&self) -> Vec<String> {
        let template = &self.templates[self.selected_template];
        let mut lines = vec![
            format!("Project: {}", self.project_name),
            format!("Template: {}", template.name),
            format!("Resolution: {}x{}", self.custom_width, self.custom_height),
            format!("Primary: {}", template.primary_preset.display_name()),
            String::new(),
            "Scenes:".to_string(),
            format!("  - Title Menu (Menu)"),
            format!("  - Gameplay ({})", template.primary_preset.display_name()),
            format!("  - Pause Menu (Menu)"),
        ];

        for extra in &self.extra_scenes {
            lines.push(format!("  - {} ({})", extra.name, extra.preset.display_name()));
        }

        lines.push(String::new());
        lines.push(format!("Systems: {}", template.primary_preset.default_systems().join(", ")));

        lines
    }
}

impl Default for NewProjectWizard {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wizard_basic_flow() {
        let mut wiz = NewProjectWizard::new();
        assert_eq!(wiz.step, WizardStep::ChooseTemplate);
        assert!(wiz.is_active());

        // Step 1: choose template
        wiz.select_template(0); // Platformer
        wiz.next();
        assert_eq!(wiz.step, WizardStep::ProjectSettings);

        // Step 2: name + resolution
        wiz.project_name = "Cool Game".to_string();
        wiz.next();
        assert_eq!(wiz.step, WizardStep::AddScenes);

        // Step 3: add an extra scene
        wiz.add_extra_scene("Boss Arena".to_string(), ScenePreset::Fighting);
        assert_eq!(wiz.extra_scenes.len(), 1);
        wiz.next();
        assert_eq!(wiz.step, WizardStep::Review);

        // Step 4: review + confirm
        let summary = wiz.review_summary();
        assert!(summary.iter().any(|l| l.contains("Cool Game")));
        assert!(summary.iter().any(|l| l.contains("Boss Arena")));

        wiz.next();
        assert_eq!(wiz.step, WizardStep::Done);
        assert!(!wiz.is_active());

        let project = wiz.result.as_ref().unwrap();
        assert_eq!(project.name, "Cool Game");
        assert!(project.find_scene("title_menu").is_some());
        assert!(project.find_scene("gameplay").is_some());
        assert!(project.find_scene("scene_0").is_some()); // Boss Arena
    }

    #[test]
    fn wizard_back_navigation() {
        let mut wiz = NewProjectWizard::new();
        wiz.next(); // → ProjectSettings
        wiz.next(); // → AddScenes
        wiz.back(); // → ProjectSettings
        assert_eq!(wiz.step, WizardStep::ProjectSettings);
        wiz.back(); // → ChooseTemplate
        assert_eq!(wiz.step, WizardStep::ChooseTemplate);
        wiz.back(); // stays at ChooseTemplate
        assert_eq!(wiz.step, WizardStep::ChooseTemplate);
    }

    #[test]
    fn wizard_cancel() {
        let mut wiz = NewProjectWizard::new();
        wiz.next();
        wiz.cancel();
        assert_eq!(wiz.step, WizardStep::Cancelled);
        assert!(!wiz.is_active());
        assert!(wiz.result.is_none());
    }

    #[test]
    fn wizard_text_input() {
        let mut wiz = NewProjectWizard::new();
        wiz.project_name.clear();
        wiz.type_char('H');
        wiz.type_char('i');
        assert_eq!(wiz.project_name, "Hi");
        wiz.backspace();
        assert_eq!(wiz.project_name, "H");
    }

    #[test]
    fn wizard_turn_based_rpg_adds_battle() {
        let mut wiz = NewProjectWizard::new();
        // Find Turn-Based RPG template
        let idx = wiz.templates.iter().position(|t| t.name == "Turn-Based RPG").unwrap();
        wiz.select_template(idx);
        wiz.next(); // settings
        wiz.next(); // scenes
        wiz.next(); // review
        wiz.next(); // done

        let project = wiz.result.as_ref().unwrap();
        assert!(project.find_scene("battle").is_some(), "Should auto-add battle scene");
        let gameplay = project.find_scene("gameplay").unwrap();
        assert!(
            gameplay.transitions.iter().any(|t| t.target == "battle"),
            "Gameplay should transition to battle"
        );
    }

    #[test]
    fn wizard_step_numbers() {
        let mut wiz = NewProjectWizard::new();
        assert_eq!(wiz.step_number(), 1);
        wiz.next();
        assert_eq!(wiz.step_number(), 2);
        wiz.next();
        assert_eq!(wiz.step_number(), 3);
        wiz.next();
        assert_eq!(wiz.step_number(), 4);
    }

    #[test]
    fn remove_extra_scene() {
        let mut wiz = NewProjectWizard::new();
        wiz.add_extra_scene("A".to_string(), ScenePreset::Puzzle);
        wiz.add_extra_scene("B".to_string(), ScenePreset::Fighting);
        assert_eq!(wiz.extra_scenes.len(), 2);
        wiz.remove_extra_scene(0);
        assert_eq!(wiz.extra_scenes.len(), 1);
        assert_eq!(wiz.extra_scenes[0].name, "B");
    }
}
