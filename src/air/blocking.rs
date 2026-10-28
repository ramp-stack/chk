use pelican_ui::Context;
pub use pelican_ui::components::{MessageContent, Sticker, Gif};

use prism::air::{Id, Name, Metadata, Contract, Instance};

use serde::{Deserialize, Serialize};

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

    fn apply(&mut self, message: Self::Message, _metadata: Metadata) -> Self::Result {
        match message {
            BlockAction::Block => self.is_blocked = true,
            BlockAction::Unblock => self.is_blocked = false,
        }
    }
}

impl Block {
    pub fn create(ctx: &mut Context, name: Name) -> Instance<Block> {
        ctx.air.instances::<Block>().list().into_iter().find(|(_, block)| {
            let block = block.pending();
            block.blocker == ctx.air.me() && block.blocked == name
        }).map(|(_, b)| b).unwrap_or_else(|| {
            let block = ctx.air.instances::<Block>().create(name);
            if name != ctx.air.me() {block.share(name);}
            block
        })
    }

    pub fn did_i_block(ctx: &mut Context, name: Name) -> bool {
        let me = ctx.air.me();
        ctx.air.instances::<Block>().list().iter().any(|(_, b)| {
            let block = b.pending();
            block.blocker == me && block.blocked == name && block.is_blocked
        })
    }

    pub fn am_i_blocked(ctx: &mut Context, name: Name) -> bool {
        let me = ctx.air.me();
        ctx.air.instances::<Block>().list().iter().any(|(_, b)| {
            let block = b.pending();
            block.blocker == name && block.blocked == me && block.is_blocked
        })
    }
}

#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum BlockAction {Block, Unblock}
