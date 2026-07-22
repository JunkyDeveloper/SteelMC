//! Redstone torch behaviors (standing and wall variants).
//!
//! These mirror the placement/survival rules of regular torches but add a `LIT`
//! property and are intended to be expanded with redstone logic later.

use steel_macros::block_behavior;
use steel_registry::blocks::BlockRef;
use steel_registry::blocks::block_state_ext::BlockStateExt;
use steel_registry::blocks::properties::{BlockStateProperties, Direction};
use steel_utils::{BlockPos, BlockStateId};

use crate::behavior::block::BlockBehavior;
use crate::behavior::blocks::decoration::torch_block::{
    standing_torch_can_survive, standing_torch_update_shape, wall_torch_can_survive,
    wall_torch_update_shape,
};
use crate::behavior::context::BlockPlaceContext;
use crate::world::{LevelReader, ScheduledTickAccess};

/// Standing redstone torch (`redstone_torch`).
///
/// TODO: Redstone functionality (signal output, neighbor notifications,
/// scheduled ticks, burnout, particle effects).
#[block_behavior]
pub struct RedstoneTorchBlock {
    block: BlockRef,
}

impl RedstoneTorchBlock {
    #[must_use]
    /// Creates a new standing redstone torch behavior for the given block ref.
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }
}

impl BlockBehavior for RedstoneTorchBlock {
    /// Checks if a redstone torch can survive at the given position.
    /// Requires the block below to provide center support on its top face.
    fn can_survive(&self, _state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> bool {
        standing_torch_can_survive(world, pos)
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        world: &dyn ScheduledTickAccess,
        pos: BlockPos,
        direction: Direction,
        _neighbor_pos: BlockPos,
        _neighbor_state: BlockStateId,
    ) -> BlockStateId {
        standing_torch_update_shape(state, world, pos, direction)
    }

    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        let default_state = self.block.default_state();
        if !self.can_survive(default_state, context.world, context.place_pos()) {
            return None;
        }
        Some(default_state.set_value(&BlockStateProperties::LIT, true))
    }

    // TODO: implement redstone signal source behavior, neighbor updates, and burnout.
}

/// Wall redstone torch (`redstone_wall_torch`).
///
/// TODO: Redstone functionality (signal output by facing, neighbor notifications,
/// scheduled ticks, burnout, particle effects).
#[block_behavior]
pub struct RedstoneWallTorchBlock {
    block: BlockRef,
}

impl RedstoneWallTorchBlock {
    #[must_use]
    /// Creates a new wall redstone torch behavior for the given block ref.
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }
}

impl BlockBehavior for RedstoneWallTorchBlock {
    /// Checks if a wall redstone torch can survive at the given position.
    /// Requires the block behind (opposite of facing) to provide a sturdy face.
    fn can_survive(&self, state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> bool {
        wall_torch_can_survive(state, world, pos)
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        world: &dyn ScheduledTickAccess,
        pos: BlockPos,
        direction: Direction,
        _neighbor_pos: BlockPos,
        _neighbor_state: BlockStateId,
    ) -> BlockStateId {
        wall_torch_update_shape(state, world, pos, direction)
    }

    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        let clicked_face = context.clicked_face();
        if clicked_face.is_horizontal() {
            let facing = clicked_face;
            let state = self
                .block
                .default_state()
                .set_value(&BlockStateProperties::HORIZONTAL_FACING, facing)
                .set_value(&BlockStateProperties::LIT, true);
            if self.can_survive(state, context.world, context.place_pos()) {
                return Some(state);
            }
        }

        for &facing in &[
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ] {
            let state = self
                .block
                .default_state()
                .set_value(&BlockStateProperties::HORIZONTAL_FACING, facing)
                .set_value(&BlockStateProperties::LIT, true);
            if self.can_survive(state, context.world, context.place_pos()) {
                return Some(state);
            }
        }

        None
    }

    // TODO: implement redstone signal source behavior, neighbor updates, and burnout.
}
