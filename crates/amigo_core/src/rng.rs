//! A seeded random number generator for simulation code.
//!
//! `GameContext::rng` is one of these. Replays and lockstep netcode need
//! every machine to draw the same numbers in the same order (ADR-0001), so
//! there is no `thread_rng` or wall-clock seeding inside: a seed fully
//! determines the sequence, on every platform, and the whole state is one
//! `u64` that a replay or a save game can store.
//!
//! The generator is SplitMix64 (Steele, Lea and Flood, 2014). It is not
//! cryptographic; it is fast, has no bad seeds (zero included), and passes
//! BigCrush.

use crate::math::Fix;
use serde::{Deserialize, Serialize};

/// Deterministic random numbers for gameplay. See the [module docs](self).
///
/// ```
/// use amigo_core::SimRng;
///
/// let mut a = SimRng::new(42);
/// let mut b = SimRng::new(a.state());
/// assert_eq!(a.next_u32(), b.next_u32());
/// let roll = a.range(1, 7); // a die: 1..=6
/// assert!((1..7).contains(&roll));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimRng {
    state: u64,
}

impl SimRng {
    /// A generator seeded with `seed`. Every seed is valid.
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The whole internal state. `SimRng::new(rng.state())` continues the
    /// sequence exactly where `rng` stands.
    pub const fn state(&self) -> u64 {
        self.state
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A uniform integer in `0..n`, without modulo bias; 0 when `n` is 0.
    pub fn below(&mut self, n: u32) -> u32 {
        // Lemire's multiply-and-reject.
        let mut m = u64::from(self.next_u32()) * u64::from(n);
        if (m as u32) < n {
            let threshold = n.wrapping_neg() % n;
            while (m as u32) < threshold {
                m = u64::from(self.next_u32()) * u64::from(n);
            }
        }
        (m >> 32) as u32
    }

    /// A uniform integer in `min..max`; `min` when the range is empty.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        if max <= min {
            return min;
        }
        let span = (i64::from(max) - i64::from(min)) as u32;
        (i64::from(min) + i64::from(self.below(span))) as i32
    }

    /// A uniform [`Fix`] in `[0, 1)`: 16 random fraction bits.
    pub fn fix_unit(&mut self) -> Fix {
        Fix::from_bits((self.next_u32() >> 16) as i32)
    }

    /// A uniform [`Fix`] in `[min, max)`, to the last Q16.16 bit; `min` when
    /// the range is empty.
    pub fn fix_range(&mut self, min: Fix, max: Fix) -> Fix {
        if max <= min {
            return min;
        }
        let span = i64::from(max.to_bits()) - i64::from(min.to_bits());
        let offset = i64::from(self.below(span as u32));
        Fix::from_bits((i64::from(min.to_bits()) + offset) as i32)
    }

    /// `true` with probability `p` (clamped to `[0, 1]`).
    pub fn chance(&mut self, p: Fix) -> bool {
        self.fix_unit() < p
    }

    /// A uniformly chosen element, or `None` for an empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        let len = u32::try_from(items.len()).ok()?;
        if len == 0 {
            return None;
        }
        items.get(self.below(len) as usize)
    }

    /// Shuffle `items` in place (Fisher-Yates). Slices longer than
    /// `u32::MAX` only shuffle their first `u32::MAX` elements.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        let len = items.len().min(u32::MAX as usize);
        for i in (1..len).rev() {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_sequence() {
        // SplitMix64's published reference output for seed 0. A change here
        // breaks every recorded replay.
        let mut rng = SimRng::new(0);
        assert_eq!(rng.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        assert_eq!(rng.next_u64(), 0x06c4_5d18_8009_454f);

        let mut rng = SimRng::new(1234);
        let draws: Vec<i32> = (0..8).map(|_| rng.range(-3, 4)).collect();
        assert_eq!(draws, [2, 1, -2, -1, 2, 1, 0, -2]);
        assert_eq!(rng.fix_unit().to_bits(), 0x0000_5d68);
    }

    #[test]
    fn state_resumes_the_sequence() {
        let mut a = SimRng::new(99);
        a.next_u64();
        let mut b = SimRng::new(a.state());
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut rng = SimRng::new(7);
        for _ in 0..10_000 {
            assert!(rng.below(10) < 10);
            assert!((-5..5).contains(&rng.range(-5, 5)));
            let f = rng.fix_unit();
            assert!(f >= Fix::ZERO && f < Fix::ONE);
            let g = rng.fix_range(Fix::from_num(-2), Fix::from_num(3));
            assert!(g >= Fix::from_num(-2) && g < Fix::from_num(3));
        }
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.range(5, 5), 5);
        assert_eq!(rng.range(i32::MIN, i32::MIN + 1), i32::MIN);
        assert!(rng.range(i32::MIN, i32::MAX) < i32::MAX);
        assert_eq!(rng.fix_range(Fix::ONE, Fix::ZERO), Fix::ONE);
        assert!(rng.fix_range(Fix::MIN, Fix::MAX) < Fix::MAX);
    }

    #[test]
    fn below_is_roughly_uniform() {
        let mut rng = SimRng::new(2024);
        let mut counts = [0u32; 6];
        for _ in 0..60_000 {
            counts[rng.below(6) as usize] += 1;
        }
        for c in counts {
            assert!((9_500..10_500).contains(&c), "{counts:?}");
        }
    }

    #[test]
    fn chance_pick_and_shuffle() {
        let mut rng = SimRng::new(5);
        assert!(!rng.chance(Fix::ZERO));
        assert!(rng.chance(Fix::ONE));
        assert_eq!(rng.pick::<u8>(&[]), None);
        assert_eq!(rng.pick(&[9]), Some(&9));

        let mut items: Vec<u32> = (0..50).collect();
        rng.shuffle(&mut items);
        assert_ne!(items, (0..50).collect::<Vec<_>>());
        items.sort_unstable();
        assert_eq!(items, (0..50).collect::<Vec<_>>());
    }
}
