//! `PotentSulfurBlock` behavior

use steel_macros::block_behavior;
use steel_registry::blocks::BlockRef;
use steel_registry::blocks::block_state_ext::BlockStateExt as _;
use steel_registry::blocks::properties::{BlockStateProperties, PotentSulfurState};
use steel_registry::vanilla_block_tags::BlockTag;
use steel_utils::{BlockPos, BlockStateId, Direction};

use crate::behavior::{BlockBehavior, BlockPlaceContext, BlockStateBehaviorExt as _};
use crate::fluid::FluidStateExt as _;
use crate::world::{LevelReader, ScheduledTickAccess};

/// Vanilla `PotentSulfurBlock` behavior
///
/// TODO: Implement block-entity-driven eruption ticking once block entity
/// support is in place
#[block_behavior]
pub struct PotentSulfurBlock {
    block: BlockRef,
}

impl PotentSulfurBlock {
    /// New potent sulfur block behavior
    #[must_use]
    pub const fn new(block: BlockRef) -> Self {
        Self { block }
    }

    fn valid_state(state: BlockStateId, world: &dyn LevelReader, pos: BlockPos) -> BlockStateId {
        let above_fluid = world.get_block_state(pos.above()).get_fluid_state();
        if !above_fluid.is_source() || !above_fluid.is_water() {
            return state.set_value(
                &BlockStateProperties::POTENT_SULFUR_STATE,
                PotentSulfurState::Dry,
            );
        }

        let below = world.get_block_state(pos.below());
        let below_fluid = below.get_fluid_state();
        let fluid_ok = below_fluid.is_empty() || below_fluid.is_source();

        if below
            .get_block()
            .has_tag(&BlockTag::CAUSES_CONTINUOUS_GEYSER_ERUPTIONS)
            && fluid_ok
        {
            return state.set_value(
                &BlockStateProperties::POTENT_SULFUR_STATE,
                PotentSulfurState::Continuous,
            );
        }

        if below
            .get_block()
            .has_tag(&BlockTag::CAUSES_PERIODIC_GEYSER_ERUPTIONS)
            && fluid_ok
        {
            // Keep ERUPTING if already mideruption otherwise arm it and go brrt
            if state.get_value(&BlockStateProperties::POTENT_SULFUR_STATE)
                == PotentSulfurState::Erupting
            {
                return state;
            }
            return state.set_value(
                &BlockStateProperties::POTENT_SULFUR_STATE,
                PotentSulfurState::Dormant,
            );
        }

        state.set_value(
            &BlockStateProperties::POTENT_SULFUR_STATE,
            PotentSulfurState::Wet,
        )
    }
}

impl BlockBehavior for PotentSulfurBlock {
    fn get_state_for_placement(&self, context: &BlockPlaceContext<'_>) -> Option<BlockStateId> {
        Some(Self::valid_state(
            self.block.default_state(),
            context.world,
            context.relative_pos,
        ))
    }

    fn update_shape(
        &self,
        state: BlockStateId,
        world: &dyn ScheduledTickAccess,
        pos: BlockPos,
        _direction: Direction,
        _neighbor_pos: BlockPos,
        _neighbor_state: BlockStateId,
    ) -> BlockStateId {
        Self::valid_state(state, world, pos)
    }
}
