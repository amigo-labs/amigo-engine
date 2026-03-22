use crate::ecs::EntityId;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A ballot cast by a voter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ballot {
    pub voter: EntityId,
    /// The choice this voter selected. Semantics defined by the caller.
    pub choice: u32,
}

/// Result of tallying votes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteOutcome {
    /// A single choice won.
    Decided { winner: u32, votes: u32 },
    /// A tie between multiple choices (sorted ascending).
    Tie { choices: Vec<u32>, votes: u32 },
    /// No votes were cast.
    NoVotes,
    /// Majority chose the skip option.
    Skipped,
}

/// Current phase of a voting session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VotePhase {
    /// No vote in progress.
    Inactive,
    /// Accepting ballots.
    Open,
    /// Time expired, tallying.
    Tallying,
    /// Result is available.
    Resolved,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Events produced by the voting system.
#[derive(Clone, Debug)]
pub enum VoteEvent {
    Started { eligible_voters: u32 },
    BallotCast { voter: EntityId, choice: u32 },
    TimeWarning { seconds_remaining: f32 },
    Closed,
    Resolved { outcome: VoteOutcome },
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a voting session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoteConfig {
    /// Duration in seconds that voting is open.
    pub duration: f32,
    /// The choice ID that means "skip" / "abstain".
    pub skip_choice: Option<u32>,
    /// If true, a plurality wins. If false, strict majority (>50%) is required.
    pub plurality_wins: bool,
    /// Time remaining at which to emit a TimeWarning event.
    pub warning_threshold: f32,
}

impl Default for VoteConfig {
    fn default() -> Self {
        Self {
            duration: 30.0,
            skip_choice: Some(0),
            plurality_wins: true,
            warning_threshold: 10.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Voting session
// ---------------------------------------------------------------------------

/// Manages a single voting session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VotingSession {
    config: VoteConfig,
    phase: VotePhase,
    ballots: Vec<Ballot>,
    eligible: FxHashSet<EntityId>,
    timer: f32,
    outcome: Option<VoteOutcome>,
    warning_sent: bool,
}

impl VotingSession {
    pub fn new() -> Self {
        Self {
            config: VoteConfig::default(),
            phase: VotePhase::Inactive,
            ballots: Vec::new(),
            eligible: FxHashSet::default(),
            timer: 0.0,
            outcome: None,
            warning_sent: false,
        }
    }

    /// Start a new voting session with the given eligible voters.
    pub fn start(&mut self, eligible: &[EntityId], config: VoteConfig) {
        self.config = config;
        self.phase = VotePhase::Open;
        self.ballots.clear();
        self.eligible = eligible.iter().copied().collect();
        self.timer = self.config.duration;
        self.outcome = None;
        self.warning_sent = false;
    }

    /// Cast a ballot. Returns false if voter is ineligible or already voted.
    pub fn cast(&mut self, voter: EntityId, choice: u32) -> bool {
        if self.phase != VotePhase::Open {
            return false;
        }
        if !self.eligible.contains(&voter) {
            return false;
        }
        if self.ballots.iter().any(|b| b.voter == voter) {
            return false;
        }
        self.ballots.push(Ballot { voter, choice });
        true
    }

    /// Whether a specific voter has already cast a ballot.
    pub fn has_voted(&self, voter: EntityId) -> bool {
        self.ballots.iter().any(|b| b.voter == voter)
    }

    /// Tick the voting timer. Returns events.
    pub fn update(&mut self, dt: f32) -> Vec<VoteEvent> {
        let mut events = Vec::new();

        if self.phase != VotePhase::Open {
            return events;
        }

        self.timer -= dt;

        // Auto-close when all eligible voters have voted.
        if self.ballots.len() >= self.eligible.len() {
            self.resolve(&mut events);
            return events;
        }

        // Time warning.
        if !self.warning_sent && self.timer <= self.config.warning_threshold && self.timer > 0.0 {
            self.warning_sent = true;
            events.push(VoteEvent::TimeWarning {
                seconds_remaining: self.timer,
            });
        }

        // Time expired.
        if self.timer <= 0.0 {
            self.resolve(&mut events);
        }

        events
    }

    /// Force-close voting early.
    pub fn close_early(&mut self) {
        if self.phase == VotePhase::Open {
            let mut events = Vec::new();
            self.resolve(&mut events);
        }
    }

    /// Get the current phase.
    pub fn phase(&self) -> VotePhase {
        self.phase
    }

    /// Get the outcome (only valid in Resolved phase).
    pub fn outcome(&self) -> Option<&VoteOutcome> {
        self.outcome.as_ref()
    }

    /// How many eligible voters exist / have voted.
    pub fn vote_counts(&self) -> (u32, u32) {
        (self.eligible.len() as u32, self.ballots.len() as u32)
    }

    /// Reset to Inactive for a new round.
    pub fn reset(&mut self) {
        self.phase = VotePhase::Inactive;
        self.ballots.clear();
        self.eligible.clear();
        self.outcome = None;
        self.warning_sent = false;
    }

    fn resolve(&mut self, events: &mut Vec<VoteEvent>) {
        self.phase = VotePhase::Tallying;
        events.push(VoteEvent::Closed);

        let outcome = tally(
            &self.ballots,
            self.config.skip_choice,
            self.config.plurality_wins,
        );
        self.outcome = Some(outcome.clone());
        self.phase = VotePhase::Resolved;
        events.push(VoteEvent::Resolved { outcome });
    }
}

impl Default for VotingSession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Pure tally function
// ---------------------------------------------------------------------------

/// Pure function: tally a set of ballots. Deterministic.
pub fn tally(ballots: &[Ballot], skip_choice: Option<u32>, plurality_wins: bool) -> VoteOutcome {
    if ballots.is_empty() {
        return VoteOutcome::NoVotes;
    }

    // Count votes per choice.
    let mut counts: std::collections::BTreeMap<u32, u32> = std::collections::BTreeMap::new();
    for ballot in ballots {
        *counts.entry(ballot.choice).or_insert(0) += 1;
    }

    // Check if skip won.
    if let Some(skip) = skip_choice {
        if let Some(&skip_votes) = counts.get(&skip) {
            if plurality_wins {
                let max_non_skip = counts
                    .iter()
                    .filter(|(&k, _)| k != skip)
                    .map(|(_, &v)| v)
                    .max()
                    .unwrap_or(0);
                if skip_votes > max_non_skip {
                    return VoteOutcome::Skipped;
                }
            } else if skip_votes > ballots.len() as u32 / 2 {
                return VoteOutcome::Skipped;
            }
        }
    }

    // Find the maximum vote count (excluding skip).
    let non_skip: Vec<(u32, u32)> = counts
        .iter()
        .filter(|(&k, _)| skip_choice != Some(k))
        .map(|(&k, &v)| (k, v))
        .collect();

    if non_skip.is_empty() {
        return VoteOutcome::Skipped;
    }

    let max_votes = non_skip.iter().map(|(_, v)| *v).max().unwrap_or(0);

    // Majority check if required.
    if !plurality_wins && max_votes <= ballots.len() as u32 / 2 {
        let tied: Vec<u32> = non_skip
            .iter()
            .filter(|(_, v)| *v == max_votes)
            .map(|(k, _)| *k)
            .collect();
        return VoteOutcome::Tie {
            choices: tied,
            votes: max_votes,
        };
    }

    let winners: Vec<u32> = non_skip
        .iter()
        .filter(|(_, v)| *v == max_votes)
        .map(|(k, _)| *k)
        .collect();

    if winners.len() == 1 {
        VoteOutcome::Decided {
            winner: winners[0],
            votes: max_votes,
        }
    } else {
        VoteOutcome::Tie {
            choices: winners,
            votes: max_votes,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::EntityId;

    fn eid(n: u32) -> EntityId {
        EntityId::from_raw(n, 0)
    }

    #[test]
    fn tally_decided() {
        let ballots = vec![
            Ballot {
                voter: eid(1),
                choice: 1,
            },
            Ballot {
                voter: eid(2),
                choice: 1,
            },
            Ballot {
                voter: eid(3),
                choice: 2,
            },
        ];
        let result = tally(&ballots, None, true);
        assert_eq!(
            result,
            VoteOutcome::Decided {
                winner: 1,
                votes: 2
            }
        );
    }

    #[test]
    fn tally_tie() {
        let ballots = vec![
            Ballot {
                voter: eid(1),
                choice: 1,
            },
            Ballot {
                voter: eid(2),
                choice: 2,
            },
        ];
        let result = tally(&ballots, None, true);
        assert_eq!(
            result,
            VoteOutcome::Tie {
                choices: vec![1, 2],
                votes: 1
            }
        );
    }

    #[test]
    fn tally_skipped() {
        let ballots = vec![
            Ballot {
                voter: eid(1),
                choice: 0,
            },
            Ballot {
                voter: eid(2),
                choice: 0,
            },
            Ballot {
                voter: eid(3),
                choice: 1,
            },
        ];
        let result = tally(&ballots, Some(0), true);
        assert_eq!(result, VoteOutcome::Skipped);
    }

    #[test]
    fn tally_no_votes() {
        let result = tally(&[], None, true);
        assert_eq!(result, VoteOutcome::NoVotes);
    }

    #[test]
    fn tally_majority_required() {
        // 2 out of 4 votes — not a strict majority.
        let ballots = vec![
            Ballot {
                voter: eid(1),
                choice: 1,
            },
            Ballot {
                voter: eid(2),
                choice: 1,
            },
            Ballot {
                voter: eid(3),
                choice: 2,
            },
            Ballot {
                voter: eid(4),
                choice: 3,
            },
        ];
        let result = tally(&ballots, None, false);
        // 2 votes is not > 2 (4/2), so it's a tie.
        assert!(matches!(result, VoteOutcome::Tie { .. }));
    }

    #[test]
    fn session_lifecycle() {
        let mut session = VotingSession::new();
        let eligible = vec![eid(1), eid(2), eid(3)];

        session.start(&eligible, VoteConfig::default());
        assert_eq!(session.phase(), VotePhase::Open);

        assert!(session.cast(eid(1), 2));
        assert!(session.cast(eid(2), 2));
        // Duplicate vote.
        assert!(!session.cast(eid(1), 3));
        // Ineligible voter.
        assert!(!session.cast(eid(99), 1));

        assert!(session.cast(eid(3), 1));

        // All voted — should auto-close on next update.
        let events = session.update(0.016);
        assert_eq!(session.phase(), VotePhase::Resolved);
        assert!(events
            .iter()
            .any(|e| matches!(e, VoteEvent::Resolved { .. })));
        assert_eq!(
            *session.outcome().unwrap(),
            VoteOutcome::Decided {
                winner: 2,
                votes: 2
            }
        );
    }

    #[test]
    fn session_time_expires() {
        let mut session = VotingSession::new();
        let eligible = vec![eid(1), eid(2)];
        session.start(
            &eligible,
            VoteConfig {
                duration: 5.0,
                warning_threshold: 2.0,
                ..Default::default()
            },
        );

        session.cast(eid(1), 3);
        // Nobody else votes, time runs out.
        let events = session.update(6.0);
        assert_eq!(session.phase(), VotePhase::Resolved);
        assert_eq!(
            *session.outcome().unwrap(),
            VoteOutcome::Decided {
                winner: 3,
                votes: 1
            }
        );
        assert!(events.iter().any(|e| matches!(e, VoteEvent::Closed)));
    }
}
