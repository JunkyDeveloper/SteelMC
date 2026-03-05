//! fire block behavior implementation.

use steel_registry::blocks::BlockRef;
use steel_registry::vanilla_blocks::{NETHER_PORTAL, OBSIDIAN};
use steel_utils::{BlockPos, BlockStateId};

use crate::behavior::block::BlockBehaviour;
use crate::behavior::context::BlockPlaceContext;
use crate::portal::portal_shape::{PortalFrameConfig, PortalShape};
use crate::world::World;

/// Behavior for fire blocks.
///
/// Fire burns, makes hot, and hurts
pub struct FireBlock {
    block: BlockRef,
}

impl FireBlock {
    /// Creates a new fire block behavior.
    #[must_use]
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }
}

impl BlockBehaviour for FireBlock {
    fn get_state_for_placement(&self, _context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        Some(self.block.default_state())
    }

    fn on_place(
        &self,
        _state: BlockStateId,
        world: &World,
        pos: BlockPos,
        _old_state: BlockStateId,
        _moved_by_piston: bool,
    ) {
        if let Some(tester) = PortalShape::find_portal_shape(
            world,
            pos,
            &PortalFrameConfig {
                min_width: 2,
                max_width: 21,
                min_height: 3,
                max_height: 21,
                frame: OBSIDIAN,
                portal: NETHER_PORTAL,
            },
        ) {
            tester.place_portal_blocks(world);
            // TODO: Play ignite sound, damage item
        }
    }
}
