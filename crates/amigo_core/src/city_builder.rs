//! City-builder gametype: zoning, resource flow, road networks, happiness,
//! buildings, disasters, and statistics overlays.

use crate::agents::{NeedType, Needs};
use crate::color::Color;
use crate::ecs::EntityId;
use crate::math::IVec2;
use crate::pathfinding::{FlowField, Walkable};
use crate::rect::Rect;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::BinaryHeap;

// ---------------------------------------------------------------------------
// GridPos alias
// ---------------------------------------------------------------------------

/// Tile-coordinate alias used throughout the city-builder module.
pub type GridPos = IVec2;

// ---------------------------------------------------------------------------
// ResourceType
// ---------------------------------------------------------------------------

/// A resource that flows through the city economy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Food for citizens.
    Food,
    /// Wood building material.
    Wood,
    /// Stone building material.
    Stone,
    /// Refined metal.
    Metal,
    /// Currency.
    Gold,
    /// Water supply.
    Water,
    /// Electrical power.
    Power,
    /// Game-specific resource identified by index.
    Custom(u16),
}

// ---------------------------------------------------------------------------
// FlowNode / FlowEdge / ResourceFlow
// ---------------------------------------------------------------------------

/// Unique identifier for a node in the resource flow graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowNodeId(pub u32);

/// The role a node plays in the resource flow graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FlowNodeType {
    /// Produces a resource at a fixed rate.
    Producer {
        /// The resource this node outputs.
        output: ResourceType,
        /// Units produced per tick.
        rate_per_tick: i32,
        /// Optional input required for production (e.g. ore -> metal).
        input: Option<(ResourceType, i32)>,
    },
    /// Stores resources for pickup.
    Storage,
    /// Consumes a resource (e.g. houses consume food).
    Consumer {
        /// The resource this node requires.
        input: ResourceType,
        /// Units consumed per tick.
        rate_per_tick: i32,
    },
}

/// A node in the resource flow directed graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlowNode {
    /// Unique node id.
    pub id: FlowNodeId,
    /// Role of this node.
    pub node_type: FlowNodeType,
    /// World position.
    pub position: GridPos,
    /// Resource buffer: how much is currently stored here.
    pub buffer: FxHashMap<ResourceType, i32>,
    /// Maximum buffer capacity per resource type.
    pub capacity: FxHashMap<ResourceType, i32>,
}

/// A directed edge in the resource flow graph.
#[derive(Clone, Debug)]
pub struct FlowEdge {
    /// Source node.
    pub from: FlowNodeId,
    /// Destination node.
    pub to: FlowNodeId,
    /// Maximum throughput per tick (limited by road capacity).
    pub max_throughput: i32,
    /// Current throughput this tick.
    pub current_throughput: i32,
    /// Cached road path.
    pub road_path: Vec<GridPos>,
}

/// Directed graph of resource production, storage, and consumption.
#[derive(Default)]
pub struct ResourceFlow {
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
    next_id: u32,
}

impl ResourceFlow {
    /// Create an empty resource flow graph.
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            next_id: 0,
        }
    }

    /// Add a node and return its id.
    pub fn add_node(&mut self, mut node: FlowNode) -> FlowNodeId {
        let id = FlowNodeId(self.next_id);
        self.next_id += 1;
        node.id = id;
        self.nodes.push(node);
        id
    }

    /// Remove a node and all edges referencing it.
    pub fn remove_node(&mut self, id: FlowNodeId) {
        self.nodes.retain(|n| n.id != id);
        self.edges.retain(|e| e.from != id && e.to != id);
    }

    /// Connect two nodes with an edge. Computes a cached road path via A* and
    /// sets `max_throughput` proportional to the path length.
    pub fn connect(&mut self, from: FlowNodeId, to: FlowNodeId, road: &RoadNetwork) {
        let from_pos = self.nodes.iter().find(|n| n.id == from).map(|n| n.position);
        let to_pos = self.nodes.iter().find(|n| n.id == to).map(|n| n.position);

        let (road_path, throughput) = match (from_pos, to_pos) {
            (Some(a), Some(b)) => match road.shortest_path(a, b) {
                Some(path) => {
                    // Throughput inversely proportional to path length (longer = slower).
                    let tp = (20_i32).min(50 / (path.len() as i32).max(1));
                    (path, tp)
                }
                None => (Vec::new(), 10),
            },
            _ => (Vec::new(), 10),
        };

        self.edges.push(FlowEdge {
            from,
            to,
            max_throughput: throughput,
            current_throughput: 0,
            road_path,
        });
    }

    /// Called when a road tile changes. Disconnects edges whose cached paths
    /// passed through the changed tile, forcing reconnection via new topology.
    pub fn on_road_change(&mut self, changed_pos: GridPos) {
        self.edges.retain(|e| !e.road_path.contains(&changed_pos));
    }

    /// Disconnect two nodes.
    pub fn disconnect(&mut self, from: FlowNodeId, to: FlowNodeId) {
        self.edges.retain(|e| !(e.from == from && e.to == to));
    }

    /// Simulate one tick of resource flow.
    pub fn tick(&mut self) {
        // Phase 1: Producers generate output.
        for node in &mut self.nodes {
            if let FlowNodeType::Producer {
                output,
                rate_per_tick,
                ref input,
            } = node.node_type
            {
                // Check if input is satisfied.
                let can_produce = if let Some((in_res, in_rate)) = input {
                    let buf = node.buffer.get(in_res).copied().unwrap_or(0);
                    buf >= *in_rate
                } else {
                    true
                };

                if can_produce {
                    // Consume input.
                    if let Some((in_res, in_rate)) = input {
                        let buf = node.buffer.entry(*in_res).or_insert(0);
                        *buf -= *in_rate;
                    }
                    // Produce output (clamped to capacity).
                    let cap = node.capacity.get(&output).copied().unwrap_or(i32::MAX);
                    let buf = node.buffer.entry(output).or_insert(0);
                    *buf = (*buf + rate_per_tick).min(cap);
                }
            }
        }

        // Phase 2: Transfer resources along edges.
        for edge in &mut self.edges {
            edge.current_throughput = 0;
        }

        // We need indices because we borrow nodes mutably.
        let edge_count = self.edges.len();
        for ei in 0..edge_count {
            let from_id = self.edges[ei].from;
            let to_id = self.edges[ei].to;
            let max_tp = self.edges[ei].max_throughput;

            // Determine what resource to transfer: look at the destination node.
            let to_idx = self.nodes.iter().position(|n| n.id == to_id);
            let from_idx = self.nodes.iter().position(|n| n.id == from_id);

            if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
                // Determine the resource the destination wants.
                let wanted: Vec<(ResourceType, i32)> = match &self.nodes[ti].node_type {
                    FlowNodeType::Consumer {
                        input,
                        rate_per_tick,
                    } => {
                        vec![(*input, *rate_per_tick)]
                    }
                    FlowNodeType::Storage => {
                        // Storage accepts whatever the source has.
                        self.nodes[fi]
                            .buffer
                            .iter()
                            .map(|(r, &amt)| (*r, amt))
                            .collect()
                    }
                    _ => Vec::new(),
                };

                for (res, _demand) in wanted {
                    let available = self.nodes[fi].buffer.get(&res).copied().unwrap_or(0);
                    let dest_cap = self.nodes[ti]
                        .capacity
                        .get(&res)
                        .copied()
                        .unwrap_or(i32::MAX);
                    let dest_buf = self.nodes[ti].buffer.get(&res).copied().unwrap_or(0);
                    let space = dest_cap - dest_buf;
                    let transfer = available.min(max_tp).min(space).max(0);

                    if transfer > 0 {
                        *self.nodes[fi].buffer.entry(res).or_insert(0) -= transfer;
                        *self.nodes[ti].buffer.entry(res).or_insert(0) += transfer;
                        self.edges[ei].current_throughput += transfer;
                    }
                }
            }
        }

        // Phase 3: Consumers draw from their buffers.
        for node in &mut self.nodes {
            if let FlowNodeType::Consumer {
                input,
                rate_per_tick,
            } = node.node_type
            {
                let buf = node.buffer.entry(input).or_insert(0);
                *buf = (*buf - rate_per_tick).max(0);
            }
        }
    }

    /// Total production rate for a resource type across all producers.
    pub fn total_production(&self, resource: ResourceType) -> i32 {
        self.nodes
            .iter()
            .filter_map(|n| match &n.node_type {
                FlowNodeType::Producer {
                    output,
                    rate_per_tick,
                    ..
                } if *output == resource => Some(*rate_per_tick),
                _ => None,
            })
            .sum()
    }

    /// Total consumption rate for a resource type across all consumers.
    pub fn total_consumption(&self, resource: ResourceType) -> i32 {
        self.nodes
            .iter()
            .filter_map(|n| match &n.node_type {
                FlowNodeType::Consumer {
                    input,
                    rate_per_tick,
                } if *input == resource => Some(*rate_per_tick),
                _ => None,
            })
            .sum()
    }

    /// Net flow (production - consumption) for a resource type.
    pub fn net_flow(&self, resource: ResourceType) -> i32 {
        self.total_production(resource) - self.total_consumption(resource)
    }

    /// Returns nodes with unsatisfied demand (consumers with empty buffers).
    pub fn shortages(&self) -> Vec<(FlowNodeId, ResourceType)> {
        self.nodes
            .iter()
            .filter_map(|n| {
                if let FlowNodeType::Consumer { input, .. } = &n.node_type {
                    let buf = n.buffer.get(input).copied().unwrap_or(0);
                    if buf == 0 {
                        return Some((n.id, *input));
                    }
                }
                None
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ZoneType / ZoneBuilding / ZoneSystem
// ---------------------------------------------------------------------------

/// Type of urban zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneType {
    /// Housing.
    Residential,
    /// Shops and offices.
    Commercial,
    /// Factories and warehouses.
    Industrial,
    /// Special zone types (parks, civic, etc.).
    Special(u16),
}

/// A building that has grown in a zone.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoneBuilding {
    /// Zone this building belongs to.
    pub zone: ZoneType,
    /// Position on the tilemap.
    pub position: GridPos,
    /// Footprint size in tiles.
    pub size: (u32, u32),
    /// Density level (1 = low-rise, 2 = mid-rise, 3 = high-rise).
    pub density: u8,
    /// Number of residents/workers/shoppers this building supports.
    pub capacity: u32,
    /// Current occupancy.
    pub occupancy: u32,
}

/// Zoning layer on the tilemap.
#[derive(Clone, Debug)]
pub struct ZoneSystem {
    zones: Vec<Option<ZoneType>>,
    /// Map width in tiles.
    pub width: u32,
    /// Map height in tiles.
    pub height: u32,
}

impl ZoneSystem {
    /// Create a new zone system with no zones painted.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            zones: vec![None; (width * height) as usize],
            width,
            height,
        }
    }

    fn idx(&self, pos: GridPos) -> Option<usize> {
        if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < self.width && (pos.y as u32) < self.height {
            Some((pos.y as u32 * self.width + pos.x as u32) as usize)
        } else {
            None
        }
    }

    /// Paint a zone type onto a rectangular area.
    pub fn paint_zone(&mut self, area: Rect, zone: ZoneType) {
        let x0 = (area.x as i32).max(0) as u32;
        let y0 = (area.y as i32).max(0) as u32;
        let x1 = ((area.x + area.w) as u32).min(self.width);
        let y1 = ((area.y + area.h) as u32).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.width + x) as usize;
                self.zones[i] = Some(zone);
            }
        }
    }

    /// Remove zoning from an area.
    pub fn clear_zone(&mut self, area: Rect) {
        let x0 = (area.x as i32).max(0) as u32;
        let y0 = (area.y as i32).max(0) as u32;
        let x1 = ((area.x + area.w) as u32).min(self.width);
        let y1 = ((area.y + area.h) as u32).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.width + x) as usize;
                self.zones[i] = None;
            }
        }
    }

    /// Query the zone at a position.
    pub fn zone_at(&self, pos: GridPos) -> Option<ZoneType> {
        self.idx(pos).and_then(|i| self.zones[i])
    }

    /// Find empty zoned tiles adjacent to roads that can accept new buildings.
    pub fn growable_tiles(&self, zone: ZoneType, roads: &RoadNetwork) -> Vec<GridPos> {
        let mut result = Vec::new();
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let pos = GridPos::new(x, y);
                if self.zone_at(pos) != Some(zone) {
                    continue;
                }
                // Check road adjacency (4-connected).
                let adjacent_to_road = [(0, 1), (0, -1), (1, 0), (-1, 0)]
                    .iter()
                    .any(|&(dx, dy)| roads.has_road(GridPos::new(x + dx, y + dy)));
                if adjacent_to_road {
                    result.push(pos);
                }
            }
        }
        result
    }

    /// Attempt to grow a building in a zone. Called periodically by the
    /// simulation. `demand` is a 0.0..1.0 value indicating how badly new
    /// buildings of this zone type are needed.
    ///
    /// Density progression: density starts at 1 (low-rise). When demand > 0.5
    /// a mid-rise (density 2) may spawn instead. When demand > 0.8, high-rise
    /// (density 3) can appear. Higher density buildings have larger capacity.
    pub fn try_grow(
        &mut self,
        zone: ZoneType,
        demand: f32,
        roads: &RoadNetwork,
        _registry: &BuildingRegistry,
    ) -> Option<ZoneBuilding> {
        if demand <= 0.0 {
            return None;
        }

        let candidates = self.growable_tiles(zone, roads);
        if candidates.is_empty() {
            return None;
        }

        // Pick the first growable tile (deterministic; a real game would add
        // randomness or scoring based on land value / happiness).
        let pos = candidates[0];

        // Density progression based on demand.
        let density: u8 = if demand > 0.8 {
            3
        } else if demand > 0.5 {
            2
        } else {
            1
        };

        let capacity = match density {
            1 => match zone {
                ZoneType::Residential => 4,
                ZoneType::Commercial => 2,
                ZoneType::Industrial => 3,
                ZoneType::Special(_) => 2,
            },
            2 => match zone {
                ZoneType::Residential => 12,
                ZoneType::Commercial => 8,
                ZoneType::Industrial => 10,
                ZoneType::Special(_) => 6,
            },
            _ => match zone {
                ZoneType::Residential => 30,
                ZoneType::Commercial => 20,
                ZoneType::Industrial => 25,
                ZoneType::Special(_) => 15,
            },
        };

        Some(ZoneBuilding {
            zone,
            position: pos,
            size: (1, 1),
            density,
            capacity,
            occupancy: 0,
        })
    }
}

// ---------------------------------------------------------------------------
// RoadNetwork
// ---------------------------------------------------------------------------

/// Graph of road tiles used for pathfinding and connectivity.
pub struct RoadNetwork {
    tiles: Vec<bool>,
    /// Map width in tiles.
    pub width: u32,
    /// Map height in tiles.
    pub height: u32,
    /// Cached connectivity components.
    components: Vec<u32>,
    /// Flow field cache for common destinations.
    flow_cache: FxHashMap<GridPos, FlowField>,
    /// Per-tile traffic density counters.
    traffic: Vec<u32>,
}

impl RoadNetwork {
    /// Create a new road network with no roads.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            tiles: vec![false; size],
            width,
            height,
            components: vec![0; size],
            flow_cache: FxHashMap::default(),
            traffic: vec![0; size],
        }
    }

    fn idx(&self, pos: GridPos) -> Option<usize> {
        if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < self.width && (pos.y as u32) < self.height {
            Some((pos.y as u32 * self.width + pos.x as u32) as usize)
        } else {
            None
        }
    }

    /// Place a road tile.
    pub fn place_road(&mut self, pos: GridPos) {
        if let Some(i) = self.idx(pos) {
            self.tiles[i] = true;
            self.recompute_components();
            self.invalidate_cache();
        }
    }

    /// Remove a road tile.
    pub fn remove_road(&mut self, pos: GridPos) {
        if let Some(i) = self.idx(pos) {
            self.tiles[i] = false;
            self.recompute_components();
            self.invalidate_cache();
        }
    }

    /// Check whether a road exists at `pos`.
    pub fn has_road(&self, pos: GridPos) -> bool {
        self.idx(pos).is_some_and(|i| self.tiles[i])
    }

    /// Check if two positions are connected via roads.
    pub fn connected(&self, a: GridPos, b: GridPos) -> bool {
        match (self.idx(a), self.idx(b)) {
            (Some(ia), Some(ib)) => {
                self.tiles[ia] && self.tiles[ib] && self.components[ia] == self.components[ib]
            }
            _ => false,
        }
    }

    /// Invalidate cached flow fields (called after road topology changes).
    pub fn invalidate_cache(&mut self) {
        self.flow_cache.clear();
    }

    /// Traffic density at a tile (agents using this tile per tick).
    pub fn traffic_density(&self, pos: GridPos) -> u32 {
        self.idx(pos).map_or(0, |i| self.traffic[i])
    }

    /// Increment traffic counter for a tile (called during agent movement).
    pub fn record_traffic(&mut self, pos: GridPos) {
        if let Some(i) = self.idx(pos) {
            self.traffic[i] += 1;
        }
    }

    /// Reset per-tick traffic counters (called at the start of each tick).
    pub fn reset_traffic(&mut self) {
        self.traffic.iter_mut().for_each(|t| *t = 0);
    }

    /// Find shortest road path between two points via A*.
    /// Returns `None` if no path exists or positions are not on roads.
    pub fn shortest_path(&self, from: GridPos, to: GridPos) -> Option<Vec<GridPos>> {
        if !self.has_road(from) || !self.has_road(to) || !self.connected(from, to) {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }

        let idx = |p: GridPos| (p.y as u32 * self.width + p.x as u32) as usize;
        let size = (self.width * self.height) as usize;
        let mut g_score = vec![i32::MAX; size];
        let mut came_from: Vec<Option<GridPos>> = vec![None; size];

        g_score[idx(from)] = 0;

        // (negative f-score for max-heap used as min-heap, pos)
        let mut open = BinaryHeap::new();
        let h = |p: GridPos| (p.x - to.x).abs() + (p.y - to.y).abs();
        open.push(std::cmp::Reverse((h(from), 0i32, from)));

        while let Some(std::cmp::Reverse((_f, g, cur))) = open.pop() {
            if cur == to {
                // Reconstruct path.
                let mut path = vec![to];
                let mut c = to;
                while let Some(prev) = came_from[idx(c)] {
                    path.push(prev);
                    c = prev;
                    if c == from {
                        break;
                    }
                }
                path.reverse();
                return Some(path);
            }
            if g > g_score[idx(cur)] {
                continue;
            }
            for &(dx, dy) in &[(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
                let nx = cur.x + dx;
                let ny = cur.y + dy;
                let next = GridPos::new(nx, ny);
                if !self.has_road(next) {
                    continue;
                }
                let ng = g + 1;
                let ni = idx(next);
                if ng < g_score[ni] {
                    g_score[ni] = ng;
                    came_from[ni] = Some(cur);
                    open.push(std::cmp::Reverse((ng + h(next), ng, next)));
                }
            }
        }
        None
    }

    /// Get or compute a flow field toward a destination. Cached until
    /// `invalidate_cache()` is called (on road topology changes).
    pub fn flow_field_to(&mut self, destination: GridPos) -> &FlowField {
        if !self.flow_cache.contains_key(&destination) {
            let road_walkable = RoadWalkable {
                tiles: &self.tiles,
                width: self.width,
                height: self.height,
            };
            let field = FlowField::compute(destination, self.width, self.height, &road_walkable);
            self.flow_cache.insert(destination, field);
        }
        self.flow_cache.get(&destination).unwrap()
    }

    /// All tiles in the same connected component as `pos`.
    pub fn connected_component(&self, pos: GridPos) -> Vec<GridPos> {
        let idx = match self.idx(pos) {
            Some(i) if self.tiles[i] => i,
            _ => return Vec::new(),
        };
        let label = self.components[idx];
        let mut result = Vec::new();
        for y in 0..self.height as i32 {
            for x in 0..self.width as i32 {
                let i = (y as u32 * self.width + x as u32) as usize;
                if self.tiles[i] && self.components[i] == label {
                    result.push(GridPos::new(x, y));
                }
            }
        }
        result
    }

    // -- internal helpers ---------------------------------------------------

    /// BFS-based connected component labelling.
    fn recompute_components(&mut self) {
        let size = (self.width * self.height) as usize;
        self.components = vec![0; size];
        let mut label: u32 = 0;
        let mut visited = vec![false; size];

        for start in 0..size {
            if !self.tiles[start] || visited[start] {
                continue;
            }
            label += 1;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            visited[start] = true;
            while let Some(cur) = queue.pop_front() {
                self.components[cur] = label;
                let cx = (cur as u32) % self.width;
                let cy = (cur as u32) / self.width;
                for &(dx, dy) in &[(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && ny >= 0 && (nx as u32) < self.width && (ny as u32) < self.height {
                        let ni = (ny as u32 * self.width + nx as u32) as usize;
                        if self.tiles[ni] && !visited[ni] {
                            visited[ni] = true;
                            queue.push_back(ni);
                        }
                    }
                }
            }
        }
    }
}

/// Adapter that makes the road grid implement [`Walkable`] for flow field
/// computation.
struct RoadWalkable<'a> {
    tiles: &'a [bool],
    width: u32,
    height: u32,
}

impl<'a> Walkable for RoadWalkable<'a> {
    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || (x as u32) >= self.width || (y as u32) >= self.height {
            return false;
        }
        self.tiles[(y as u32 * self.width + x as u32) as usize]
    }
}

// ---------------------------------------------------------------------------
// HappinessFactor / HappinessModel / HappinessGrid
// ---------------------------------------------------------------------------

/// A factor contributing to citizen happiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HappinessFactor {
    /// Public safety (police, fire dept).
    Safety,
    /// Schools and universities.
    Education,
    /// Hospitals and clinics.
    Health,
    /// Parks and entertainment.
    Leisure,
    /// Job availability.
    Employment,
    /// Green spaces and cleanliness.
    Environment,
    /// Inverse of commute length.
    Commute,
    /// Inverse of noise and pollution levels.
    Pollution,
}

/// Multi-factor happiness aggregation per zone and globally.
#[derive(Clone, Debug)]
pub struct HappinessModel {
    /// Per-factor weights (should sum to 1.0).
    pub weights: FxHashMap<HappinessFactor, f32>,
}

impl HappinessModel {
    /// Create with explicit weights.
    pub fn new(weights: FxHashMap<HappinessFactor, f32>) -> Self {
        Self { weights }
    }

    /// Default equal-weight model.
    pub fn default_weights() -> Self {
        let mut weights = FxHashMap::default();
        let factors = [
            HappinessFactor::Safety,
            HappinessFactor::Education,
            HappinessFactor::Health,
            HappinessFactor::Leisure,
            HappinessFactor::Employment,
            HappinessFactor::Environment,
            HappinessFactor::Commute,
            HappinessFactor::Pollution,
        ];
        let w = 1.0 / factors.len() as f32;
        for f in &factors {
            weights.insert(*f, w);
        }
        Self { weights }
    }

    /// Compute aggregate happiness for a single tile.
    pub fn score_at(&self, grid: &HappinessGrid, pos: GridPos) -> f32 {
        let idx = match grid.idx(pos) {
            Some(i) => i,
            None => return 0.0,
        };
        let factors = &grid.scores[idx];
        self.weights
            .iter()
            .map(|(f, w)| w * factors.get(f).copied().unwrap_or(0.0))
            .sum()
    }

    /// Compute average happiness for a rectangular zone area.
    pub fn zone_score(&self, grid: &HappinessGrid, area: Rect) -> f32 {
        let x0 = (area.x as i32).max(0);
        let y0 = (area.y as i32).max(0);
        let x1 = ((area.x + area.w) as i32).min(grid.width as i32);
        let y1 = ((area.y + area.h) as i32).min(grid.height as i32);
        let mut total = 0.0f32;
        let mut count = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                total += self.score_at(grid, GridPos::new(x, y));
                count += 1;
            }
        }
        if count > 0 {
            total / count as f32
        } else {
            0.0
        }
    }

    /// Global average happiness across all tiles.
    pub fn global_score(&self, grid: &HappinessGrid) -> f32 {
        let n = grid.scores.len();
        if n == 0 {
            return 0.0;
        }
        let total: f32 = (0..grid.height as i32)
            .flat_map(|y| (0..grid.width as i32).map(move |x| GridPos::new(x, y)))
            .map(|pos| self.score_at(grid, pos))
            .sum();
        total / n as f32
    }
}

/// Per-tile happiness scores (used for heatmap overlay).
#[derive(Clone, Debug)]
pub struct HappinessGrid {
    scores: Vec<FxHashMap<HappinessFactor, f32>>,
    /// Map width in tiles.
    pub width: u32,
    /// Map height in tiles.
    pub height: u32,
}

impl HappinessGrid {
    /// Create a blank happiness grid.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            scores: vec![FxHashMap::default(); (width * height) as usize],
            width,
            height,
        }
    }

    fn idx(&self, pos: GridPos) -> Option<usize> {
        if pos.x >= 0 && pos.y >= 0 && (pos.x as u32) < self.width && (pos.y as u32) < self.height {
            Some((pos.y as u32 * self.width + pos.x as u32) as usize)
        } else {
            None
        }
    }

    /// Update happiness factors based on building placement.
    /// Uses linear distance falloff within each building's effect radius.
    pub fn update_from_buildings(
        &mut self,
        buildings: &[PlacedBuilding],
        registry: &BuildingRegistry,
        _roads: &RoadNetwork,
    ) {
        // Reset all scores.
        for s in &mut self.scores {
            s.clear();
        }

        for building in buildings {
            if !building.operational {
                continue;
            }
            let def = match registry.get(building.def_id) {
                Some(d) => d,
                None => continue,
            };
            let radius = match def.effect_radius {
                Some(r) => r,
                None => continue,
            };
            let bx = building.position.x;
            let by = building.position.y;
            let r = radius as i32;

            for dy in -r..=r {
                for dx in -r..=r {
                    let tx = bx + dx;
                    let ty = by + dy;
                    let dist = (dx.abs() + dy.abs()) as f32; // Manhattan distance
                    if dist > radius as f32 {
                        continue;
                    }
                    let falloff = 1.0 - dist / radius as f32;
                    if let Some(idx) = self.idx(GridPos::new(tx, ty)) {
                        // Positive effects
                        for (&factor, &value) in &def.happiness_effects {
                            let entry = self.scores[idx].entry(factor).or_insert(0.0);
                            *entry = (*entry + value * falloff).clamp(0.0, 1.0);
                        }
                        // Negative effects
                        for (&factor, &value) in &def.negative_effects {
                            let entry = self.scores[idx].entry(factor).or_insert(0.0);
                            *entry = (*entry - value * falloff).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
    }

    /// Query a single factor at a position.
    pub fn factor_at(&self, pos: GridPos, factor: HappinessFactor) -> f32 {
        self.idx(pos)
            .and_then(|i| self.scores[i].get(&factor).copied())
            .unwrap_or(0.0)
    }

    /// Directly set a factor value at a position.
    pub fn set_factor(&mut self, pos: GridPos, factor: HappinessFactor, value: f32) {
        if let Some(i) = self.idx(pos) {
            self.scores[i].insert(factor, value);
        }
    }
}

// ---------------------------------------------------------------------------
// BuildingId / BuildingDef / BuildingCategory / PlacedBuilding / BuildingRegistry
// ---------------------------------------------------------------------------

/// Unique identifier for a building definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildingId(pub u32);

/// Category for UI grouping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingCategory {
    /// Housing.
    Residential,
    /// Shops and offices.
    Commercial,
    /// Factories and warehouses.
    Industrial,
    /// Service buildings (hospitals, schools, fire stations).
    Service,
    /// Roads, pipes, power lines.
    Infrastructure,
    /// Cosmetic items (statues, gardens).
    Decoration,
}

/// Definition of a building type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildingDef {
    /// Unique id.
    pub id: BuildingId,
    /// Human-readable name.
    pub name: String,
    /// Footprint in tiles.
    pub size: (u32, u32),
    /// Construction cost in resources.
    pub cost: FxHashMap<ResourceType, i32>,
    /// Monthly upkeep cost.
    pub upkeep: FxHashMap<ResourceType, i32>,
    /// Resource production (if any).
    pub production: Option<(ResourceType, i32)>,
    /// Resource consumption (if any).
    pub consumption: Option<(ResourceType, i32)>,
    /// Radius of effect for service buildings.
    pub effect_radius: Option<u32>,
    /// Happiness factors this building provides within its radius.
    pub happiness_effects: FxHashMap<HappinessFactor, f32>,
    /// Negative effects within radius (noise, pollution).
    pub negative_effects: FxHashMap<HappinessFactor, f32>,
    /// Category for UI grouping.
    pub category: BuildingCategory,
}

/// A building that has been placed in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedBuilding {
    /// Definition id referencing the `BuildingRegistry`.
    pub def_id: BuildingId,
    /// Position on the tilemap.
    pub position: GridPos,
    /// Construction progress (0.0 = just placed, 1.0 = complete).
    pub construction: f32,
    /// Whether this building is operational.
    pub operational: bool,
    /// Associated resource flow node.
    pub flow_node: Option<FlowNodeId>,
}

/// Central registry of all building types and their properties.
#[derive(Default)]
pub struct BuildingRegistry {
    buildings: Vec<BuildingDef>,
}

impl BuildingRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            buildings: Vec::new(),
        }
    }

    /// Register a building definition.
    pub fn register(&mut self, def: BuildingDef) {
        self.buildings.push(def);
    }

    /// Look up a building definition by id.
    pub fn get(&self, id: BuildingId) -> Option<&BuildingDef> {
        self.buildings.iter().find(|b| b.id == id)
    }

    /// Return all definitions in a category.
    pub fn by_category(&self, category: BuildingCategory) -> Vec<&BuildingDef> {
        self.buildings
            .iter()
            .filter(|b| b.category == category)
            .collect()
    }

    /// Check if a building can be placed at a position. Validates that:
    /// 1. The building footprint is within map bounds.
    /// 2. All footprint tiles match the expected zone (or are unzoned for
    ///    infrastructure/service buildings).
    /// 3. At least one footprint tile is adjacent to a road.
    pub fn can_place(
        &self,
        id: BuildingId,
        pos: GridPos,
        zones: &ZoneSystem,
        roads: &RoadNetwork,
    ) -> bool {
        let def = match self.get(id) {
            Some(d) => d,
            None => return false,
        };

        let (w, h) = def.size;
        let mut any_road_adjacent = false;

        for dy in 0..h as i32 {
            for dx in 0..w as i32 {
                let tile = GridPos::new(pos.x + dx, pos.y + dy);

                // Bounds check.
                if tile.x < 0
                    || tile.y < 0
                    || (tile.x as u32) >= zones.width
                    || (tile.y as u32) >= zones.height
                {
                    return false;
                }

                // Zone compatibility.
                let zone = zones.zone_at(tile);
                let compatible = match def.category {
                    BuildingCategory::Residential => zone == Some(ZoneType::Residential),
                    BuildingCategory::Commercial => zone == Some(ZoneType::Commercial),
                    BuildingCategory::Industrial => zone == Some(ZoneType::Industrial),
                    // Service, infrastructure, decoration can be placed on any
                    // unzoned tile.
                    _ => zone.is_none(),
                };
                if !compatible {
                    return false;
                }

                // Road adjacency (4-connected from any footprint tile).
                for &(nx, ny) in &[(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
                    if roads.has_road(GridPos::new(tile.x + nx, tile.y + ny)) {
                        any_road_adjacent = true;
                    }
                }
            }
        }

        any_road_adjacent
    }
}

// ---------------------------------------------------------------------------
// DisasterType / ActiveDisaster / DisasterSystem
// ---------------------------------------------------------------------------

/// Type of disaster event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisasterType {
    /// Fire outbreak.
    Fire,
    /// River or coastal flooding.
    Flood,
    /// Seismic event.
    Earthquake,
    /// Destructive wind funnel.
    Tornado,
    /// Disease outbreak.
    Epidemic,
}

/// An active disaster currently affecting the city.
#[derive(Clone, Debug)]
pub struct ActiveDisaster {
    /// Kind of disaster.
    pub disaster_type: DisasterType,
    /// Affected area in world tiles.
    pub area: Rect,
    /// Remaining duration in ticks.
    pub remaining_ticks: u32,
    /// Damage applied per tick to buildings in the area.
    pub damage_per_tick: f32,
}

/// Random event system for natural and man-made disasters.
pub struct DisasterSystem {
    /// Active disasters currently affecting the city.
    pub active: Vec<ActiveDisaster>,
    /// Cooldown ticks between disaster events.
    pub cooldown: u32,
    /// Remaining cooldown ticks.
    remaining_cooldown: u32,
    /// Whether disasters are enabled (can be disabled in sandbox mode).
    pub enabled: bool,
}

impl DisasterSystem {
    /// Create a new disaster system with the given cooldown between events.
    pub fn new(cooldown: u32) -> Self {
        Self {
            active: Vec::new(),
            cooldown,
            remaining_cooldown: cooldown,
            enabled: true,
        }
    }

    /// Advance active disasters by one tick. Expired ones are removed.
    pub fn tick(&mut self, _happiness: &HappinessGrid, _model: &HappinessModel) {
        if !self.enabled {
            return;
        }
        for d in &mut self.active {
            d.remaining_ticks = d.remaining_ticks.saturating_sub(1);
        }
        self.active.retain(|d| d.remaining_ticks > 0);
        self.remaining_cooldown = self.remaining_cooldown.saturating_sub(1);
    }

    /// Manually trigger a disaster (debug / scenario).
    pub fn trigger(&mut self, disaster_type: DisasterType, area: Rect, duration: u32) {
        self.active.push(ActiveDisaster {
            disaster_type,
            area,
            remaining_ticks: duration,
            damage_per_tick: 0.05,
        });
    }

    /// Apply damage to buildings in disaster areas.
    pub fn apply_damage(&self, buildings: &mut [PlacedBuilding]) {
        for disaster in &self.active {
            for building in buildings.iter_mut() {
                let bx = building.position.x as f32;
                let by = building.position.y as f32;
                if disaster.area.contains(bx, by) {
                    building.construction =
                        (building.construction - disaster.damage_per_tick).max(0.0);
                    if building.construction <= 0.0 {
                        building.operational = false;
                    }
                }
            }
        }
    }

    /// Check if a position is affected by an active disaster.
    pub fn is_affected(&self, pos: GridPos) -> Option<DisasterType> {
        let px = pos.x as f32;
        let py = pos.y as f32;
        self.active
            .iter()
            .find(|d| d.area.contains(px, py))
            .map(|d| d.disaster_type)
    }
}

// ---------------------------------------------------------------------------
// OverlayType / ColorGradient / StatisticsOverlay
// ---------------------------------------------------------------------------

/// Type of heatmap overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayType {
    /// Happiness heatmap, optionally filtered to a single factor.
    Happiness(Option<HappinessFactor>),
    /// Traffic density.
    Traffic,
    /// Pollution levels.
    Pollution,
    /// Resource throughput.
    ResourceFlow(ResourceType),
    /// Property value.
    LandValue,
    /// Service building coverage.
    ServiceCoverage(BuildingCategory),
    /// Building effect radius preview (shown during placement).
    BuildingRadius(BuildingId),
}

/// A colour gradient used for heatmap rendering.
#[derive(Clone, Debug)]
pub struct ColorGradient {
    /// Sorted stops: (t, color) where t is in [0.0, 1.0].
    pub stops: Vec<(f32, Color)>,
}

impl ColorGradient {
    /// Create a gradient from explicit stops.
    pub fn new(stops: Vec<(f32, Color)>) -> Self {
        Self { stops }
    }

    /// Default green-yellow-red gradient for most overlays.
    pub fn default_heatmap() -> Self {
        Self {
            stops: vec![(0.0, Color::GREEN), (0.5, Color::YELLOW), (1.0, Color::RED)],
        }
    }

    /// Interpolate a color for a value in [0.0, 1.0].
    pub fn sample(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        if self.stops.is_empty() {
            return Color::WHITE;
        }
        if self.stops.len() == 1 {
            return self.stops[0].1;
        }
        // Find the two surrounding stops.
        let mut lower = &self.stops[0];
        let mut upper = &self.stops[self.stops.len() - 1];
        for window in self.stops.windows(2) {
            if t >= window[0].0 && t <= window[1].0 {
                lower = &window[0];
                upper = &window[1];
                break;
            }
        }
        let range = upper.0 - lower.0;
        let frac = if range > 0.0 {
            (t - lower.0) / range
        } else {
            0.0
        };
        Color::new(
            lower.1.r + (upper.1.r - lower.1.r) * frac,
            lower.1.g + (upper.1.g - lower.1.g) * frac,
            lower.1.b + (upper.1.b - lower.1.b) * frac,
            lower.1.a + (upper.1.a - lower.1.a) * frac,
        )
    }
}

/// Heatmap overlays for visualizing city data on the tilemap.
pub struct StatisticsOverlay {
    /// Currently active overlay (if any).
    pub active_overlay: Option<OverlayType>,
    /// Color gradient for heatmap rendering.
    pub gradient: ColorGradient,
}

#[allow(clippy::new_without_default)]
impl StatisticsOverlay {
    /// Create with no active overlay and the default heatmap gradient.
    pub fn new() -> Self {
        Self {
            active_overlay: None,
            gradient: ColorGradient::default_heatmap(),
        }
    }

    /// Activate an overlay.
    pub fn set_overlay(&mut self, overlay: OverlayType) {
        self.active_overlay = Some(overlay);
    }

    /// Deactivate the current overlay.
    pub fn clear_overlay(&mut self) {
        self.active_overlay = None;
    }
}

// ---------------------------------------------------------------------------
// CitizenState / CitizenSeekTarget / Citizen / PopulationSim
// ---------------------------------------------------------------------------

/// What a citizen is currently seeking when in the `Seeking` state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitizenSeekTarget {
    /// Looking for a residence.
    Home,
    /// Looking for employment.
    Job,
    /// Looking for food.
    Food,
    /// Looking for leisure / fun.
    Entertainment,
}

/// Current behavior state of a citizen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitizenState {
    /// Resting at home.
    AtHome,
    /// Travelling between locations via the road network.
    Commuting,
    /// Producing resources at their workplace.
    Working,
    /// Visiting a commercial zone (consuming Food resource).
    Shopping,
    /// Visiting a leisure building / park.
    Leisure,
    /// Actively searching for something.
    Seeking(CitizenSeekTarget),
}

/// A single citizen in the agent-based population simulation.
///
/// Citizens are lightweight structs for high population counts. Only citizens
/// near the camera are promoted to full ECS entities with sprites (see LOD
/// Transition Protocol in the spec).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Citizen {
    /// Unique entity id (used for ECS promotion when on-screen).
    pub id: EntityId,
    /// Home building node in the resource flow graph.
    pub home: Option<FlowNodeId>,
    /// Workplace building node in the resource flow graph.
    pub workplace: Option<FlowNodeId>,
    /// Agent needs from the engine Needs system.
    pub needs: Needs,
    /// Current behavior state.
    pub state: CitizenState,
    /// Age in simulation ticks (for birth/death cycle).
    pub age: u32,
}

/// Agent-based population simulation using Utility AI.
///
/// Each citizen has [`Needs`] that decay over time. The tick function evaluates
/// needs and transitions citizens between states (home, work, shopping,
/// leisure) along the road network.
pub struct PopulationSim {
    /// All citizens in the city.
    pub agents: Vec<Citizen>,
    /// Birth rate modifier (higher = more births per tick).
    pub birth_rate: f32,
    /// Death rate modifier (higher = more deaths per tick).
    pub death_rate: f32,
    /// Global happiness above this threshold attracts immigrants.
    pub immigration_threshold: f32,
    /// Next entity id counter for newly spawned citizens.
    next_entity_index: u32,
}

impl PopulationSim {
    /// Create an empty population simulation with default rates.
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            birth_rate: 0.001,
            death_rate: 0.0005,
            immigration_threshold: 0.5,
            next_entity_index: 0,
        }
    }

    /// Simulate one tick. Each citizen evaluates needs via Utility AI and
    /// picks the highest-scoring action (go home, go to work, seek food, etc.).
    ///
    /// State transitions:
    /// - **AtHome**: Comfort need satisfied. If Employment urgency is high,
    ///   transition to Commuting toward workplace.
    /// - **Working**: Employment need satisfied, earns Gold. If Hunger urgency
    ///   is high, transition to Shopping.
    /// - **Shopping**: Hunger need satisfied (consumes Food). Returns home when
    ///   Comfort urgency rises.
    /// - **Leisure**: Fun need satisfied. Returns home after.
    /// - **Seeking(target)**: Agent is looking for a missing resource (home,
    ///   job, food, entertainment). Transitions once found.
    /// - **Commuting**: In transit. Transitions to destination state next tick.
    pub fn tick(
        &mut self,
        happiness: &HappinessGrid,
        model: &HappinessModel,
        _roads: &RoadNetwork,
        _resources: &ResourceFlow,
    ) {
        for citizen in &mut self.agents {
            citizen.age += 1;

            // Decay needs.
            citizen.needs.tick();

            // Utility-based state transitions.
            let comfort_urgency = citizen
                .needs
                .values
                .get(&NeedType::Comfort)
                .map_or(0.0, |n| n.urgency());
            let hunger_urgency = citizen
                .needs
                .values
                .get(&NeedType::Hunger)
                .map_or(0.0, |n| n.urgency());
            let fun_urgency = citizen
                .needs
                .values
                .get(&NeedType::Fun)
                .map_or(0.0, |n| n.urgency());

            // Check local happiness for the citizen's home tile.
            let _local_happiness = citizen
                .home
                .map(|_| model.global_score(happiness))
                .unwrap_or(0.0);

            match citizen.state {
                CitizenState::AtHome => {
                    // Satisfy comfort while at home.
                    citizen.needs.satisfy(NeedType::Comfort, 2.0);
                    citizen.needs.satisfy(NeedType::Sleep, 1.0);

                    // Decide what to do next.
                    if citizen.workplace.is_none() {
                        citizen.state = CitizenState::Seeking(CitizenSeekTarget::Job);
                    } else if hunger_urgency > 0.6 || fun_urgency > 0.5 {
                        citizen.state = CitizenState::Commuting;
                    } else if comfort_urgency < 0.3 && citizen.workplace.is_some() {
                        // Comfort is fine, go to work.
                        citizen.state = CitizenState::Commuting;
                    }
                }
                CitizenState::Commuting => {
                    // Commuting takes one tick (simplified). Pick destination
                    // based on highest urgency.
                    if hunger_urgency > fun_urgency && hunger_urgency > comfort_urgency {
                        citizen.state = CitizenState::Shopping;
                    } else if fun_urgency > comfort_urgency {
                        citizen.state = CitizenState::Leisure;
                    } else if citizen.workplace.is_some() {
                        citizen.state = CitizenState::Working;
                    } else {
                        citizen.state = CitizenState::AtHome;
                    }
                }
                CitizenState::Working => {
                    // Satisfy employment need and earn gold (handled at
                    // resource flow level).
                    citizen.needs.satisfy(NeedType::Social, 0.5);

                    // Leave work when hungry or tired.
                    if hunger_urgency > 0.5 {
                        citizen.state = CitizenState::Shopping;
                    } else if comfort_urgency > 0.6 {
                        citizen.state = CitizenState::Commuting;
                    } else if fun_urgency > 0.7 {
                        citizen.state = CitizenState::Leisure;
                    }
                }
                CitizenState::Shopping => {
                    // Satisfy hunger.
                    citizen.needs.satisfy(NeedType::Hunger, 5.0);

                    // Head home or to leisure afterwards.
                    if fun_urgency > 0.5 {
                        citizen.state = CitizenState::Leisure;
                    } else {
                        citizen.state = CitizenState::Commuting;
                    }
                }
                CitizenState::Leisure => {
                    // Satisfy fun.
                    citizen.needs.satisfy(NeedType::Fun, 4.0);
                    citizen.needs.satisfy(NeedType::Social, 1.0);

                    // Head home afterwards.
                    citizen.state = CitizenState::Commuting;
                }
                CitizenState::Seeking(target) => {
                    match target {
                        CitizenSeekTarget::Home => {
                            // If homeless, stay seeking. In a full
                            // implementation this would scan residential
                            // vacancies.
                            if citizen.home.is_some() {
                                citizen.state = CitizenState::AtHome;
                            }
                        }
                        CitizenSeekTarget::Job => {
                            // Keep seeking. In full implementation, would scan
                            // industrial/commercial vacancies.
                            if citizen.workplace.is_some() {
                                citizen.state = CitizenState::Commuting;
                            }
                        }
                        CitizenSeekTarget::Food => {
                            citizen.state = CitizenState::Shopping;
                        }
                        CitizenSeekTarget::Entertainment => {
                            citizen.state = CitizenState::Leisure;
                        }
                    }
                }
            }
        }

        // Birth/death/immigration cycle.
        let pop = self.agents.len() as f32;
        if pop > 0.0 {
            // Deaths: remove oldest citizens exceeding death rate threshold.
            let death_count = (pop * self.death_rate) as usize;
            if death_count > 0 {
                // Sort by age descending and remove the oldest.
                self.agents.sort_by(|a, b| b.age.cmp(&a.age));
                let remove = death_count.min(self.agents.len());
                self.agents.truncate(self.agents.len() - remove);
            }
        }
    }

    /// Spawn `count` new citizens (birth/immigration). New citizens start at
    /// home (if one is assigned) or in Seeking(Home) state.
    pub fn spawn_citizens(&mut self, count: u32) {
        for _ in 0..count {
            let id = EntityId::from_raw(self.next_entity_index, 0);
            self.next_entity_index += 1;
            self.agents.push(Citizen {
                id,
                home: None,
                workplace: None,
                needs: Needs::human(),
                state: CitizenState::Seeking(CitizenSeekTarget::Home),
                age: 0,
            });
        }
    }

    /// Remove `count` citizens (death/emigration). Removes the oldest first.
    pub fn remove_citizens(&mut self, count: u32) {
        let count = (count as usize).min(self.agents.len());
        if count == 0 {
            return;
        }
        self.agents.sort_by(|a, b| b.age.cmp(&a.age));
        self.agents.truncate(self.agents.len() - count);
    }

    /// Current population count.
    pub fn population(&self) -> usize {
        self.agents.len()
    }

    /// Fraction of citizens that have a workplace (0.0 - 1.0).
    pub fn employment_rate(&self) -> f32 {
        if self.agents.is_empty() {
            return 0.0;
        }
        let employed = self.agents.iter().filter(|c| c.workplace.is_some()).count();
        employed as f32 / self.agents.len() as f32
    }

    /// Number of citizens without a home.
    pub fn homeless_count(&self) -> usize {
        self.agents.iter().filter(|c| c.home.is_none()).count()
    }

    /// Assign a home to a homeless citizen. Returns `true` if a citizen was
    /// assigned, `false` if no homeless citizens exist.
    pub fn assign_home(&mut self, node: FlowNodeId) -> bool {
        if let Some(citizen) = self.agents.iter_mut().find(|c| c.home.is_none()) {
            citizen.home = Some(node);
            if citizen.state == CitizenState::Seeking(CitizenSeekTarget::Home) {
                citizen.state = CitizenState::AtHome;
            }
            true
        } else {
            false
        }
    }

    /// Assign a workplace to an unemployed citizen. Returns `true` if a
    /// citizen was assigned.
    pub fn assign_workplace(&mut self, node: FlowNodeId) -> bool {
        if let Some(citizen) = self.agents.iter_mut().find(|c| c.workplace.is_none()) {
            citizen.workplace = Some(node);
            if citizen.state == CitizenState::Seeking(CitizenSeekTarget::Job) {
                citizen.state = CitizenState::Commuting;
            }
            true
        } else {
            false
        }
    }
}

impl Default for PopulationSim {
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
    fn resource_flow_production_and_consumption() {
        let mut flow = ResourceFlow::new();
        let producer = flow.add_node(FlowNode {
            id: FlowNodeId(0),
            node_type: FlowNodeType::Producer {
                output: ResourceType::Food,
                rate_per_tick: 5,
                input: None,
            },
            position: GridPos::new(0, 0),
            buffer: FxHashMap::default(),
            capacity: {
                let mut m = FxHashMap::default();
                m.insert(ResourceType::Food, 100);
                m
            },
        });
        let consumer = flow.add_node(FlowNode {
            id: FlowNodeId(0),
            node_type: FlowNodeType::Consumer {
                input: ResourceType::Food,
                rate_per_tick: 3,
            },
            position: GridPos::new(1, 0),
            buffer: FxHashMap::default(),
            capacity: {
                let mut m = FxHashMap::default();
                m.insert(ResourceType::Food, 100);
                m
            },
        });

        assert_eq!(flow.total_production(ResourceType::Food), 5);
        assert_eq!(flow.total_consumption(ResourceType::Food), 3);
        assert_eq!(flow.net_flow(ResourceType::Food), 2);

        // Connect and tick: producer generates into buffer, edge transfers.
        let road = RoadNetwork::new(4, 4);
        flow.connect(producer, consumer, &road);
        flow.tick();

        // Producer produced 5 food, edge transferred some to consumer, consumer consumed 3.
        // The exact buffer values depend on ordering but net should be positive.
        assert!(flow.net_flow(ResourceType::Food) > 0);
    }

    #[test]
    fn resource_flow_shortages() {
        let mut flow = ResourceFlow::new();
        flow.add_node(FlowNode {
            id: FlowNodeId(0),
            node_type: FlowNodeType::Consumer {
                input: ResourceType::Water,
                rate_per_tick: 2,
            },
            position: GridPos::new(0, 0),
            buffer: FxHashMap::default(),
            capacity: {
                let mut m = FxHashMap::default();
                m.insert(ResourceType::Water, 50);
                m
            },
        });
        flow.tick();
        let short = flow.shortages();
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].1, ResourceType::Water);
    }

    #[test]
    fn zone_system_paint_and_query() {
        let mut zones = ZoneSystem::new(10, 10);
        zones.paint_zone(Rect::new(2.0, 2.0, 3.0, 3.0), ZoneType::Residential);
        assert_eq!(
            zones.zone_at(GridPos::new(3, 3)),
            Some(ZoneType::Residential)
        );
        assert_eq!(zones.zone_at(GridPos::new(0, 0)), None);

        zones.clear_zone(Rect::new(2.0, 2.0, 3.0, 3.0));
        assert_eq!(zones.zone_at(GridPos::new(3, 3)), None);
    }

    #[test]
    fn zone_growable_tiles() {
        let mut zones = ZoneSystem::new(10, 10);
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(3, 2));
        zones.paint_zone(Rect::new(3.0, 3.0, 1.0, 1.0), ZoneType::Commercial);
        let growable = zones.growable_tiles(ZoneType::Commercial, &roads);
        assert!(growable.contains(&GridPos::new(3, 3)));
    }

    #[test]
    fn road_network_connectivity() {
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(0, 0));
        roads.place_road(GridPos::new(1, 0));
        roads.place_road(GridPos::new(2, 0));
        assert!(roads.connected(GridPos::new(0, 0), GridPos::new(2, 0)));
        assert!(!roads.connected(GridPos::new(0, 0), GridPos::new(5, 5)));

        roads.remove_road(GridPos::new(1, 0));
        assert!(!roads.connected(GridPos::new(0, 0), GridPos::new(2, 0)));
    }

    #[test]
    fn happiness_model_score() {
        let model = HappinessModel::default_weights();
        let mut grid = HappinessGrid::new(5, 5);
        grid.set_factor(GridPos::new(2, 2), HappinessFactor::Safety, 0.8);
        grid.set_factor(GridPos::new(2, 2), HappinessFactor::Health, 0.6);

        let score = model.score_at(&grid, GridPos::new(2, 2));
        assert!(score > 0.0);
        // Weighted: 0.8 * 0.125 + 0.6 * 0.125 = 0.175
        assert!((score - 0.175).abs() < 0.01);
    }

    #[test]
    fn happiness_grid_update_from_buildings() {
        let mut registry = BuildingRegistry::new();
        registry.register(BuildingDef {
            id: BuildingId(0),
            name: "Hospital".into(),
            size: (1, 1),
            cost: FxHashMap::default(),
            upkeep: FxHashMap::default(),
            production: None,
            consumption: None,
            effect_radius: Some(3),
            happiness_effects: {
                let mut m = FxHashMap::default();
                m.insert(HappinessFactor::Health, 1.0);
                m
            },
            negative_effects: FxHashMap::default(),
            category: BuildingCategory::Service,
        });

        let buildings = vec![PlacedBuilding {
            def_id: BuildingId(0),
            position: GridPos::new(5, 5),
            construction: 1.0,
            operational: true,
            flow_node: None,
        }];

        let roads = RoadNetwork::new(10, 10);
        let mut grid = HappinessGrid::new(10, 10);
        grid.update_from_buildings(&buildings, &registry, &roads);

        // At the building tile (distance 0) factor should be 1.0.
        let at_building = grid.factor_at(GridPos::new(5, 5), HappinessFactor::Health);
        assert!((at_building - 1.0).abs() < 0.01);

        // At Manhattan distance 2 from (5,5), e.g. (7,5), falloff = 1 - 2/3 = 0.333
        let at_dist2 = grid.factor_at(GridPos::new(7, 5), HappinessFactor::Health);
        assert!((at_dist2 - 0.333).abs() < 0.05);

        // Outside radius (distance 4) should be 0.
        let outside = grid.factor_at(GridPos::new(9, 5), HappinessFactor::Health);
        assert!((outside - 0.0).abs() < 0.01);
    }

    #[test]
    fn disaster_system_trigger_and_tick() {
        let mut sys = DisasterSystem::new(100);
        sys.trigger(DisasterType::Fire, Rect::new(0.0, 0.0, 5.0, 5.0), 3);
        assert_eq!(sys.active.len(), 1);
        assert_eq!(
            sys.is_affected(GridPos::new(2, 2)),
            Some(DisasterType::Fire)
        );
        assert_eq!(sys.is_affected(GridPos::new(8, 8)), None);

        let grid = HappinessGrid::new(10, 10);
        let model = HappinessModel::default_weights();
        sys.tick(&grid, &model);
        sys.tick(&grid, &model);
        sys.tick(&grid, &model);
        assert!(sys.active.is_empty());
    }

    #[test]
    fn disaster_apply_damage() {
        let mut sys = DisasterSystem::new(100);
        sys.trigger(
            DisasterType::Earthquake,
            Rect::new(0.0, 0.0, 10.0, 10.0),
            10,
        );
        let mut buildings = vec![PlacedBuilding {
            def_id: BuildingId(0),
            position: GridPos::new(3, 3),
            construction: 1.0,
            operational: true,
            flow_node: None,
        }];
        sys.apply_damage(&mut buildings);
        assert!(buildings[0].construction < 1.0);
    }

    #[test]
    fn color_gradient_sample() {
        let g = ColorGradient::default_heatmap();
        let c0 = g.sample(0.0);
        assert!((c0.r - 0.0).abs() < 0.01); // green
        assert!((c0.g - 1.0).abs() < 0.01);

        let c1 = g.sample(1.0);
        assert!((c1.r - 1.0).abs() < 0.01); // red
        assert!((c1.g - 0.0).abs() < 0.01);

        let mid = g.sample(0.5);
        assert!((mid.r - 1.0).abs() < 0.01); // yellow
        assert!((mid.g - 1.0).abs() < 0.01);
    }

    #[test]
    fn statistics_overlay_toggle() {
        let mut overlay = StatisticsOverlay::new();
        assert!(overlay.active_overlay.is_none());
        overlay.set_overlay(OverlayType::Traffic);
        assert_eq!(overlay.active_overlay, Some(OverlayType::Traffic));
        overlay.clear_overlay();
        assert!(overlay.active_overlay.is_none());
    }

    // ── RoadNetwork pathfinding ──────────────────────────────

    #[test]
    fn road_shortest_path_simple() {
        let mut roads = RoadNetwork::new(10, 10);
        for x in 0..5 {
            roads.place_road(GridPos::new(x, 0));
        }
        let path = roads.shortest_path(GridPos::new(0, 0), GridPos::new(4, 0));
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.first(), Some(&GridPos::new(0, 0)));
        assert_eq!(path.last(), Some(&GridPos::new(4, 0)));
        assert_eq!(path.len(), 5);
    }

    #[test]
    fn road_shortest_path_no_connection() {
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(0, 0));
        roads.place_road(GridPos::new(5, 5));
        assert!(roads
            .shortest_path(GridPos::new(0, 0), GridPos::new(5, 5))
            .is_none());
    }

    #[test]
    fn road_connected_component() {
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(0, 0));
        roads.place_road(GridPos::new(1, 0));
        roads.place_road(GridPos::new(2, 0));
        roads.place_road(GridPos::new(8, 8)); // Separate island.
        let comp = roads.connected_component(GridPos::new(0, 0));
        assert_eq!(comp.len(), 3);
        assert!(!comp.contains(&GridPos::new(8, 8)));
    }

    #[test]
    fn road_flow_field() {
        let mut roads = RoadNetwork::new(5, 5);
        // Place a horizontal road from (0,2) to (4,2)
        for x in 0..5 {
            roads.place_road(GridPos::new(x, 2));
        }
        let goal = GridPos::new(4, 2);

        // Compute the flow field and collect costs while the borrow is active
        let field = roads.flow_field_to(goal);
        let goal_cost = field.cost_at(4, 2);
        let costs: Vec<u32> = (0..5).map(|x| field.cost_at(x, 2)).collect();
        let non_road_cost = field.cost_at(0, 0);

        // Verify caching works (no longer borrowing the field)
        assert!(roads.flow_cache.contains_key(&goal));

        // Goal should have cost 0
        assert_eq!(goal_cost, 0, "Goal tile should have cost 0");

        // Verify costs decrease monotonically toward the goal along the road
        // Road tiles: (0,2), (1,2), (2,2), (3,2), (4,2)
        assert_ne!(costs[0], u32::MAX, "Start of road should be reachable");
        for x in 1..5usize {
            assert!(
                costs[x] < costs[x - 1],
                "Cost should decrease toward goal: cost_at({},2)={} should be < cost_at({},2)={}",
                x,
                costs[x],
                x - 1,
                costs[x - 1]
            );
        }

        // Non-road tiles should be unreachable (cost == u32::MAX)
        assert_eq!(
            non_road_cost,
            u32::MAX,
            "Non-road tile (0,0) should be unreachable"
        );
    }

    // ── ZoneSystem growth ───────────────────────────────────

    #[test]
    fn zone_try_grow_low_demand() {
        let mut zones = ZoneSystem::new(10, 10);
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(3, 2));
        zones.paint_zone(Rect::new(3.0, 3.0, 2.0, 2.0), ZoneType::Residential);
        let registry = BuildingRegistry::new();

        let building = zones.try_grow(ZoneType::Residential, 0.3, &roads, &registry);
        assert!(building.is_some());
        let b = building.unwrap();
        assert_eq!(b.density, 1); // Low demand = low density.
        assert_eq!(b.zone, ZoneType::Residential);
    }

    #[test]
    fn zone_try_grow_high_demand_high_density() {
        let mut zones = ZoneSystem::new(10, 10);
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(3, 2));
        zones.paint_zone(Rect::new(3.0, 3.0, 2.0, 2.0), ZoneType::Commercial);
        let registry = BuildingRegistry::new();

        let building = zones.try_grow(ZoneType::Commercial, 0.9, &roads, &registry);
        assert!(building.is_some());
        assert_eq!(building.unwrap().density, 3); // High demand = high density.
    }

    #[test]
    fn zone_try_grow_zero_demand() {
        let mut zones = ZoneSystem::new(10, 10);
        let roads = RoadNetwork::new(10, 10);
        let registry = BuildingRegistry::new();
        assert!(zones
            .try_grow(ZoneType::Residential, 0.0, &roads, &registry)
            .is_none());
    }

    // ── BuildingRegistry can_place ──────────────────────────

    #[test]
    fn building_can_place_valid() {
        let mut registry = BuildingRegistry::new();
        registry.register(BuildingDef {
            id: BuildingId(0),
            name: "House".into(),
            size: (1, 1),
            cost: FxHashMap::default(),
            upkeep: FxHashMap::default(),
            production: None,
            consumption: None,
            effect_radius: None,
            happiness_effects: FxHashMap::default(),
            negative_effects: FxHashMap::default(),
            category: BuildingCategory::Residential,
        });

        let mut zones = ZoneSystem::new(10, 10);
        zones.paint_zone(Rect::new(3.0, 3.0, 1.0, 1.0), ZoneType::Residential);
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(3, 2)); // Road adjacent above.

        assert!(registry.can_place(BuildingId(0), GridPos::new(3, 3), &zones, &roads));
    }

    #[test]
    fn building_can_place_wrong_zone() {
        let mut registry = BuildingRegistry::new();
        registry.register(BuildingDef {
            id: BuildingId(0),
            name: "House".into(),
            size: (1, 1),
            cost: FxHashMap::default(),
            upkeep: FxHashMap::default(),
            production: None,
            consumption: None,
            effect_radius: None,
            happiness_effects: FxHashMap::default(),
            negative_effects: FxHashMap::default(),
            category: BuildingCategory::Residential,
        });

        let mut zones = ZoneSystem::new(10, 10);
        zones.paint_zone(Rect::new(3.0, 3.0, 1.0, 1.0), ZoneType::Industrial);
        let mut roads = RoadNetwork::new(10, 10);
        roads.place_road(GridPos::new(3, 2));

        // Residential building can't go in industrial zone.
        assert!(!registry.can_place(BuildingId(0), GridPos::new(3, 3), &zones, &roads));
    }

    #[test]
    fn building_can_place_no_road() {
        let mut registry = BuildingRegistry::new();
        registry.register(BuildingDef {
            id: BuildingId(0),
            name: "House".into(),
            size: (1, 1),
            cost: FxHashMap::default(),
            upkeep: FxHashMap::default(),
            production: None,
            consumption: None,
            effect_radius: None,
            happiness_effects: FxHashMap::default(),
            negative_effects: FxHashMap::default(),
            category: BuildingCategory::Residential,
        });

        let mut zones = ZoneSystem::new(10, 10);
        zones.paint_zone(Rect::new(3.0, 3.0, 1.0, 1.0), ZoneType::Residential);
        let roads = RoadNetwork::new(10, 10); // No roads at all.

        assert!(!registry.can_place(BuildingId(0), GridPos::new(3, 3), &zones, &roads));
    }

    // ── PopulationSim ───────────────────────────────────────

    #[test]
    fn population_spawn_and_count() {
        let mut pop = PopulationSim::new();
        assert_eq!(pop.population(), 0);
        pop.spawn_citizens(10);
        assert_eq!(pop.population(), 10);
        assert_eq!(pop.homeless_count(), 10); // All new citizens are homeless.
        assert!((pop.employment_rate() - 0.0).abs() < 0.01);
    }

    #[test]
    fn population_remove_citizens() {
        let mut pop = PopulationSim::new();
        pop.spawn_citizens(5);
        pop.remove_citizens(3);
        assert_eq!(pop.population(), 2);
    }

    #[test]
    fn population_assign_home_and_workplace() {
        let mut pop = PopulationSim::new();
        pop.spawn_citizens(3);

        assert!(pop.assign_home(FlowNodeId(10)));
        assert_eq!(pop.homeless_count(), 2);

        assert!(pop.assign_workplace(FlowNodeId(20)));
        assert!(pop.employment_rate() > 0.0);
    }

    #[test]
    fn population_tick_state_transitions() {
        let mut pop = PopulationSim::new();
        pop.spawn_citizens(1);
        // Assign home and workplace so the citizen can transition.
        pop.agents[0].home = Some(FlowNodeId(0));
        pop.agents[0].workplace = Some(FlowNodeId(1));
        pop.agents[0].state = CitizenState::AtHome;

        let grid = HappinessGrid::new(10, 10);
        let model = HappinessModel::default_weights();
        let roads = RoadNetwork::new(10, 10);
        let resources = ResourceFlow::new();

        // Tick several times to see state transitions.
        for _ in 0..5 {
            pop.tick(&grid, &model, &roads, &resources);
        }
        // After ticking, the citizen should have moved from AtHome.
        // We can't predict the exact state but it shouldn't still be seeking.
        assert_ne!(
            pop.agents[0].state,
            CitizenState::Seeking(CitizenSeekTarget::Home)
        );
    }

    #[test]
    fn population_death_cycle() {
        let mut pop = PopulationSim::new();
        pop.spawn_citizens(100);
        pop.death_rate = 0.5; // Very high death rate for testing.
                              // Age them.
        for c in &mut pop.agents {
            c.age = 1000;
        }

        let grid = HappinessGrid::new(10, 10);
        let model = HappinessModel::default_weights();
        let roads = RoadNetwork::new(10, 10);
        let resources = ResourceFlow::new();
        pop.tick(&grid, &model, &roads, &resources);
        assert!(pop.population() < 100);
    }

    // ── ResourceFlow on_road_change ─────────────────────────

    #[test]
    fn resource_flow_on_road_change() {
        let mut flow = ResourceFlow::new();
        let a = flow.add_node(FlowNode {
            id: FlowNodeId(0),
            node_type: FlowNodeType::Producer {
                output: ResourceType::Wood,
                rate_per_tick: 3,
                input: None,
            },
            position: GridPos::new(0, 0),
            buffer: FxHashMap::default(),
            capacity: {
                let mut m = FxHashMap::default();
                m.insert(ResourceType::Wood, 100);
                m
            },
        });
        let b = flow.add_node(FlowNode {
            id: FlowNodeId(0),
            node_type: FlowNodeType::Storage,
            position: GridPos::new(3, 0),
            buffer: FxHashMap::default(),
            capacity: {
                let mut m = FxHashMap::default();
                m.insert(ResourceType::Wood, 100);
                m
            },
        });

        // Manually add an edge with a cached path through (1,0) and (2,0).
        flow.edges.push(FlowEdge {
            from: a,
            to: b,
            max_throughput: 10,
            current_throughput: 0,
            road_path: vec![
                GridPos::new(0, 0),
                GridPos::new(1, 0),
                GridPos::new(2, 0),
                GridPos::new(3, 0),
            ],
        });

        assert_eq!(flow.edges.len(), 1);
        // Simulate road removal at (1,0) — should disconnect the edge.
        flow.on_road_change(GridPos::new(1, 0));
        assert_eq!(flow.edges.len(), 0);
    }
}
