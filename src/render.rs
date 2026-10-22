//! Mermaid rendering of the team graph and the work graph.

use crate::state::State;
use crate::team::Team;
use crate::work::Ticket;

pub fn team_mermaid(team: &Team) -> String {
    let _ = team;
    String::from("flowchart LR\n")
}

pub fn work_mermaid(tickets: &[Ticket], state: &State) -> String {
    let _ = (tickets, state);
    String::from("flowchart TD\n")
}
