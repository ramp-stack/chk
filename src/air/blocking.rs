use pelican_ui::Context;
use pelican_ui::utils::Timestamp;
use pelican_ui::components::{MessageGroups};
pub use pelican_ui::components::{MessageContent, Sticker, Gif};

use maverick_os::air::{Id, Name, Metadata, Contract, Instance};

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::profiles::Profile;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Block {
    pub blocker: Name, // if this is me, then i blocked someone else
    pub blocked: Name, // if this is me, then i have been blocked
    pub is_blocked: bool, // so the user can be unblocked
}

impl Contract for Block {
    type Init = Name;
    type Message = BlockAction;
    type Result = ();

    fn id() -> Id {Id::hash("Block")}

    fn init(init: Self::Init, metadata: Metadata) -> Self {
        Block {
            blocker: metadata.signer,
            blocked: init,
            is_blocked: true,
        }
    }

    // Send message
    fn apply(&mut self, message: Self::Message, metadata: Metadata) -> Self::Result {
        match message {
            BlockAction::Block => {
                self.is_blocked = true;
            },
            BlockAction::Unblock => {
                self.is_blocked = false;
            },
        }
        
        ()
    }
}

impl Block {
    pub fn create(ctx: &mut Context, name: Name) -> Instance<Block> {
        // since 'I' run this code, the blocked will always be someone else
        ctx.list::<Block>().into_iter().find(|(_, block)| {
            // check if I have blocked this person before
            let block = block.pending();
            block.blocker == ctx.me() && block.blocked == name
        }).map(|(_, b)| b).unwrap_or_else(|| {
            let mut block = ctx.create::<Block>(name);
            block.share(name);
            block
        })
    }

    pub fn did_i_block(ctx: &mut Context, name: Name) -> bool {
        let me = ctx.me();
        ctx.list::<Block>().iter().any(|(_, b)| {
            let block = b.pending();
            block.blocker == me && block.blocked == name && block.is_blocked
        })
    }

    pub fn am_i_blocked(ctx: &mut Context, name: Name) -> bool {
        let me = ctx.me();
        ctx.list::<Block>().iter().any(|(_, b)| {
            let block = b.pending();
            block.blocker == name && block.blocked == me && block.is_blocked
        })
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum BlockAction {Block, Unblock}
