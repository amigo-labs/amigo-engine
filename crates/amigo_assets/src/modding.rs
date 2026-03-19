//! Modding framework: data-driven content override system.
//!
//! Mods are directories with a `mod.toml` manifest. Assets in mods override
//! base game assets by path. RON data files support replace and extend modes.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// ModManifest
// ---------------------------------------------------------------------------

/// Parsed from `mod.toml` in each mod directory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub engine_version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub priority: i32,
}

// ---------------------------------------------------------------------------
// ModInfo
// ---------------------------------------------------------------------------

/// Runtime state for a discovered mod.
#[derive(Clone, Debug)]
pub struct ModInfo {
    pub manifest: ModManifest,
    pub path: PathBuf,
    pub active: bool,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Mod system errors.
#[derive(Debug)]
pub enum ModError {
    DirectoryNotFound(PathBuf),
    InvalidManifest {
        mod_name: String,
        reason: String,
    },
    VersionMismatch {
        mod_name: String,
        required: String,
        actual: String,
    },
    MissingDependency {
        mod_name: String,
        dependency: String,
    },
    CircularDependency {
        cycle: Vec<String>,
    },
}

impl std::fmt::Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectoryNotFound(p) => write!(f, "Mod directory not found: {}", p.display()),
            Self::InvalidManifest { mod_name, reason } => {
                write!(f, "Invalid mod.toml in {mod_name}: {reason}")
            }
            Self::VersionMismatch {
                mod_name,
                required,
                actual,
            } => write!(
                f,
                "Engine version mismatch for {mod_name}: requires {required}, got {actual}"
            ),
            Self::MissingDependency {
                mod_name,
                dependency,
            } => write!(f, "Missing dependency: {mod_name} requires {dependency}"),
            Self::CircularDependency { cycle } => write!(f, "Circular dependency: {:?}", cycle),
        }
    }
}

impl std::error::Error for ModError {}

// ---------------------------------------------------------------------------
// DataOverrideMode
// ---------------------------------------------------------------------------

/// How a mod's RON data file interacts with the base data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataOverrideMode {
    Replace,
    Extend,
}

// ---------------------------------------------------------------------------
// ModReport
// ---------------------------------------------------------------------------

/// Summary of what a mod application changed.
#[derive(Clone, Debug, Default)]
pub struct ModReport {
    pub overridden_assets: Vec<String>,
    pub replaced_data: Vec<String>,
    pub extended_data: Vec<String>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// ModManager
// ---------------------------------------------------------------------------

/// Central mod management system.
pub struct ModManager {
    mods_dir: PathBuf,
    discovered: Vec<ModInfo>,
    load_order: Vec<usize>,
    engine_version: String,
}

impl ModManager {
    pub fn new(mods_dir: PathBuf, engine_version: &str) -> Self {
        Self {
            mods_dir,
            discovered: Vec::new(),
            load_order: Vec::new(),
            engine_version: engine_version.to_string(),
        }
    }

    /// Scan mods directory for mod.toml manifests.
    pub fn discover(&mut self) -> Result<usize, ModError> {
        self.discovered.clear();
        if !self.mods_dir.exists() {
            return Ok(0);
        }

        let entries = std::fs::read_dir(&self.mods_dir)
            .map_err(|_| ModError::DirectoryNotFound(self.mods_dir.clone()))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("mod.toml");
            if !manifest_path.exists() {
                self.discovered.push(ModInfo {
                    manifest: ModManifest {
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                        version: "0.0.0".into(),
                        author: "unknown".into(),
                        engine_version: "*".into(),
                        description: None,
                        dependencies: Vec::new(),
                        priority: 0,
                    },
                    path: path.clone(),
                    active: false,
                    errors: vec!["mod.toml not found".into()],
                });
                continue;
            }

            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match toml::from_str::<ModManifest>(&content) {
                    Ok(manifest) => {
                        self.discovered.push(ModInfo {
                            manifest,
                            path: path.clone(),
                            active: false,
                            errors: Vec::new(),
                        });
                    }
                    Err(e) => {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        self.discovered.push(ModInfo {
                            manifest: ModManifest {
                                name: name.clone(),
                                version: "0.0.0".into(),
                                author: "unknown".into(),
                                engine_version: "*".into(),
                                description: None,
                                dependencies: Vec::new(),
                                priority: 0,
                            },
                            path: path.clone(),
                            active: false,
                            errors: vec![format!("TOML parse error: {e}")],
                        });
                    }
                },
                Err(e) => {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    self.discovered.push(ModInfo {
                        manifest: ModManifest {
                            name,
                            version: "0.0.0".into(),
                            author: "unknown".into(),
                            engine_version: "*".into(),
                            description: None,
                            dependencies: Vec::new(),
                            priority: 0,
                        },
                        path: path.clone(),
                        active: false,
                        errors: vec![format!("IO error: {e}")],
                    });
                }
            }
        }

        Ok(self.discovered.len())
    }

    pub fn discovered(&self) -> &[ModInfo] {
        &self.discovered
    }

    /// Activate a mod by name.
    pub fn activate(&mut self, name: &str) -> Result<(), ModError> {
        let idx = self.find_mod(name)?;

        // Check for errors
        if !self.discovered[idx].errors.is_empty() {
            return Err(ModError::InvalidManifest {
                mod_name: name.to_string(),
                reason: self.discovered[idx].errors.join("; "),
            });
        }

        // Check engine version compatibility (simple: "*" matches all)
        let required = &self.discovered[idx].manifest.engine_version;
        if required != "*" && !version_compatible(required, &self.engine_version) {
            return Err(ModError::VersionMismatch {
                mod_name: name.to_string(),
                required: required.clone(),
                actual: self.engine_version.clone(),
            });
        }

        // Check dependencies
        let deps = self.discovered[idx].manifest.dependencies.clone();
        for dep in &deps {
            let dep_idx = self.discovered.iter().position(|m| m.manifest.name == *dep);
            match dep_idx {
                Some(di) if self.discovered[di].active => {}
                _ => {
                    return Err(ModError::MissingDependency {
                        mod_name: name.to_string(),
                        dependency: dep.clone(),
                    });
                }
            }
        }

        self.discovered[idx].active = true;
        Ok(())
    }

    /// Deactivate a mod and cascade to dependents.
    pub fn deactivate(&mut self, name: &str) -> Result<Vec<String>, ModError> {
        let idx = self.find_mod(name)?;
        self.discovered[idx].active = false;

        // Find and deactivate mods that depend on this one
        let mut cascaded = Vec::new();
        loop {
            let mut found = false;
            for i in 0..self.discovered.len() {
                if !self.discovered[i].active {
                    continue;
                }
                if self.discovered[i]
                    .manifest
                    .dependencies
                    .contains(&name.to_string())
                {
                    self.discovered[i].active = false;
                    cascaded.push(self.discovered[i].manifest.name.clone());
                    found = true;
                }
            }
            if !found {
                break;
            }
        }

        Ok(cascaded)
    }

    /// Compute load order via topological sort.
    pub fn compute_load_order(&mut self) -> Result<(), ModError> {
        let active: Vec<usize> = self
            .discovered
            .iter()
            .enumerate()
            .filter(|(_, m)| m.active)
            .map(|(i, _)| i)
            .collect();

        let n = active.len();
        let mut in_degree = vec![0u32; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        // Build adjacency list
        for (local_i, &global_i) in active.iter().enumerate() {
            for dep in &self.discovered[global_i].manifest.dependencies {
                if let Some(local_j) = active
                    .iter()
                    .position(|&gi| self.discovered[gi].manifest.name == *dep)
                {
                    adj[local_j].push(local_i); // dep -> dependent
                    in_degree[local_i] += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<usize> = in_degree
            .iter()
            .enumerate()
            .filter(|(_, &d)| d == 0)
            .map(|(i, _)| i)
            .collect();
        let mut order = Vec::new();

        while let Some(node) = queue.pop() {
            order.push(node);
            for &next in &adj[node] {
                in_degree[next] -= 1;
                if in_degree[next] == 0 {
                    queue.push(next);
                }
            }
        }

        if order.len() < n {
            // Circular dependency detected
            let cycle: Vec<String> = (0..n)
                .filter(|i| in_degree[*i] > 0)
                .map(|i| self.discovered[active[i]].manifest.name.clone())
                .collect();
            return Err(ModError::CircularDependency { cycle });
        }

        // Sort within dependency tiers by priority then name
        order.sort_by(|&a, &b| {
            let pa = self.discovered[active[a]].manifest.priority;
            let pb = self.discovered[active[b]].manifest.priority;
            pa.cmp(&pb).then_with(|| {
                self.discovered[active[a]]
                    .manifest
                    .name
                    .cmp(&self.discovered[active[b]].manifest.name)
            })
        });

        self.load_order = order.into_iter().map(|i| active[i]).collect();
        Ok(())
    }

    pub fn load_order(&self) -> Vec<&str> {
        self.load_order
            .iter()
            .map(|&i| self.discovered[i].manifest.name.as_str())
            .collect()
    }

    pub fn is_active(&self, name: &str) -> bool {
        self.discovered
            .iter()
            .any(|m| m.manifest.name == name && m.active)
    }

    pub fn set_priority(&mut self, name: &str, priority: i32) {
        if let Some(m) = self.discovered.iter_mut().find(|m| m.manifest.name == name) {
            m.manifest.priority = priority;
        }
    }

    fn find_mod(&self, name: &str) -> Result<usize, ModError> {
        self.discovered
            .iter()
            .position(|m| m.manifest.name == name)
            .ok_or_else(|| ModError::DirectoryNotFound(self.mods_dir.join(name)))
    }
}

// ---------------------------------------------------------------------------
// SemVer helpers (minimal)
// ---------------------------------------------------------------------------

fn version_compatible(range: &str, actual: &str) -> bool {
    // Simple implementation: check if actual starts with range prefix
    // For real SemVer ranges, use a proper parser
    if range == "*" {
        return true;
    }
    // Handle ">=X.Y.Z" and "<X.Y.Z" constraints separated by commas
    for constraint in range.split(',') {
        let constraint = constraint.trim();
        if let Some(version) = constraint.strip_prefix(">=") {
            if !version_gte(actual, version.trim()) {
                return false;
            }
        } else if let Some(version) = constraint.strip_prefix('>') {
            if !version_gt(actual, version.trim()) {
                return false;
            }
        } else if let Some(version) = constraint.strip_prefix("<=") {
            if !version_lte(actual, version.trim()) {
                return false;
            }
        } else if let Some(version) = constraint.strip_prefix('<') {
            if !version_lt(actual, version.trim()) {
                return false;
            }
        } else if let Some(version) = constraint.strip_prefix('=') {
            if actual != version.trim() {
                return false;
            }
        }
    }
    true
}

fn parse_version(v: &str) -> (u32, u32, u32) {
    let parts: Vec<u32> = v.split('.').filter_map(|s| s.parse().ok()).collect();
    (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
        parts.get(2).copied().unwrap_or(0),
    )
}

fn version_gte(actual: &str, required: &str) -> bool {
    parse_version(actual) >= parse_version(required)
}

fn version_gt(actual: &str, required: &str) -> bool {
    parse_version(actual) > parse_version(required)
}

fn version_lte(actual: &str, required: &str) -> bool {
    parse_version(actual) <= parse_version(required)
}

fn version_lt(actual: &str, required: &str) -> bool {
    parse_version(actual) < parse_version(required)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_mods_dir() -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mods_path = std::env::temp_dir().join(format!("amigo_mod_test_{id}"));
        let _ = fs::remove_dir_all(&mods_path);
        fs::create_dir_all(&mods_path).unwrap();
        mods_path
    }

    fn write_mod(mods_path: &Path, name: &str, manifest: &str) {
        let mod_dir = mods_path.join(name);
        fs::create_dir_all(&mod_dir).unwrap();
        fs::write(mod_dir.join("mod.toml"), manifest).unwrap();
    }

    #[test]
    fn discover_mods() {
        let mods_path = setup_mods_dir();
        write_mod(
            &mods_path,
            "test_mod",
            r#"
name = "test_mod"
version = "1.0.0"
author = "Test"
engine_version = "*"
"#,
        );
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        let count = mgr.discover().unwrap();
        assert_eq!(count, 1);
        assert_eq!(mgr.discovered()[0].manifest.name, "test_mod");
        assert!(mgr.discovered()[0].errors.is_empty());
    }

    #[test]
    fn discover_invalid_mod() {
        let mods_path = setup_mods_dir();
        let mod_dir = mods_path.join("bad_mod");
        fs::create_dir_all(&mod_dir).unwrap();
        // No mod.toml
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        mgr.discover().unwrap();
        assert!(!mgr.discovered()[0].errors.is_empty());
    }

    #[test]
    fn activate_deactivate() {
        let mods_path = setup_mods_dir();
        write_mod(
            &mods_path,
            "mod_a",
            r#"
name = "mod_a"
version = "1.0.0"
author = "Test"
engine_version = "*"
"#,
        );
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        mgr.discover().unwrap();
        mgr.activate("mod_a").unwrap();
        assert!(mgr.is_active("mod_a"));
        mgr.deactivate("mod_a").unwrap();
        assert!(!mgr.is_active("mod_a"));
    }

    #[test]
    fn dependency_check() {
        let mods_path = setup_mods_dir();
        write_mod(
            &mods_path,
            "base_mod",
            r#"
name = "base_mod"
version = "1.0.0"
author = "Test"
engine_version = "*"
"#,
        );
        write_mod(
            &mods_path,
            "child_mod",
            r#"
name = "child_mod"
version = "1.0.0"
author = "Test"
engine_version = "*"
dependencies = ["base_mod"]
"#,
        );
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        mgr.discover().unwrap();
        // child_mod should fail without base_mod active
        assert!(mgr.activate("child_mod").is_err());
        // Activate base first, then child
        mgr.activate("base_mod").unwrap();
        mgr.activate("child_mod").unwrap();
        assert!(mgr.is_active("child_mod"));
    }

    #[test]
    fn cascade_deactivation() {
        let mods_path = setup_mods_dir();
        write_mod(
            &mods_path,
            "base",
            r#"
name = "base"
version = "1.0.0"
author = "Test"
engine_version = "*"
"#,
        );
        write_mod(
            &mods_path,
            "child",
            r#"
name = "child"
version = "1.0.0"
author = "Test"
engine_version = "*"
dependencies = ["base"]
"#,
        );
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        mgr.discover().unwrap();
        mgr.activate("base").unwrap();
        mgr.activate("child").unwrap();
        let cascaded = mgr.deactivate("base").unwrap();
        assert!(cascaded.contains(&"child".to_string()));
        assert!(!mgr.is_active("child"));
    }

    #[test]
    fn load_order_by_priority() {
        let mods_path = setup_mods_dir();
        write_mod(
            &mods_path,
            "low",
            r#"
name = "low"
version = "1.0.0"
author = "Test"
engine_version = "*"
priority = 1
"#,
        );
        write_mod(
            &mods_path,
            "high",
            r#"
name = "high"
version = "1.0.0"
author = "Test"
engine_version = "*"
priority = 10
"#,
        );
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        mgr.discover().unwrap();
        mgr.activate("low").unwrap();
        mgr.activate("high").unwrap();
        mgr.compute_load_order().unwrap();
        let order = mgr.load_order();
        assert_eq!(order[0], "low");
        assert_eq!(order[1], "high");
    }

    #[test]
    fn version_compat() {
        assert!(version_compatible("*", "0.1.0"));
        assert!(version_compatible(">=0.1.0", "0.1.0"));
        assert!(version_compatible(">=0.1.0", "0.2.0"));
        assert!(!version_compatible(">=1.0.0", "0.9.0"));
        assert!(version_compatible(">=0.1.0, <1.0.0", "0.5.0"));
        assert!(!version_compatible(">=0.1.0, <1.0.0", "1.0.0"));
    }

    #[test]
    fn empty_mods_dir() {
        let mods_path = setup_mods_dir();
        let mut mgr = ModManager::new(mods_path, "0.1.0");
        let count = mgr.discover().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn nonexistent_mods_dir() {
        let mut mgr = ModManager::new(PathBuf::from("/nonexistent/mods"), "0.1.0");
        let count = mgr.discover().unwrap();
        assert_eq!(count, 0); // No error, just no mods
    }
}
