//! Spline-Pfade für glatte Bewegungskurven in der Simulation.
//!
//! Stellt zwei Kurventypen bereit:
//! - [`CatmullRomSpline`]: automatische Tangenten durch Kontrollpunkte (C1-stetig)
//! - [`CubicBezier`]: explizite Kontrollpunkte für präzise Designer-Kontrolle
//!
//! Alle Berechnungen verwenden Fixed-Point-Arithmetik (`Fix = I16F16`) für
//! deterministischen Multiplayer und Replay-Kompatibilität.

use crate::math::{Fix, SimVec2};

// ── Catmull-Rom Spline ────────────────────────────────────────────────────────

/// Catmull-Rom Spline durch eine Folge von Kontrollpunkten.
///
/// Interpoliert C1-stetig durch alle Punkte. An den Endpunkten werden
/// Ghost-Punkte durch Extrapolation erzeugt, sodass der Spline exakt bei
/// `points[0]` beginnt und bei `points[n-1]` endet.
///
/// Mindestens 2 Kontrollpunkte sind erforderlich.
#[derive(Clone, Debug)]
pub struct CatmullRomSpline {
    /// Kontrollpunkte des Splines. Mindestens 2 Punkte erforderlich.
    pub points: Vec<SimVec2>,
}

impl CatmullRomSpline {
    /// Erstellt einen neuen Catmull-Rom Spline aus den gegebenen Kontrollpunkten.
    ///
    /// Mindestens 2 Punkte sind nötig; bei weniger Punkten liefern `sample`
    /// und `tangent` `SimVec2::ZERO`.
    pub fn new(points: Vec<SimVec2>) -> Self {
        Self { points }
    }

    /// Anzahl der Segmente. Ein Segment liegt zwischen zwei benachbarten Punkten.
    /// Gibt 0 zurück wenn weniger als 2 Punkte vorhanden sind.
    pub fn segment_count(&self) -> usize {
        if self.points.len() < 2 {
            return 0;
        }
        self.points.len() - 1
    }

    /// Position auf dem Spline bei Parameter t ∈ [0.0, 1.0].
    ///
    /// t=0.0 → erster Kontrollpunkt, t=1.0 → letzter Kontrollpunkt.
    /// t wird auf [0, 1] geclampt.
    pub fn sample(&self, t: Fix) -> SimVec2 {
        let n = self.points.len();
        if n == 0 {
            return SimVec2::ZERO;
        }
        if n == 1 {
            return self.points[0];
        }

        let t = clamp_fix(t, Fix::ZERO, Fix::ONE);

        // t=1.0 exakt → letzter Punkt (verhindert Segment-Index out-of-range)
        if t >= Fix::ONE {
            return self.points[n - 1];
        }

        let seg_count = Fix::from_num(self.segment_count() as i32);
        let scaled = t * seg_count;
        let seg_idx = scaled.floor().to_num::<usize>();
        let local_t = scaled - Fix::from_num(seg_idx as i32);

        let (p0, p1, p2, p3) = self.segment_points(seg_idx);
        catmull_rom_position(p0, p1, p2, p3, local_t)
    }

    /// Tangenten-Vektor (Ableitung) auf dem Spline bei t ∈ [0.0, 1.0].
    ///
    /// Der Vektor ist nicht normalisiert. Gibt `SimVec2::ZERO` bei weniger
    /// als 2 Punkten zurück.
    pub fn tangent(&self, t: Fix) -> SimVec2 {
        let n = self.points.len();
        if n < 2 {
            return SimVec2::ZERO;
        }

        let t = clamp_fix(t, Fix::ZERO, Fix::ONE);

        let seg_count_usize = self.segment_count();
        let seg_count = Fix::from_num(seg_count_usize as i32);

        // t=1.0 → letztes Segment verwenden
        let clamped_for_seg = if t >= Fix::ONE {
            Fix::ONE - Fix::from_num(1) / (seg_count * Fix::from_num(1000))
        } else {
            t
        };

        let scaled = clamped_for_seg * seg_count;
        let seg_idx = scaled.floor().to_num::<usize>().min(seg_count_usize - 1);
        let local_t = scaled - Fix::from_num(seg_idx as i32);

        let (p0, p1, p2, p3) = self.segment_points(seg_idx);
        catmull_rom_tangent(p0, p1, p2, p3, local_t)
    }

    /// Liefert die 4 Kontrollpunkte für Segment `seg_idx` inklusive Ghost-Punkten.
    fn segment_points(&self, seg_idx: usize) -> (SimVec2, SimVec2, SimVec2, SimVec2) {
        let n = self.points.len();
        let p1 = self.points[seg_idx];
        let p2 = self.points[seg_idx + 1];

        // Ghost-Punkt vor dem ersten Segment: Extrapolation rückwärts
        let p0 = if seg_idx == 0 {
            // p[-1] = 2 * p[0] - p[1]
            self.points[0] * Fix::from_num(2) - self.points[1]
        } else {
            self.points[seg_idx - 1]
        };

        // Ghost-Punkt nach dem letzten Segment: Extrapolation vorwärts
        let p3 = if seg_idx + 2 >= n {
            // p[n] = 2 * p[n-1] - p[n-2]
            self.points[n - 1] * Fix::from_num(2) - self.points[n - 2]
        } else {
            self.points[seg_idx + 2]
        };

        (p0, p1, p2, p3)
    }
}

// ── Catmull-Rom Formeln ───────────────────────────────────────────────────────

/// Berechnet die Position eines Catmull-Rom Segments bei lokalem Parameter t.
///
/// Formel: q(t) = 0.5 * ((2*P1) + (-P0+P2)*t + (2*P0-5*P1+4*P2-P3)*t²
///                       + (-P0+3*P1-3*P2+P3)*t³)
fn catmull_rom_position(p0: SimVec2, p1: SimVec2, p2: SimVec2, p3: SimVec2, t: Fix) -> SimVec2 {
    let half = Fix::from_num(0.5_f32);
    let t2 = t * t;
    let t3 = t2 * t;

    // Koeffizienten
    let c0 = p1 * Fix::from_num(2);
    let c1 = (p2 - p0) * t;
    let c2 = (p0 * Fix::from_num(2) - p1 * Fix::from_num(5) + p2 * Fix::from_num(4) - p3) * t2;
    let c3 = (-p0 + p1 * Fix::from_num(3) - p2 * Fix::from_num(3) + p3) * t3;

    (c0 + c1 + c2 + c3) * half
}

/// Berechnet die Ableitung (Tangente) eines Catmull-Rom Segments bei lokalem t.
///
/// Ableitung der Position-Formel nach t:
/// q'(t) = 0.5 * ((-P0+P2) + 2*(2*P0-5*P1+4*P2-P3)*t + 3*(-P0+3*P1-3*P2+P3)*t²)
fn catmull_rom_tangent(p0: SimVec2, p1: SimVec2, p2: SimVec2, p3: SimVec2, t: Fix) -> SimVec2 {
    let half = Fix::from_num(0.5_f32);
    let t2 = t * t;

    let c0 = p2 - p0;
    let c1 = (p0 * Fix::from_num(2) - p1 * Fix::from_num(5) + p2 * Fix::from_num(4) - p3)
        * (Fix::from_num(2) * t);
    let c2 = (-p0 + p1 * Fix::from_num(3) - p2 * Fix::from_num(3) + p3)
        * (Fix::from_num(3) * t2);

    (c0 + c1 + c2) * half
}

// ── Kubische Bezier Kurve ─────────────────────────────────────────────────────

/// Kubische Bezier-Kurve mit vier expliziten Kontrollpunkten.
///
/// - `p0`: Startpunkt (Kurve beginnt hier)
/// - `p1`: Erster Kontrollpunkt (zieht die Kurve am Anfang)
/// - `p2`: Zweiter Kontrollpunkt (zieht die Kurve am Ende)
/// - `p3`: Endpunkt (Kurve endet hier)
///
/// Die Kurve berührt nur p0 und p3; p1 und p2 sind Handles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    /// Startpunkt der Kurve.
    pub p0: SimVec2,
    /// Erster Kontrollpunkt (Handle am Start).
    pub p1: SimVec2,
    /// Zweiter Kontrollpunkt (Handle am Ende).
    pub p2: SimVec2,
    /// Endpunkt der Kurve.
    pub p3: SimVec2,
}

impl CubicBezier {
    /// Erstellt eine neue kubische Bezier-Kurve.
    pub fn new(p0: SimVec2, p1: SimVec2, p2: SimVec2, p3: SimVec2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// Position auf der Kurve bei t ∈ [0.0, 1.0].
    ///
    /// B(t) = (1-t)³*P0 + 3*(1-t)²*t*P1 + 3*(1-t)*t²*P2 + t³*P3
    ///
    /// t=0.0 → p0, t=1.0 → p3. t wird auf [0, 1] geclampt.
    pub fn sample(&self, t: Fix) -> SimVec2 {
        let t = clamp_fix(t, Fix::ZERO, Fix::ONE);
        let one_minus_t = Fix::ONE - t;

        let om1 = one_minus_t;
        let om2 = om1 * om1;
        let om3 = om2 * om1;

        let t2 = t * t;
        let t3 = t2 * t;

        let three = Fix::from_num(3);

        self.p0 * om3
            + self.p1 * (three * om2 * t)
            + self.p2 * (three * om1 * t2)
            + self.p3 * t3
    }

    /// Tangenten-Vektor (Ableitung) bei t ∈ [0.0, 1.0].
    ///
    /// B'(t) = 3*(1-t)²*(P1-P0) + 6*(1-t)*t*(P2-P1) + 3*t²*(P3-P2)
    ///
    /// Nicht normalisiert. t wird auf [0, 1] geclampt.
    pub fn tangent(&self, t: Fix) -> SimVec2 {
        let t = clamp_fix(t, Fix::ZERO, Fix::ONE);
        let one_minus_t = Fix::ONE - t;

        let om2 = one_minus_t * one_minus_t;
        let t2 = t * t;

        let three = Fix::from_num(3);
        let six = Fix::from_num(6);

        (self.p1 - self.p0) * (three * om2)
            + (self.p2 - self.p1) * (six * one_minus_t * t)
            + (self.p3 - self.p2) * (three * t2)
    }
}

// ── Hilfsfunktionen ───────────────────────────────────────────────────────────

/// Clampt einen Fix-Wert auf [min, max].
#[inline]
fn clamp_fix(v: Fix, min: Fix, max: Fix) -> Fix {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Fix;

    fn fix(v: f32) -> Fix {
        Fix::from_num(v)
    }

    fn pt(x: f32, y: f32) -> SimVec2 {
        SimVec2::from_f32(x, y)
    }

    // Prüft ob zwei SimVec2-Werte innerhalb einer Toleranz übereinstimmen.
    fn approx_eq(a: SimVec2, b: SimVec2, tolerance: f32) -> bool {
        let tol = fix(tolerance);
        let dx = if a.x >= b.x { a.x - b.x } else { b.x - a.x };
        let dy = if a.y >= b.y { a.y - b.y } else { b.y - a.y };
        dx <= tol && dy <= tol
    }

    #[test]
    fn catmull_rom_sample_at_zero_is_first_point() {
        let spline = CatmullRomSpline::new(vec![
            pt(0.0, 0.0),
            pt(10.0, 5.0),
            pt(20.0, 0.0),
            pt(30.0, 5.0),
        ]);
        let result = spline.sample(Fix::ZERO);
        assert!(
            approx_eq(result, pt(0.0, 0.0), 0.01),
            "Erwartet (0, 0), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn catmull_rom_sample_at_one_is_last_point() {
        let spline = CatmullRomSpline::new(vec![
            pt(0.0, 0.0),
            pt(10.0, 5.0),
            pt(20.0, 0.0),
            pt(30.0, 5.0),
        ]);
        let result = spline.sample(Fix::ONE);
        assert!(
            approx_eq(result, pt(30.0, 5.0), 0.01),
            "Erwartet (30, 5), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn catmull_rom_midpoint_between_two_points() {
        // Mit nur 2 Punkten und Ghost-Punkten durch Extrapolation:
        // p[-1] = 2*(0,0) - (10,0) = (-10,0)
        // p[2]  = 2*(10,0) - (0,0) = (20,0)
        // Catmull-Rom mit p0=(-10,0), p1=(0,0), p2=(10,0), p3=(20,0) bei t=0.5
        // = 0.5 * (2*(0,0) + (10,0-(-10,0))*0.5 + (2*(-10,0)-5*(0,0)+4*(10,0)-(20,0))*0.25
        //          + ((-(-10,0))+3*(0,0)-3*(10,0)+(20,0))*0.125)
        // Vereinfacht: x-Achse linear → Mittelpunkt = (5, 0)
        let spline = CatmullRomSpline::new(vec![pt(0.0, 0.0), pt(10.0, 0.0)]);
        let result = spline.sample(fix(0.5));
        assert!(
            approx_eq(result, pt(5.0, 0.0), 0.1),
            "Erwartet Mittelpunkt (5, 0), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn bezier_sample_at_zero_is_p0() {
        let bezier = CubicBezier::new(pt(1.0, 2.0), pt(5.0, 8.0), pt(9.0, 8.0), pt(12.0, 2.0));
        let result = bezier.sample(Fix::ZERO);
        assert!(
            approx_eq(result, pt(1.0, 2.0), 0.01),
            "Erwartet p0=(1,2), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn bezier_sample_at_one_is_p3() {
        let bezier = CubicBezier::new(pt(1.0, 2.0), pt(5.0, 8.0), pt(9.0, 8.0), pt(12.0, 2.0));
        let result = bezier.sample(Fix::ONE);
        assert!(
            approx_eq(result, pt(12.0, 2.0), 0.01),
            "Erwartet p3=(12,2), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn bezier_straight_line_midpoint() {
        // Gerade Linie: p0=(0,0), p1=(33,0), p2=(66,0), p3=(100,0)
        // Bei t=0.5 sollte der Mittelpunkt bei (50, 0) liegen (±2px Toleranz)
        let bezier = CubicBezier::new(pt(0.0, 0.0), pt(33.0, 0.0), pt(66.0, 0.0), pt(100.0, 0.0));
        let result = bezier.sample(fix(0.5));
        assert!(
            approx_eq(result, pt(50.0, 0.0), 2.0),
            "Erwartet ~(50, 0), erhalten: {:?}",
            result
        );
    }

    #[test]
    fn catmull_rom_tangent_not_zero_at_midpoint() {
        let spline = CatmullRomSpline::new(vec![
            pt(0.0, 0.0),
            pt(10.0, 5.0),
            pt(20.0, 0.0),
            pt(30.0, 5.0),
        ]);
        let tangent = spline.tangent(fix(0.5));
        assert!(
            tangent != SimVec2::ZERO,
            "Tangente bei t=0.5 sollte nicht ZERO sein, erhalten: {:?}",
            tangent
        );
    }

    #[test]
    fn bezier_tangent_at_zero_direction() {
        // B'(0) = 3*(P1-P0) → zeigt in Richtung P1-P0
        let p0 = pt(0.0, 0.0);
        let p1 = pt(5.0, 3.0);
        let p2 = pt(8.0, 6.0);
        let p3 = pt(10.0, 0.0);
        let bezier = CubicBezier::new(p0, p1, p2, p3);

        let tangent = bezier.tangent(Fix::ZERO);
        let expected_dir = p1 - p0; // (5, 3) skaliert um 3 → (15, 9)

        // Prüfe: Vorzeichen der Komponenten stimmen überein
        assert!(
            tangent.x > Fix::ZERO && tangent.y > Fix::ZERO,
            "Tangente bei t=0 sollte positiv x und y haben (Richtung P1-P0), erhalten: {:?}",
            tangent
        );
        // Prüfe Proportionalität: tangent.x / tangent.y ≈ expected_dir.x / expected_dir.y
        // tangent = 3 * (p1 - p0) = 3 * (5, 3) = (15, 9)
        assert!(
            approx_eq(tangent, SimVec2::new(fix(15.0), fix(9.0)), 0.1),
            "Erwartet Tangente (15, 9) = 3*(P1-P0), erhalten: {:?}",
            tangent
        );
        let _ = expected_dir; // verhindert unused-warning
    }
}
