use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Calendar system — day/season cycle
// ---------------------------------------------------------------------------

/// Seasons in a year.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn next(self) -> Self {
        match self {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Autumn,
            Season::Autumn => Season::Winter,
            Season::Winter => Season::Spring,
        }
    }

    pub fn index(self) -> u32 {
        match self {
            Season::Spring => 0,
            Season::Summer => 1,
            Season::Autumn => 2,
            Season::Winter => 3,
        }
    }
}

/// Events produced by the calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarEvent {
    DayChanged { day: u32, season: Season, year: u32 },
    SeasonChanged { season: Season, year: u32 },
    YearChanged { year: u32 },
}

/// Calendar that tracks day/season/year based on ticks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calendar {
    pub ticks_per_day: u32,
    pub days_per_season: u32,
    pub day: u32,
    pub season: Season,
    pub year: u32,
    tick_counter: u32,
}

impl Calendar {
    pub fn new(ticks_per_day: u32, days_per_season: u32) -> Self {
        Self {
            ticks_per_day,
            days_per_season,
            day: 1,
            season: Season::Spring,
            year: 1,
            tick_counter: 0,
        }
    }

    /// Returns the time of day as a fraction 0.0..1.0.
    pub fn day_progress(&self) -> f32 {
        self.tick_counter as f32 / self.ticks_per_day as f32
    }

    /// Returns the hour (0..23) based on day progress.
    pub fn hour(&self) -> u32 {
        (self.day_progress() * 24.0) as u32
    }

    /// Total number of days elapsed since the start.
    pub fn total_days(&self) -> u32 {
        let seasons_per_year = 4;
        ((self.year - 1) * seasons_per_year * self.days_per_season)
            + (self.season.index() * self.days_per_season)
            + (self.day - 1)
    }

    /// Advance by one tick, returning any events.
    pub fn tick(&mut self) -> Vec<CalendarEvent> {
        let mut events = Vec::new();
        self.tick_counter += 1;

        if self.tick_counter >= self.ticks_per_day {
            self.tick_counter = 0;
            self.day += 1;

            if self.day > self.days_per_season {
                self.day = 1;
                let next_season = self.season.next();
                if next_season == Season::Spring && self.season == Season::Winter {
                    self.year += 1;
                    events.push(CalendarEvent::YearChanged { year: self.year });
                }
                self.season = next_season;
                events.push(CalendarEvent::SeasonChanged {
                    season: self.season,
                    year: self.year,
                });
            }

            events.push(CalendarEvent::DayChanged {
                day: self.day,
                season: self.season,
                year: self.year,
            });
        }

        events
    }
}

// ---------------------------------------------------------------------------
// Growth system — stage-based growth with timers
// ---------------------------------------------------------------------------

/// A single stage in a growth process.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrowthStage {
    /// Duration in ticks to complete this stage.
    pub duration: u32,
    /// If true, the entity needs to be watered to progress in this stage.
    pub needs_water: bool,
}

impl GrowthStage {
    pub fn new(duration: u32) -> Self {
        Self {
            duration,
            needs_water: false,
        }
    }

    pub fn with_water(mut self) -> Self {
        self.needs_water = true;
        self
    }
}

/// Definition for something that grows (crop, tree, animal, etc).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrowthDef {
    pub id: u32,
    pub name: String,
    pub stages: Vec<GrowthStage>,
    /// Seasons where growth is allowed. Empty = all seasons.
    pub allowed_seasons: Vec<Season>,
}

impl GrowthDef {
    pub fn new(id: u32, name: impl Into<String>, stages: Vec<GrowthStage>) -> Self {
        Self {
            id,
            name: name.into(),
            stages,
            allowed_seasons: Vec::new(),
        }
    }

    pub fn with_seasons(mut self, seasons: Vec<Season>) -> Self {
        self.allowed_seasons = seasons;
        self
    }

    pub fn total_duration(&self) -> u32 {
        self.stages.iter().map(|s| s.duration).sum()
    }
}

/// Events produced by the growth system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrowthEvent {
    StageAdvanced { instance_id: u32, new_stage: u32 },
    Completed { instance_id: u32 },
    Withered { instance_id: u32 },
}

/// A running growth instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrowthInstance {
    pub id: u32,
    pub def_id: u32,
    pub current_stage: u32,
    pub ticks_in_stage: u32,
    pub watered: bool,
    pub withered: bool,
    /// How many ticks without water before withering. 0 = never withers.
    pub wither_threshold: u32,
    pub dry_ticks: u32,
}

impl GrowthInstance {
    pub fn new(id: u32, def_id: u32) -> Self {
        Self {
            id,
            def_id,
            current_stage: 0,
            ticks_in_stage: 0,
            watered: false,
            withered: false,
            wither_threshold: 0,
            dry_ticks: 0,
        }
    }

    pub fn with_wither_threshold(mut self, ticks: u32) -> Self {
        self.wither_threshold = ticks;
        self
    }

    pub fn water(&mut self) {
        self.watered = true;
        self.dry_ticks = 0;
    }

    pub fn is_complete(&self, def: &GrowthDef) -> bool {
        self.current_stage as usize >= def.stages.len()
    }
}

/// Tick all growth instances, returning events.
pub fn tick_growth(
    instances: &mut [GrowthInstance],
    defs: &[GrowthDef],
    current_season: Season,
) -> Vec<GrowthEvent> {
    let mut events = Vec::new();

    for inst in instances.iter_mut() {
        if inst.withered {
            continue;
        }

        let def = match defs.iter().find(|d| d.id == inst.def_id) {
            Some(d) => d,
            None => continue,
        };

        // Check if already complete
        if inst.current_stage as usize >= def.stages.len() {
            continue;
        }

        // Check season
        if !def.allowed_seasons.is_empty() && !def.allowed_seasons.contains(&current_season) {
            continue;
        }

        let stage = &def.stages[inst.current_stage as usize];

        // Check water requirement
        if stage.needs_water && !inst.watered {
            if inst.wither_threshold > 0 {
                inst.dry_ticks += 1;
                if inst.dry_ticks >= inst.wither_threshold {
                    inst.withered = true;
                    events.push(GrowthEvent::Withered { instance_id: inst.id });
                }
            }
            continue;
        }

        inst.ticks_in_stage += 1;

        if inst.ticks_in_stage >= stage.duration {
            inst.current_stage += 1;
            inst.ticks_in_stage = 0;
            inst.watered = false;

            if inst.current_stage as usize >= def.stages.len() {
                events.push(GrowthEvent::Completed { instance_id: inst.id });
            } else {
                events.push(GrowthEvent::StageAdvanced {
                    instance_id: inst.id,
                    new_stage: inst.current_stage,
                });
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Farm grid — interactable tile grid
// ---------------------------------------------------------------------------

/// State of a soil tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SoilState {
    /// Untouched ground.
    Empty,
    /// Tilled but nothing planted.
    Tilled,
    /// Has something planted (linked to a GrowthInstance).
    Planted { growth_id: u32 },
}

/// A single farm tile.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FarmTile {
    pub soil: SoilState,
    pub moisture: f32,
    pub fertility: f32,
}

impl Default for FarmTile {
    fn default() -> Self {
        Self {
            soil: SoilState::Empty,
            moisture: 0.0,
            fertility: 1.0,
        }
    }
}

/// Events produced by farm grid operations.
#[derive(Clone, Debug, PartialEq)]
pub enum FarmEvent {
    Tilled { x: u32, y: u32 },
    Watered { x: u32, y: u32 },
    Planted { x: u32, y: u32, growth_id: u32 },
    Harvested { x: u32, y: u32, growth_id: u32 },
}

/// A grid of interactable farm tiles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FarmGrid {
    pub width: u32,
    pub height: u32,
    tiles: Vec<FarmTile>,
    /// How much moisture decreases per tick (evaporation).
    pub moisture_decay: f32,
}

impl FarmGrid {
    pub fn new(width: u32, height: u32) -> Self {
        let tiles = vec![FarmTile::default(); (width * height) as usize];
        Self {
            width,
            height,
            tiles,
            moisture_decay: 0.001,
        }
    }

    pub fn with_moisture_decay(mut self, decay: f32) -> Self {
        self.moisture_decay = decay;
        self
    }

    pub fn get(&self, x: u32, y: u32) -> Option<&FarmTile> {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            Some(&self.tiles[idx])
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut FarmTile> {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            Some(&mut self.tiles[idx])
        } else {
            None
        }
    }

    /// Till a tile (prepare for planting).
    pub fn till(&mut self, x: u32, y: u32) -> Option<FarmEvent> {
        let tile = self.get_mut(x, y)?;
        if tile.soil == SoilState::Empty {
            tile.soil = SoilState::Tilled;
            Some(FarmEvent::Tilled { x, y })
        } else {
            None
        }
    }

    /// Water a tile (increase moisture).
    pub fn water(&mut self, x: u32, y: u32, amount: f32) -> Option<FarmEvent> {
        let tile = self.get_mut(x, y)?;
        tile.moisture = (tile.moisture + amount).min(1.0);
        Some(FarmEvent::Watered { x, y })
    }

    /// Plant on a tilled tile.
    pub fn plant(&mut self, x: u32, y: u32, growth_id: u32) -> Option<FarmEvent> {
        let tile = self.get_mut(x, y)?;
        if tile.soil == SoilState::Tilled {
            tile.soil = SoilState::Planted { growth_id };
            Some(FarmEvent::Planted { x, y, growth_id })
        } else {
            None
        }
    }

    /// Harvest from a planted tile, returning it to tilled state.
    pub fn harvest(&mut self, x: u32, y: u32) -> Option<FarmEvent> {
        let tile = self.get_mut(x, y)?;
        if let SoilState::Planted { growth_id } = tile.soil {
            tile.soil = SoilState::Tilled;
            Some(FarmEvent::Harvested { x, y, growth_id })
        } else {
            None
        }
    }

    /// Tick moisture decay on all tiles.
    pub fn tick_moisture(&mut self) {
        let decay = self.moisture_decay;
        for tile in &mut self.tiles {
            tile.moisture = (tile.moisture - decay).max(0.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_day_advance() {
        let mut cal = Calendar::new(10, 28); // 10 ticks per day
        let mut day_events = Vec::new();

        for _ in 0..10 {
            day_events.extend(cal.tick());
        }

        assert_eq!(cal.day, 2);
        assert!(day_events.iter().any(|e| matches!(e, CalendarEvent::DayChanged { day: 2, .. })));
    }

    #[test]
    fn calendar_season_advance() {
        let mut cal = Calendar::new(1, 3); // 1 tick per day, 3 days per season

        // 3 ticks = 3 days = end of spring
        for _ in 0..3 {
            cal.tick();
        }
        assert_eq!(cal.season, Season::Summer);
        assert_eq!(cal.day, 1);
    }

    #[test]
    fn calendar_year_advance() {
        let mut cal = Calendar::new(1, 2); // 1 tick/day, 2 days/season = 8 days/year
        let mut year_event = false;

        for _ in 0..8 {
            let events = cal.tick();
            if events.iter().any(|e| matches!(e, CalendarEvent::YearChanged { .. })) {
                year_event = true;
            }
        }
        assert_eq!(cal.year, 2);
        assert!(year_event);
    }

    #[test]
    fn calendar_hour() {
        let mut cal = Calendar::new(24, 28);
        // After 6 ticks out of 24, should be hour 6
        for _ in 0..6 {
            cal.tick();
        }
        assert_eq!(cal.hour(), 6);
    }

    #[test]
    fn calendar_total_days() {
        let mut cal = Calendar::new(1, 7);
        // Go through 10 days
        for _ in 0..10 {
            cal.tick();
        }
        assert_eq!(cal.total_days(), 10);
    }

    #[test]
    fn growth_basic() {
        let def = GrowthDef::new(0, "Wheat", vec![
            GrowthStage::new(5),
            GrowthStage::new(5),
        ]);
        let mut inst = vec![GrowthInstance::new(0, 0)];

        // Tick 5 times — should advance to stage 1
        for _ in 0..5 {
            tick_growth(&mut inst, &[def.clone()], Season::Spring);
        }
        assert_eq!(inst[0].current_stage, 1);

        // Tick 5 more — should complete
        let mut completed = false;
        for _ in 0..5 {
            let events = tick_growth(&mut inst, &[def.clone()], Season::Spring);
            if events.iter().any(|e| matches!(e, GrowthEvent::Completed { .. })) {
                completed = true;
            }
        }
        assert!(completed);
    }

    #[test]
    fn growth_needs_water() {
        let def = GrowthDef::new(0, "Tomato", vec![
            GrowthStage::new(3).with_water(),
        ]);
        let mut inst = vec![GrowthInstance::new(0, 0)];

        // Tick without watering — should not progress
        for _ in 0..5 {
            tick_growth(&mut inst, &[def.clone()], Season::Spring);
        }
        assert_eq!(inst[0].ticks_in_stage, 0);

        // Water and tick — should progress
        inst[0].water();
        for _ in 0..3 {
            tick_growth(&mut inst, &[def.clone()], Season::Spring);
        }
        assert!(inst[0].is_complete(&def));
    }

    #[test]
    fn growth_withers() {
        let def = GrowthDef::new(0, "Rose", vec![
            GrowthStage::new(10).with_water(),
        ]);
        let mut inst = vec![GrowthInstance::new(0, 0).with_wither_threshold(3)];

        let mut withered = false;
        for _ in 0..5 {
            let events = tick_growth(&mut inst, &[def.clone()], Season::Spring);
            if events.iter().any(|e| matches!(e, GrowthEvent::Withered { .. })) {
                withered = true;
            }
        }
        assert!(withered);
        assert!(inst[0].withered);
    }

    #[test]
    fn growth_season_restriction() {
        let def = GrowthDef::new(0, "Sunflower", vec![
            GrowthStage::new(3),
        ]).with_seasons(vec![Season::Summer]);
        let mut inst = vec![GrowthInstance::new(0, 0)];

        // Tick in spring — no progress
        for _ in 0..5 {
            tick_growth(&mut inst, &[def.clone()], Season::Spring);
        }
        assert_eq!(inst[0].ticks_in_stage, 0);

        // Tick in summer — should progress
        for _ in 0..3 {
            tick_growth(&mut inst, &[def.clone()], Season::Summer);
        }
        assert!(inst[0].is_complete(&def));
    }

    #[test]
    fn farm_grid_workflow() {
        let mut grid = FarmGrid::new(4, 4);

        // Till
        let event = grid.till(1, 1);
        assert!(matches!(event, Some(FarmEvent::Tilled { x: 1, y: 1 })));

        // Can't till again
        assert!(grid.till(1, 1).is_none());

        // Water
        grid.water(1, 1, 0.5);
        assert_eq!(grid.get(1, 1).unwrap().moisture, 0.5);

        // Plant
        let event = grid.plant(1, 1, 42);
        assert!(matches!(event, Some(FarmEvent::Planted { growth_id: 42, .. })));

        // Can't plant on unplanted tile
        assert!(grid.plant(0, 0, 1).is_none());

        // Harvest
        let event = grid.harvest(1, 1);
        assert!(matches!(event, Some(FarmEvent::Harvested { growth_id: 42, .. })));

        // Tile is back to tilled
        assert_eq!(grid.get(1, 1).unwrap().soil, SoilState::Tilled);
    }

    #[test]
    fn farm_grid_moisture_decay() {
        let mut grid = FarmGrid::new(2, 2).with_moisture_decay(0.1);
        grid.water(0, 0, 0.5);

        grid.tick_moisture();
        let m = grid.get(0, 0).unwrap().moisture;
        assert!((m - 0.4).abs() < 0.01);

        // Decay to zero
        for _ in 0..10 {
            grid.tick_moisture();
        }
        assert_eq!(grid.get(0, 0).unwrap().moisture, 0.0);
    }

    #[test]
    fn farm_grid_bounds() {
        let grid = FarmGrid::new(2, 2);
        assert!(grid.get(5, 5).is_none());
    }
}
