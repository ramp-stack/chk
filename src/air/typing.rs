use std::time::{SystemTime, UNIX_EPOCH};
use prism::air::{Contract, Id, Metadata, Name};
use serde::{Deserialize, Serialize};

/// Activity only, never draft text. Separate from chat history and notifications.
#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct TypingState {
    pub room: Id,
    pub author: Name,
    pub shared_with: Vec<Name>,
    active: bool,
    updated_at: u64,
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub enum TypingAction {
    Share(Name),
    Activity {active: bool, at: u64},
}

impl Contract for TypingState {
    type Init = Id;
    type Message = TypingAction;
    type Result = ();
    fn id() -> Id {Id::hash("CHK.TypingState.v1")}
    fn init(room: Id, metadata: Metadata) -> Self {
        Self {room, author: metadata.signer, shared_with: vec![], active: false, updated_at: 0}
    }
    fn apply(&mut self, action: TypingAction, metadata: Metadata) {
        if metadata.signer != self.author {return;}
        match action {
            TypingAction::Share(name) => {
                if !self.shared_with.contains(&name) {self.shared_with.push(name);}
            }
            TypingAction::Activity {active, at} if at >= self.updated_at => {
                self.active = active;
                self.updated_at = at;
            }
            _ => {}
        }
    }
}

impl TypingState {
    pub const LIFETIME_MS: u64 = 6_000;

    pub fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
    }

    pub fn is_typing(&self, now: u64, last_message_at: Option<u64>) -> bool {
        self.active
            // Allow small clock skew, but never display a far-future update.
            && self.updated_at <= now.saturating_add(2_000)
            && now < self.updated_at.saturating_add(Self::LIFETIME_MS)
            && last_message_at.is_none_or(|sent| sent < self.updated_at)
    }
}
