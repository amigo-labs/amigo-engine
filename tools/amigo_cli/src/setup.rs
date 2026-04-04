use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolGroup {
    Audio,
    ArtGen,
    MusicGen,
    All,
}

impl ToolGroup {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "audio" => Some(Self::Audio),
            "artgen" | "art-gen" => Some(Self::ArtGen),
            "music-gen" | "musicgen" => Some(Self::MusicGen),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    fn groups(self) -> Vec<ToolGroup> {
        match self {
            Self::All => vec![Self::Audio, Self::ArtGen, Self::MusicGen],
            other => vec![other],
        }
    }

    fn _requirement_file(self) -> &'static str {
        match self {
            Self::Audio => REQUIREMENTS_AUDIO,
            Self::ArtGen => REQUIREMENTS_ARTGEN,
            Self::MusicGen => REQUIREMENTS_MUSICGEN,
            Self::All => REQUIREMENTS_AUDIO, // caller iterates groups
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Audio => "Audio (Demucs, Basic Pitch)",
            Self::ArtGen => "ArtGen (Qwen-Image, FLUX.2 Klein, ComfyUI)",
            Self::MusicGen => "MusicGen (ACE-Step, AudioGen)",
            Self::All => "All Tools",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    Cpu,
    Nvidia,
    Mps,
}

impl GpuBackend {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "cpu" => Some(Self::Cpu),
            "nvidia" | "cuda" => Some(Self::Nvidia),
            "mps" | "metal" => Some(Self::Mps),
            _ => None,
        }
    }

    fn pytorch_index_url(self) -> &'static str {
        match self {
            Self::Cpu => "https://download.pytorch.org/whl/cpu",
            Self::Nvidia => "https://download.pytorch.org/whl/cu124",
            Self::Mps => "https://download.pytorch.org/whl/cpu", // Metal uses standard PyPI
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetupConfig {
    pub amigo_home: PathBuf,
    pub groups: Vec<ToolGroup>,
    pub gpu: GpuBackend,
    pub python_version: String,
}

impl Default for SetupConfig {
    fn default() -> Self {
        Self {
            amigo_home: default_amigo_home(),
            groups: vec![ToolGroup::All],
            gpu: GpuBackend::Cpu,
            python_version: "3.11".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub group: ToolGroup,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetupResult {
    pub _tools: Vec<ToolStatus>,
    pub _venv_path: PathBuf,
    pub _python_version: String,
}

// ---------------------------------------------------------------------------
// SetupManager
// ---------------------------------------------------------------------------

pub struct SetupManager {
    config: SetupConfig,
}

impl SetupManager {
    pub fn new(config: SetupConfig) -> Self {
        Self { config }
    }

    fn uv_path(&self) -> PathBuf {
        let bin = if cfg!(windows) { "uv.exe" } else { "uv" };
        self.config.amigo_home.join("bin").join(bin)
    }

    fn venv_path(&self) -> PathBuf {
        self.config.amigo_home.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        if cfg!(windows) {
            self.venv_path().join("Scripts").join("python.exe")
        } else {
            self.venv_path().join("bin").join("python")
        }
    }

    fn requirements_dir(&self) -> PathBuf {
        self.config.amigo_home.join("requirements")
    }

    pub fn has_uv(&self) -> bool {
        self.uv_path().exists()
    }

    pub fn has_venv(&self) -> bool {
        self.venv_python().exists()
    }

    /// Install uv binary to ~/.amigo/bin/.
    pub fn install_uv(&self) -> Result<(), String> {
        let uv_path = self.uv_path();
        if uv_path.exists() {
            println!("  uv already installed at {}", uv_path.display());
            return Ok(());
        }

        let bin_dir = self.config.amigo_home.join("bin");
        std::fs::create_dir_all(&bin_dir).map_err(|e| format!("mkdir failed: {e}"))?;

        println!("  Downloading uv...");

        if cfg!(windows) {
            // Use PowerShell to download uv.
            let status = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &format!(
                        "irm https://astral.sh/uv/install.ps1 | iex; \
                         Move-Item -Force (Get-Command uv).Source '{}'",
                        uv_path.display()
                    ),
                ])
                .status()
                .map_err(|e| format!("PowerShell failed: {e}"))?;

            if !status.success() {
                // Fallback: try direct download.
                let url = "https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip";
                return Err(format!(
                    "uv installation failed. Download manually from: {url}"
                ));
            }
        } else {
            let status = Command::new("sh")
                .args([
                    "-c",
                    &format!(
                        "curl -fsSL https://astral.sh/uv/install.sh | \
                         UV_INSTALL_DIR='{}' sh",
                        bin_dir.display()
                    ),
                ])
                .status()
                .map_err(|e| format!("curl failed: {e}"))?;

            if !status.success() {
                return Err("uv installation failed".into());
            }
        }

        if !uv_path.exists() {
            return Err(format!(
                "uv not found at {} after installation",
                uv_path.display()
            ));
        }

        println!("  uv installed successfully");
        Ok(())
    }

    /// Install Python via uv.
    pub fn install_python(&self) -> Result<(), String> {
        let uv = self.uv_path();
        println!("  Installing Python {}...", self.config.python_version);

        let output = Command::new(&uv)
            .args(["python", "install", &self.config.python_version])
            .output()
            .map_err(|e| format!("uv python install failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Python install failed: {stderr}"));
        }

        println!("  Python {} installed", self.config.python_version);
        Ok(())
    }

    /// Create isolated venv.
    pub fn create_venv(&self) -> Result<(), String> {
        let uv = self.uv_path();
        let venv = self.venv_path();

        if self.has_venv() {
            println!("  venv already exists at {}", venv.display());
            return Ok(());
        }

        println!("  Creating virtual environment...");

        let output = Command::new(&uv)
            .args([
                "venv",
                &venv.display().to_string(),
                "--python",
                &self.config.python_version,
            ])
            .output()
            .map_err(|e| format!("uv venv failed: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("venv creation failed: {stderr}"));
        }

        println!("  venv created at {}", venv.display());
        Ok(())
    }

    /// Write embedded requirement files to disk.
    fn write_requirements(&self) -> Result<(), String> {
        let dir = self.requirements_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {e}"))?;

        let core_content =
            REQUIREMENTS_CORE.replace("{{PYTORCH_INDEX}}", self.config.gpu.pytorch_index_url());
        write_file(&dir.join("core.txt"), &core_content)?;
        write_file(&dir.join("audio.txt"), REQUIREMENTS_AUDIO)?;
        write_file(&dir.join("artgen.txt"), REQUIREMENTS_ARTGEN)?;
        write_file(&dir.join("music-gen.txt"), REQUIREMENTS_MUSICGEN)?;

        Ok(())
    }

    /// Install packages for a tool group.
    pub fn install_packages(&self, group: ToolGroup) -> Result<(), String> {
        let uv = self.uv_path();
        let req_dir = self.requirements_dir();

        for g in group.groups() {
            let req_file = match g {
                ToolGroup::Audio => "audio.txt",
                ToolGroup::ArtGen => "artgen.txt",
                ToolGroup::MusicGen => "music-gen.txt",
                ToolGroup::All => unreachable!(),
            };
            let req_path = req_dir.join(req_file);

            println!("  Installing {}...", g.display_name());

            let output = Command::new(&uv)
                .args([
                    "pip",
                    "install",
                    "--python",
                    &self.venv_python().display().to_string(),
                    "-r",
                    &req_path.display().to_string(),
                ])
                .output()
                .map_err(|e| format!("pip install failed: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!(
                    "Package install for {} failed: {stderr}",
                    g.display_name()
                ));
            }
        }

        Ok(())
    }

    /// Verify that tools are installed by running import checks.
    pub fn verify(&self) -> Vec<ToolStatus> {
        let tools: Vec<(&str, ToolGroup, &str)> = vec![
            (
                "demucs",
                ToolGroup::Audio,
                "import demucs; print(demucs.__version__)",
            ),
            (
                "basic-pitch",
                ToolGroup::Audio,
                "import basic_pitch; print(basic_pitch.__version__)",
            ),
            (
                "midi_to_tidalcycles",
                ToolGroup::Audio,
                "import midi_to_tidalcycles; print('ok')",
            ),
            ("comfyui", ToolGroup::ArtGen, "import comfy; print('ok')"),
            (
                "audiocraft",
                ToolGroup::MusicGen,
                "import audiocraft; print(audiocraft.__version__)",
            ),
        ];

        let uv = self.uv_path();
        let python = self.venv_python();

        tools
            .into_iter()
            .map(|(name, group, check)| {
                let result = Command::new(&uv)
                    .args([
                        "run",
                        "--python",
                        &python.display().to_string(),
                        "python",
                        "-c",
                        check,
                    ])
                    .output();

                let (installed, version) = match result {
                    Ok(output) if output.status.success() => {
                        let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        (true, if ver == "ok" { None } else { Some(ver) })
                    }
                    _ => (false, None),
                };

                ToolStatus {
                    name: name.to_string(),
                    group,
                    installed,
                    version,
                }
            })
            .collect()
    }

    /// Run the full setup process.
    pub fn run_full_setup(&self) -> Result<SetupResult, String> {
        println!("\nAmigo Setup");
        println!("{}", "=".repeat(50));

        // Step 1: Install uv.
        println!("\n[1/5] Installing uv...");
        self.install_uv()?;

        // Step 2: Install Python.
        println!("\n[2/5] Installing Python...");
        self.install_python()?;

        // Step 3: Create venv.
        println!("\n[3/5] Creating virtual environment...");
        self.create_venv()?;

        // Step 4: Write requirements and install packages.
        println!("\n[4/5] Installing packages...");
        self.write_requirements()?;
        for group in &self.config.groups {
            self.install_packages(*group)?;
        }

        // Step 5: Verify.
        println!("\n[5/5] Verifying installation...");
        let tools = self.verify();
        let installed_count = tools.iter().filter(|t| t.installed).count();
        println!("  {installed_count}/{} tools verified", tools.len());

        // Write config.toml.
        self.write_config_toml(&tools)?;

        println!("\n{}", "=".repeat(50));
        println!("Setup complete!");
        println!("  venv: {}", self.venv_path().display());

        Ok(SetupResult {
            _tools: tools,
            _venv_path: self.venv_path(),
            _python_version: self.config.python_version.clone(),
        })
    }

    /// Write setup status to config.toml.
    fn write_config_toml(&self, tools: &[ToolStatus]) -> Result<(), String> {
        let config_path = self.config.amigo_home.join("config.toml");
        let now = chrono_lite_now();

        let groups = &self.config.groups;
        let has_audio = groups
            .iter()
            .any(|g| matches!(g, ToolGroup::Audio | ToolGroup::All));
        let has_artgen = groups
            .iter()
            .any(|g| matches!(g, ToolGroup::ArtGen | ToolGroup::All));
        let has_musicgen = groups
            .iter()
            .any(|g| matches!(g, ToolGroup::MusicGen | ToolGroup::All));

        let gpu_str = match self.config.gpu {
            GpuBackend::Cpu => "cpu",
            GpuBackend::Nvidia => "nvidia",
            GpuBackend::Mps => "mps",
        };

        let mut toml = format!(
            r#"[setup]
version = "0.1.0"
installed_at = "{now}"
python_version = "{}"
gpu_backend = "{gpu_str}"

[groups]
audio = {has_audio}
artgen = {has_artgen}
music_gen = {has_musicgen}

[tools]
"#,
            self.config.python_version,
        );

        for tool in tools {
            let ver = tool
                .version
                .as_deref()
                .map(|v| format!(", version = \"{v}\""))
                .unwrap_or_default();
            toml.push_str(&format!(
                "{} = {{ installed = {}{} }}\n",
                tool.name.replace('-', "_"),
                tool.installed,
                ver,
            ));
        }

        std::fs::write(&config_path, &toml).map_err(|e| format!("write config.toml: {e}"))?;
        Ok(())
    }

    /// Print status of all tools.
    pub fn print_status(&self) {
        println!("\nAmigo Python Toolchain Status");
        println!("{}", "-".repeat(50));

        let uv_status = if self.has_uv() {
            "installed"
        } else {
            "not installed"
        };
        let venv_status = if self.has_venv() {
            "created"
        } else {
            "not created"
        };

        println!("  uv:     {uv_status} ({})", self.uv_path().display());
        println!("  venv:   {venv_status} ({})", self.venv_path().display());

        let gpu_str = match self.config.gpu {
            GpuBackend::Cpu => "CPU-only",
            GpuBackend::Nvidia => "NVIDIA CUDA",
            GpuBackend::Mps => "macOS Metal",
        };
        println!("  GPU:    {gpu_str}");

        if self.has_venv() {
            let tools = self.verify();
            println!();
            let mut current_group = None;
            for tool in &tools {
                if current_group != Some(tool.group) {
                    current_group = Some(tool.group);
                    println!("  {}:", tool.group.display_name());
                }
                let icon = if tool.installed { "+" } else { "-" };
                let ver = tool.version.as_deref().unwrap_or(if tool.installed {
                    "ok"
                } else {
                    "not installed"
                });
                println!("    [{icon}] {:<20} {ver}", tool.name);
            }
        }

        println!("{}", "-".repeat(50));
    }

    /// Clean up: remove venv, python, cache.
    pub fn clean(&self, all: bool) -> Result<(), String> {
        let dirs_to_remove: Vec<PathBuf> = vec![
            self.venv_path(),
            self.config.amigo_home.join("python"),
            self.config.amigo_home.join("cache"),
            self.requirements_dir(),
        ];

        for dir in &dirs_to_remove {
            if dir.exists() {
                println!("  Removing {}...", dir.display());
                std::fs::remove_dir_all(dir).map_err(|e| format!("remove failed: {e}"))?;
            }
        }

        if all {
            let bin_dir = self.config.amigo_home.join("bin");
            if bin_dir.exists() {
                println!("  Removing {}...", bin_dir.display());
                std::fs::remove_dir_all(&bin_dir).map_err(|e| format!("remove failed: {e}"))?;
            }
        }

        // Remove config.toml.
        let config_path = self.config.amigo_home.join("config.toml");
        if config_path.exists() {
            std::fs::remove_file(&config_path).map_err(|e| format!("remove failed: {e}"))?;
        }

        println!("  Cleanup complete");
        Ok(())
    }

    /// Run a command inside the venv using uv run.
    pub fn _run_in_venv(&self, cmd: &str, args: &[&str]) -> Result<std::process::Output, String> {
        if !self.has_venv() {
            return Err("Python venv not found. Run `amigo setup` first.".into());
        }

        let uv = self.uv_path();
        let python = self.venv_python();

        let mut command = Command::new(&uv);
        command.args(["run", "--python", &python.display().to_string(), cmd]);
        command.args(args);

        command.output().map_err(|e| format!("{cmd} failed: {e}"))
    }
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

pub fn cmd_setup(args: &[String]) {
    let mut config = SetupConfig::default();
    let mut check = false;
    let mut clean = false;
    let mut clean_all = false;
    let mut update = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--only" => {
                i += 1;
                if i < args.len() {
                    if let Some(group) = ToolGroup::from_str(&args[i]) {
                        config.groups = vec![group];
                    } else {
                        eprintln!("Unknown tool group: {}", args[i]);
                        eprintln!("Valid groups: audio, artgen, music-gen, all");
                        std::process::exit(1);
                    }
                }
            }
            "--gpu" => {
                i += 1;
                if i < args.len() {
                    if let Some(gpu) = GpuBackend::from_str(&args[i]) {
                        config.gpu = gpu;
                    } else {
                        eprintln!("Unknown GPU backend: {}", args[i]);
                        eprintln!("Valid backends: cpu, nvidia, mps");
                        std::process::exit(1);
                    }
                }
            }
            "--python" => {
                i += 1;
                if i < args.len() {
                    config.python_version = args[i].clone();
                }
            }
            "--check" => check = true,
            "--clean" => clean = true,
            "--all" => clean_all = true,
            "--update" => update = true,
            other => {
                eprintln!("Unknown option: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let manager = SetupManager::new(config);

    if check {
        manager.print_status();
    } else if clean {
        if let Err(e) = manager.clean(clean_all) {
            eprintln!("Clean failed: {e}");
            std::process::exit(1);
        }
    } else if update {
        println!("Updating packages...");
        if let Err(e) = manager.write_requirements() {
            eprintln!("Failed to write requirements: {e}");
            std::process::exit(1);
        }
        for group in &manager.config.groups {
            if let Err(e) = manager.install_packages(*group) {
                eprintln!("Update failed: {e}");
                std::process::exit(1);
            }
        }
        println!("Update complete");
    } else {
        match manager.run_full_setup() {
            Ok(_result) => {}
            Err(e) => {
                eprintln!("\nSetup failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Embedded requirement files
// ---------------------------------------------------------------------------

const REQUIREMENTS_CORE: &str = r#"--index-url {{PYTORCH_INDEX}}
torch>=2.2.0
torchaudio>=2.2.0
numpy>=1.24.0
"#;

const REQUIREMENTS_AUDIO: &str = r#"-r core.txt
demucs>=4.0.0
basic-pitch>=0.3.0
midi_to_tidalcycles>=0.2.0
pretty-midi>=0.2.10
librosa>=0.10.0
soundfile>=0.12.0
"#;

const REQUIREMENTS_ARTGEN: &str = r#"-r core.txt
comfyui>=0.2.0
# Qwen-Image support
transformers>=4.40.0
accelerate>=0.30.0
# FLUX.2 Klein support (uses diffusers pipeline)
diffusers>=0.28.0
safetensors>=0.4.0
"#;

const REQUIREMENTS_MUSICGEN: &str = r#"-r core.txt
audiocraft>=1.3.0
"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_amigo_home() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default"))
            .join(".amigo")
    }
    #[cfg(not(windows))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join(".amigo")
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    let mut f =
        std::fs::File::create(path).map_err(|e| format!("create {}: {e}", path.display()))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

/// Simple ISO-8601 timestamp without chrono dependency.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Rough conversion — good enough for a timestamp.
    let days = secs / 86400;
    let years_approx = 1970 + days / 365;
    format!("{years_approx}-01-01T00:00:00Z")
}
