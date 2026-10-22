//! `wteam wait`.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::Out;
use crate::cli::{Global, WaitArgs};
use crate::state::{self, Event, State, TicketStatus};

/// Poll interval for `state.json`.
const POLL: Duration = Duration::from_millis(200);

/// True when no agent is busy and no ticket is with a worker or reviewer.
fn quiescent(s: &State) -> bool {
    s.agents.values().all(|a| !a.busy)
        && s.tickets
            .values()
            .all(|t| !matches!(t.status, TicketStatus::Doing | TicketStatus::Review))
}

/// Take the pending events and mark them consumed.
fn consume(paths: &crate::paths::Paths) -> Result<Vec<Event>> {
    state::with_state(paths, |s| {
        let events = s.pending_events();
        if let Some(last) = events.last() {
            s.events_consumed = last.seq;
        }
        Ok(events)
    })
}

pub fn run(g: &Global, a: &WaitArgs) -> Result<Out> {
    let paths = super::effort(g)?;
    let deadline = Instant::now() + Duration::from_secs(a.timeout);
    let events = loop {
        let s = state::load(&paths)?;
        if !s.pending_events().is_empty() || quiescent(&s) || Instant::now() >= deadline {
            let events = consume(&paths)?;
            // Events may land between the check and the lock; a quiet state with none is a normal return.
            break events;
        }
        std::thread::sleep(POLL);
    };
    let text = crate::watcher::format_events(&events);
    Ok(Out::new(text, serde_json::json!({ "events": events })))
}
