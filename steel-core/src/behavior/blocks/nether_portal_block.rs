//! Nether portal block behavior.

use crate::behavior::block::BlockBehaviour;
use crate::behavior::context::BlockPlaceContext;
use crate::world::World;
use steel_registry::blocks::BlockRef;
use steel_registry::blocks::block_state_ext::BlockStateExt;
use steel_registry::blocks::properties::BlockStateProperties;
use steel_registry::vanilla_blocks::AIR;
use steel_utils::math::Axis;
use steel_utils::{BlockPos, BlockStateId, Direction};

/// Behavior for the nether portal block.
pub struct NetherPortalBlock {
    #[allow(dead_code)]
    block: BlockRef,
}
impl NetherPortalBlock {
    /// Create a new `NetherPortalBlock`
    #[must_use]
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }
}

impl BlockBehaviour for NetherPortalBlock {
    fn update_shape(
        &self,
        state: BlockStateId,
        _world: &World,
        _pos: BlockPos,
        direction: Direction,
        _neighbor_pos: BlockPos,
        neighbor_state: BlockStateId,
    ) -> BlockStateId {
        if neighbor_state.is_air()
        //&& (state.get_value(&BlockStateProperties::AXIS) == direction.axis()
        //    || direction.axis() == Axis::Y)
        //&& neighbor_state.0 != state.0
        {
            return AIR.default_state();
        }
        state
    }

    fn get_state_for_placement(&self, _context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        None // TODO: add this functionality but has low priority
    }
}
