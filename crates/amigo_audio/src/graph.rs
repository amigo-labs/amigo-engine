//! Node-based audio graph built on Kira 0.9 tracks (ADR-0009).
//!
//! Gated behind `cfg(feature = "audio_graph")`. Provides persistent sub-mix
//! buses, per-bus effects, and volume routing. The default topology is:
//!
//! ```text
//! [sfx_bus] ──────────────────────┐
//! [music_bus] ── [music_filter] ──┤
//! [ambient_bus] ──────────────────┼── [master_bus] ── Kira main output
//! [stinger_bus] ──────────────────┘
//! ```

use kira::manager::backend::DefaultBackend;
use kira::manager::AudioManager as KiraManager;
use kira::track::TrackBuilder;
use kira::track::TrackHandle;
use kira::track::TrackRoutes;
use kira::tween::Tween;
use kira::Volume;
use rustc_hash::FxHashMap;
use tracing::warn;

/// Identifies a bus in the audio graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BusId(pub usize);

/// Filter type for a [`FilterNode`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterKind {
    LowPass,
    HighPass,
    BandPass,
}

/// Parameters for a bus filter.
#[derive(Clone, Copy, Debug)]
pub struct FilterParams {
    pub kind: FilterKind,
    pub cutoff_hz: f32,
}

/// A single bus (sub-mix) in the audio graph.
pub struct AudioBus {
    pub name: String,
    pub handle: TrackHandle,
    pub volume: f32,
    pub children: Vec<BusId>,
}

/// A node in the audio graph.
pub enum AudioNode {
    /// Standard mixer bus.
    Mixer(AudioBus),
    /// Bus with a filter effect applied.
    Filter { bus: AudioBus, params: FilterParams },
    /// Ducking (side-chain compression) node.
    Ducking {
        bus: AudioBus,
        sidechain_source: BusId,
        threshold: f32,
        ratio: f32,
        current_gain: f32,
    },
    /// Crossfade between two input buses.
    Crossfade {
        bus: AudioBus,
        input_a: BusId,
        input_b: BusId,
        blend: f32,
    },
}

impl AudioNode {
    fn bus(&self) -> &AudioBus {
        match self {
            AudioNode::Mixer(b) => b,
            AudioNode::Filter { bus, .. } => bus,
            AudioNode::Ducking { bus, .. } => bus,
            AudioNode::Crossfade { bus, .. } => bus,
        }
    }

    fn bus_mut(&mut self) -> &mut AudioBus {
        match self {
            AudioNode::Mixer(b) => b,
            AudioNode::Filter { bus, .. } => bus,
            AudioNode::Ducking { bus, .. } => bus,
            AudioNode::Crossfade { bus, .. } => bus,
        }
    }
}

/// The audio graph manages persistent sub-mix buses backed by Kira tracks.
pub struct AudioGraph {
    nodes: Vec<AudioNode>,
    bus_names: FxHashMap<String, BusId>,
}

impl AudioGraph {
    /// Create an audio graph with the default bus topology.
    ///
    /// Creates: master_bus, sfx_bus, music_bus, ambient_bus, stinger_bus.
    /// All child buses route into master_bus which routes to main output.
    pub fn new(kira: &mut KiraManager<DefaultBackend>) -> Result<Self, String> {
        let mut graph = Self {
            nodes: Vec::new(),
            bus_names: FxHashMap::default(),
        };

        // Master bus → Kira main output.
        let master_id = graph.add_mixer(kira, "master", None)?;

        // Child buses → master.
        graph.add_mixer(kira, "sfx", Some(master_id))?;
        graph.add_mixer(kira, "music", Some(master_id))?;
        graph.add_mixer(kira, "ambient", Some(master_id))?;
        graph.add_mixer(kira, "stinger", Some(master_id))?;

        Ok(graph)
    }

    /// Add a mixer bus, optionally routed to a parent bus.
    fn add_mixer(
        &mut self,
        kira: &mut KiraManager<DefaultBackend>,
        name: &str,
        parent: Option<BusId>,
    ) -> Result<BusId, String> {
        let routes = if let Some(pid) = parent {
            let parent_track = &self.nodes[pid.0].bus().handle;
            TrackRoutes::parent(parent_track)
        } else {
            TrackRoutes::new()
        };

        let builder = TrackBuilder::new().routes(routes);
        let handle = kira.add_sub_track(builder).map_err(|e| e.to_string())?;

        let id = BusId(self.nodes.len());
        let bus = AudioBus {
            name: name.to_string(),
            handle,
            volume: 1.0,
            children: Vec::new(),
        };
        self.nodes.push(AudioNode::Mixer(bus));
        self.bus_names.insert(name.to_string(), id);

        if let Some(pid) = parent {
            self.nodes[pid.0].bus_mut().children.push(id);
        }

        Ok(id)
    }

    /// Look up a bus by name.
    pub fn bus_id(&self, name: &str) -> Option<BusId> {
        self.bus_names.get(name).copied()
    }

    /// Get the Kira `TrackHandle` for a bus (for `output_destination`).
    pub fn track_handle(&self, bus: BusId) -> Option<&TrackHandle> {
        self.nodes.get(bus.0).map(|n| &n.bus().handle)
    }

    /// Set the volume of a named channel. Pushes immediately to Kira.
    pub fn set_channel_volume(&mut self, name: &str, volume: f32) {
        let Some(&id) = self.bus_names.get(name) else {
            warn!("AudioGraph: unknown bus '{name}'");
            return;
        };
        let node = &mut self.nodes[id.0];
        node.bus_mut().volume = volume;
        node.bus_mut()
            .handle
            .set_volume(Volume::Amplitude(volume as f64), Tween::default());
    }

    /// Get the current volume of a named channel.
    pub fn channel_volume(&self, name: &str) -> Option<f32> {
        let id = self.bus_names.get(name)?;
        Some(self.nodes[id.0].bus().volume)
    }

    /// Get the number of buses in the graph.
    pub fn bus_count(&self) -> usize {
        self.nodes.len()
    }

    /// Iterate all bus names.
    pub fn bus_names(&self) -> impl Iterator<Item = &str> {
        self.bus_names.keys().map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: Most AudioGraph tests require a Kira backend which needs an audio
    // device. We test the non-Kira parts here and use structural assertions.

    #[test]
    fn bus_id_equality_and_hashing() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BusId(0));
        set.insert(BusId(1));
        set.insert(BusId(0)); // duplicate
        assert_eq!(set.len(), 2); // BusId hashing deduplicates correctly
    }

    #[test]
    fn filter_kind_all_variants_distinct() {
        let kinds = [
            FilterKind::LowPass,
            FilterKind::HighPass,
            FilterKind::BandPass,
        ];
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(kinds[i], kinds[j]);
            }
        }
    }

    #[test]
    fn filter_params_cutoff_range() {
        let low = FilterParams {
            kind: FilterKind::LowPass,
            cutoff_hz: 20.0,
        };
        let high = FilterParams {
            kind: FilterKind::LowPass,
            cutoff_hz: 20000.0,
        };
        assert!(low.cutoff_hz < high.cutoff_hz);
        // Verify no NaN or negative frequencies
        assert!(low.cutoff_hz > 0.0);
        assert!(!high.cutoff_hz.is_nan());
    }

    #[test]
    fn bus_id_as_map_key() {
        // Verify BusId works correctly as a FxHashMap key (used by AudioGraph internals)
        let mut map = FxHashMap::default();
        map.insert(BusId(0), "master");
        map.insert(BusId(1), "sfx");
        map.insert(BusId(0), "master_updated"); // overwrite
        assert_eq!(map.len(), 2);
        assert_eq!(map[&BusId(0)], "master_updated");
        assert_eq!(map[&BusId(1)], "sfx");
    }
}
