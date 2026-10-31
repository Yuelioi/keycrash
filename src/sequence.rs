//! Diagnostic consistency between a sampled live binding and input evidence.
//! Windows uses confirmed test sends; macOS Carbon receipts are not replayed.
//! This is not sender authentication or proof of an exclusive owner.
#[cfg(windows)]
use crate::recording::{KeyEvent, Response};
#[cfg(windows)]
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Binding {
    pub key1: u32,
    pub key2: u32,
    pub modifiers: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Snapshot {
    pub profile: u32,
    pub bindings: [Binding; 5],
}
#[derive(Clone, Debug)]
pub struct Association {
    pub first_event: usize,
    pub last_event: usize,
}
#[cfg(windows)]
struct Completion {
    slot: usize,
    first: usize,
    last: usize,
    tick: u32,
}
#[cfg(windows)]
#[derive(Clone, Copy, Default)]
struct Pending {
    tick: u32,
    first: usize,
}

// Message timestamps and input timestamps share a DWORD clock. Keep upgrades
// conservative around its 500ms boundary and around delayed callback delivery.
#[cfg(windows)]
const CLOCK_MARGIN: u32 = 32;

#[cfg(windows)]
fn replay(events: &[KeyEvent], snapshot: &Snapshot) -> Option<Vec<Completion>> {
    let mut held = [false; 256];
    let mut pending = [Pending::default(); 5];
    let mut completions = Vec::new();
    let mut previous_tick: Option<u32> = None;
    for (index, event) in events.iter().enumerate() {
        let key = event.code as usize;
        if key >= held.len() {
            return None;
        }
        if event.initial {
            held[key] = event.down;
            continue;
        }
        let tick = event.native_tick?;
        let observed = event.observed_tick?;
        if observed.wrapping_sub(tick) > CLOCK_MARGIN {
            return None;
        }
        if previous_tick.is_some_and(|last| tick.wrapping_sub(last) > 60_000) {
            return None;
        }
        previous_tick = Some(tick);
        if event.down {
            // GetAsyncKeyState in GoldenDict's callback sees state BEFORE this
            // event; updating a modifier first would change its state machine.
            let modifiers = u32::from(held[0x12] || held[0xA4] || held[0xA5])
                | (u32::from(held[0x11] || held[0xA2] || held[0xA3]) << 1)
                | (u32::from(held[0x10] || held[0xA0] || held[0xA1]) << 2)
                | (u32::from(held[0x5B] || held[0x5C]) << 3);
            let mut completed = None;
            // The complete second-stage scan precedes the first-stage scan.
            for (slot, binding) in snapshot.bindings.iter().enumerate() {
                if binding.key1 == 0 {
                    break;
                }
                if binding.key2 == 0 || pending[slot].tick == 0 {
                    continue;
                }
                if binding.key2 == event.code && binding.modifiers == modifiers {
                    let gap = tick.wrapping_sub(pending[slot].tick);
                    // Either branch changes the state used by later keys. If
                    // processing-time uncertainty can cross the boundary, the
                    // rest of this replay is uncertain too.
                    if gap.abs_diff(500) <= CLOCK_MARGIN {
                        return None;
                    }
                    if gap <= 500 {
                        completed = Some(Completion {
                            slot,
                            first: pending[slot].first,
                            last: index,
                            tick,
                        });
                        break;
                    }
                    // The vendor refreshes this timer on a late SECOND key,
                    // even when that key is not the configured first key.
                    pending[slot].tick = tick;
                } else {
                    pending[slot] = Pending::default();
                }
            }
            if completed.is_none() {
                for (slot, binding) in snapshot.bindings.iter().enumerate() {
                    if binding.key1 == 0 {
                        break;
                    }
                    if binding.key1 == event.code && binding.modifiers == modifiers {
                        if binding.key2 == 0 {
                            completed = Some(Completion {
                                slot,
                                first: index,
                                last: index,
                                tick,
                            });
                        } else {
                            pending[slot] = Pending { tick, first: index };
                        }
                        break;
                    }
                }
            }
            if let Some(completion) = completed {
                pending = [Pending::default(); 5];
                completions.push(completion);
            }
        }
        // Releases change future modifier state, but do not reset timers.
        held[key] = event.down;
    }
    Some(completions)
}

#[cfg(windows)]
pub fn associate(events: &[KeyEvent], responses: &mut [Response], incomplete: bool) {
    for response in responses.iter_mut() {
        response.association = None;
    }
    if incomplete {
        return;
    }
    let mut configurations = BTreeMap::new();
    let mut changed = BTreeSet::new();
    for response in responses.iter().filter(|r| r.kind == 2) {
        let recipient = (response.pid, response.created, response.module);
        let snapshot = response.binding.clone();
        if let Some(previous) = configurations.insert(recipient, snapshot.clone())
            && previous != snapshot
        {
            changed.insert(recipient);
        }
    }
    let mut cache: BTreeMap<Snapshot, Option<Vec<Completion>>> = BTreeMap::new();
    let mut consumed = BTreeSet::new();
    let mut order: Vec<_> = (0..responses.len()).collect();
    order.sort_by_key(|i| responses[*i].elapsed);
    for index in order {
        let response = &mut responses[index];
        let recipient = (response.pid, response.created, response.module);
        if response.kind != 2
            || response.created == 0
            || response.module == 0
            || changed.contains(&recipient)
        {
            continue;
        }
        let Some(snapshot) = &response.binding else {
            continue;
        };
        if !matches!(snapshot.profile, 1 | 2) {
            continue;
        }
        let Some(completions) = cache
            .entry(snapshot.clone())
            .or_insert_with(|| replay(events, snapshot))
        else {
            continue;
        };
        let candidates: Vec<_> = completions
            .iter()
            .filter(|completion| {
                completion.slot == response.value as usize
                    && response.native_tick.wrapping_sub(completion.tick) <= CLOCK_MARGIN
                    && !consumed.contains(&(
                        recipient,
                        completion.slot,
                        completion.first,
                        completion.last,
                    ))
            })
            .collect();
        if candidates.len() == 1 {
            let completion = candidates[0];
            consumed.insert((
                recipient,
                completion.slot,
                completion.first,
                completion.last,
            ));
            response.association = Some(Association {
                first_event: completion.first,
                last_event: completion.last,
            });
        }
    }
}
