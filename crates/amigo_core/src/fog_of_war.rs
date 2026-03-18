//! Fog-of-War-System: Tile-basierte Sichtbarkeits-Verwaltung.
//!
//! Bietet eine reine Daten- und Logik-Schicht ohne Rendering-Abhängigkeiten.
//! Das Rendering (Overlay-Texturen, Shader) gehört in `amigo_render`.
//!
//! # Sichtbarkeits-Zustände
//!
//! Jedes Tile kann einen von drei Zuständen haben:
//! - `Hidden`: Tile wurde noch nie gesehen (Shroud)
//! - `Explored`: Tile war einmal sichtbar, liegt aber außerhalb des
//!   aktuellen Sichtfelds (Fog)
//! - `Visible`: Tile liegt im aktiven Sichtradius einer Einheit
//!
//! # Algorithmus
//!
//! `update_visibility` verwendet BFS mit Chebyshev-Distanz als Radius-Maßstab.
//! Ein Tile (dx, dy) ist sichtbar wenn `max(|dx|, |dy|) <= radius`.
//! Kein f32, kein unsafe — vollständig deterministisch.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::math::IVec2;

/// Sichtbarkeitszustand eines einzelnen Tiles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TileVisibility {
    /// Tile wurde noch nie gesehen. Wird als undurchsichtiger Shroud gerendert.
    #[default]
    Hidden,
    /// Tile war einmal sichtbar, liegt aber außerhalb des aktuellen Sichtfelds.
    /// Wird als halbtransparentes Fog-Overlay gerendert.
    Explored,
    /// Tile liegt im aktiven Sichtradius einer Einheit. Vollständig sichtbar.
    Visible,
}

/// 2D-Grid von Tile-Sichtbarkeitswerten für das Fog-of-War-System.
///
/// Speichert einen `TileVisibility`-Wert pro Tile als flaches Vec (row-major).
/// Koordinaten-Ursprung ist (0, 0) oben-links.
///
/// # Beispiel
///
/// ```rust
/// use amigo_core::fog_of_war::{FogOfWarGrid, TileVisibility, update_visibility};
/// use amigo_core::math::IVec2;
///
/// let mut grid = FogOfWarGrid::new(20, 20);
/// update_visibility(IVec2::new(10, 10), 3, &mut grid);
/// assert_eq!(grid.visibility_at(10, 10), TileVisibility::Visible);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FogOfWarGrid {
    data: Vec<TileVisibility>,
    width: u32,
    height: u32,
}

impl FogOfWarGrid {
    /// Erstellt ein neues Grid, bei dem alle Tiles `Hidden` sind.
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            data: vec![TileVisibility::Hidden; size],
            width,
            height,
        }
    }

    /// Gibt die Breite des Grids in Tiles zurück.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Gibt die Höhe des Grids in Tiles zurück.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Gibt den Sichtbarkeitszustand des Tiles an den Koordinaten (x, y) zurück.
    ///
    /// Gibt `Hidden` zurück wenn die Koordinaten außerhalb des Grids liegen —
    /// kein Panic.
    pub fn visibility_at(&self, x: i32, y: i32) -> TileVisibility {
        match self.index_of(x, y) {
            Some(idx) => self.data[idx],
            None => TileVisibility::Hidden,
        }
    }

    /// Setzt den Sichtbarkeitszustand eines Tiles direkt.
    ///
    /// Ignoriert out-of-bounds-Koordinaten ohne Panic.
    pub fn set_visibility(&mut self, x: i32, y: i32, state: TileVisibility) {
        if let Some(idx) = self.index_of(x, y) {
            self.data[idx] = state;
        }
    }

    /// Berechnet den flachen Index für (x, y). Gibt `None` zurück bei
    /// out-of-bounds.
    fn index_of(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            return None;
        }
        let ux = x as u32;
        let uy = y as u32;
        if ux >= self.width || uy >= self.height {
            return None;
        }
        Some((uy as usize) * (self.width as usize) + (ux as usize))
    }
}

/// Aktualisiert die Sichtbarkeit im Grid basierend auf der Position und dem
/// Radius eines Beobachters.
///
/// Ablauf:
/// 1. Alle aktuell `Visible`-Tiles werden auf `Explored` zurückgestuft.
/// 2. BFS vom `observer_pos` ausgehend: alle Tiles mit Chebyshev-Distanz
///    `<= radius` werden auf `Visible` gesetzt.
///
/// Chebyshev-Distanz: `max(|dx|, |dy|)` — erzeugt ein quadratisches
/// Sichtfeld, vollständig deterministisch ohne f32.
///
/// # Parameter
///
/// - `observer_pos`: Tile-Koordinaten des Beobachters (IVec2)
/// - `radius`: Sichtradius in Tiles (Chebyshev-Maßstab)
/// - `grid`: Das zu aktualisierende FogOfWarGrid
pub fn update_visibility(observer_pos: IVec2, radius: u32, grid: &mut FogOfWarGrid) {
    // Schritt 1: Alle Visible-Tiles auf Explored downgraden.
    for tile in grid.data.iter_mut() {
        if *tile == TileVisibility::Visible {
            *tile = TileVisibility::Explored;
        }
    }

    // Schritt 2: BFS vom Observer, alle Tiles innerhalb Chebyshev-Radius
    // auf Visible setzen.
    //
    // Da Chebyshev ein Rechteck ist, könnte man auch direkt iterieren —
    // aber BFS hält die Struktur offen für spätere LOS-Erweiterungen.
    let ox = observer_pos.x;
    let oy = observer_pos.y;

    // Visited-Tracking über ein separates bool-Vec (gleiche Dimensionen).
    let grid_size = (grid.width as usize).saturating_mul(grid.height as usize);
    let mut visited = vec![false; grid_size];

    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();

    // Startpunkt: Observer selbst.
    if let Some(start_idx) = grid.index_of(ox, oy) {
        visited[start_idx] = true;
        queue.push_back((ox, oy));
    }

    // 4-direktional + Diagonal = 8 Nachbarn (Chebyshev-BFS).
    const NEIGHBORS: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    while let Some((cx, cy)) = queue.pop_front() {
        // Chebyshev-Distanz zum Observer prüfen.
        let dx = (cx - ox).unsigned_abs();
        let dy = (cy - oy).unsigned_abs();
        let dist = dx.max(dy);

        if dist > radius {
            // Außerhalb des Radius — nicht sichtbar setzen, nicht weiter propagieren.
            continue;
        }

        // Tile auf Visible setzen.
        grid.set_visibility(cx, cy, TileVisibility::Visible);

        // Nachbarn einreihen, sofern noch nicht besucht und innerhalb Radius+1.
        for (ndx, ndy) in NEIGHBORS {
            let nx = cx + ndx;
            let ny = cy + ndy;

            // Frühzeitig abbrechen wenn Nachbar weit außerhalb des Radius.
            let adx = (nx - ox).unsigned_abs();
            let ady = (ny - oy).unsigned_abs();
            if adx > radius + 1 || ady > radius + 1 {
                continue;
            }

            if let Some(idx) = grid.index_of(nx, ny) {
                if !visited[idx] {
                    visited[idx] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Alle Tiles eines neuen Grids müssen Hidden sein.
    #[test]
    fn new_grid_all_hidden() {
        // Arrange
        let width = 10_u32;
        let height = 10_u32;

        // Act
        let grid = FogOfWarGrid::new(width, height);

        // Assert
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                assert_eq!(
                    grid.visibility_at(x, y),
                    TileVisibility::Hidden,
                    "Tile ({x},{y}) sollte Hidden sein"
                );
            }
        }
    }

    /// Nach update_visibility müssen Tiles innerhalb des Radius Visible sein.
    #[test]
    fn update_makes_nearby_tiles_visible() {
        // Arrange
        let mut grid = FogOfWarGrid::new(20, 20);
        let observer = IVec2::new(5, 5);
        let radius = 3_u32;

        // Act
        update_visibility(observer, radius, &mut grid);

        // Assert: Alle Tiles mit Chebyshev-Distanz <= radius sind Visible.
        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                let x = observer.x + dx;
                let y = observer.y + dy;
                let chebyshev = (dx.unsigned_abs()).max(dy.unsigned_abs());
                if chebyshev <= radius {
                    assert_eq!(
                        grid.visibility_at(x, y),
                        TileVisibility::Visible,
                        "Tile ({x},{y}) bei Chebyshev-Distanz {chebyshev} sollte Visible sein"
                    );
                }
            }
        }
    }

    /// Tiles die vorher sichtbar waren, müssen nach einem neuen Update
    /// (mit anderem Observer) als Explored erhalten bleiben — nicht Hidden.
    #[test]
    fn explored_tiles_stay_explored() {
        // Arrange
        let mut grid = FogOfWarGrid::new(30, 30);
        let first_pos = IVec2::new(5, 5);
        let second_pos = IVec2::new(20, 20);
        let radius = 2_u32;

        // Act: Erster Update — Tiles um (5,5) werden Visible.
        update_visibility(first_pos, radius, &mut grid);
        // Zweiter Update — Observer wechselt zu (20,20).
        update_visibility(second_pos, radius, &mut grid);

        // Assert: Tiles die vorher bei (5,5) sichtbar waren, sind jetzt Explored.
        // (5,5) selbst liegt weit weg von (20,20), daher nicht mehr Visible.
        assert_eq!(
            grid.visibility_at(5, 5),
            TileVisibility::Explored,
            "Tile (5,5) sollte nach zweitem Update Explored sein"
        );
        assert_eq!(
            grid.visibility_at(6, 5),
            TileVisibility::Explored,
            "Tile (6,5) sollte nach zweitem Update Explored sein"
        );
    }

    /// Tiles außerhalb des Radius bleiben Hidden — weder Explored noch Visible.
    #[test]
    fn hidden_beyond_radius() {
        // Arrange
        let mut grid = FogOfWarGrid::new(20, 20);
        let observer = IVec2::new(10, 10);
        let radius = 2_u32;

        // Act
        update_visibility(observer, radius, &mut grid);

        // Assert: Tiles mit Chebyshev-Distanz > radius bleiben Hidden.
        // (0,0) hat Chebyshev-Distanz max(10,10) = 10 > 2 — muss Hidden sein.
        assert_eq!(
            grid.visibility_at(0, 0),
            TileVisibility::Hidden,
            "Tile (0,0) liegt außerhalb des Radius und sollte Hidden sein"
        );
        assert_eq!(
            grid.visibility_at(19, 19),
            TileVisibility::Hidden,
            "Tile (19,19) liegt außerhalb des Radius und sollte Hidden sein"
        );
        // Direkt außerhalb: Chebyshev-Distanz = radius + 1
        let just_outside_x = observer.x + radius as i32 + 1;
        let just_outside_y = observer.y;
        assert_eq!(
            grid.visibility_at(just_outside_x, just_outside_y),
            TileVisibility::Hidden,
            "Tile ({just_outside_x},{just_outside_y}) liegt direkt außerhalb und sollte Hidden sein"
        );
    }

    /// out-of-bounds-Koordinaten dürfen nicht paniken.
    #[test]
    fn out_of_bounds_safe() {
        // Arrange
        let grid = FogOfWarGrid::new(10, 10);

        // Act & Assert: Kein Panic — gibt Hidden zurück.
        assert_eq!(grid.visibility_at(-1, -1), TileVisibility::Hidden);
        assert_eq!(grid.visibility_at(-100, 5), TileVisibility::Hidden);
        assert_eq!(grid.visibility_at(5, -100), TileVisibility::Hidden);
        assert_eq!(grid.visibility_at(10, 10), TileVisibility::Hidden); // == width/height, out-of-bounds
        assert_eq!(grid.visibility_at(100, 100), TileVisibility::Hidden);
        assert_eq!(grid.visibility_at(i32::MIN, i32::MIN), TileVisibility::Hidden);
        assert_eq!(grid.visibility_at(i32::MAX, i32::MAX), TileVisibility::Hidden);
    }
}
