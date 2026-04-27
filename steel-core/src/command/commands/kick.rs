//! Handler for the "clear" command.
use std::sync::Arc;

use crate::command::arguments::text_component::TextComponentArgument;
use crate::command::sender::CommandSender;
use crate::player::connection::NetworkConnection;
use crate::{
    command::{
        arguments::player::PlayerArgument,
        commands::{CommandExecutor, CommandHandlerBuilder, CommandHandlerDyn, argument},
        context::CommandContext,
        error::CommandError,
    },
    player::Player,
};
use steel_utils::translations::{
    COMMANDS_KICK_OWNER_FAILED, COMMANDS_KICK_SUCCESS, MULTIPLAYER_DISCONNECT_KICKED,
};
use text_components::TextComponent;
use text_components::translation::TranslatedMessage;

/// Handler for the "clear" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["kick"],
        "Kick a player from the server",
        "minecraft:command.kick",
    )
    .then(
        argument("targets", PlayerArgument::multiple())
            .executes(KickExecutor)
            .then(argument("reason", TextComponentArgument).executes(KickWithReasonExecutor)),
    )
}

struct KickExecutor;

impl CommandExecutor<((), Vec<Arc<Player>>)> for KickExecutor {
    fn execute(
        &self,
        args: ((), Vec<Arc<Player>>),
        context: &mut CommandContext,
    ) -> Result<(), CommandError> {
        kick_player(&mut context.sender, args.1, None);

        Ok(())
    }
}

struct KickWithReasonExecutor;

impl CommandExecutor<(((), Vec<Arc<Player>>), TextComponent)> for KickWithReasonExecutor {
    fn execute(
        &self,
        args: (((), Vec<Arc<Player>>), TextComponent),
        context: &mut CommandContext,
    ) -> Result<(), CommandError> {
        kick_player(&mut context.sender, args.0.1, Some(args.1));

        Ok(())
    }
}

fn kick_player(
    sender: &mut CommandSender,
    players: Vec<Arc<Player>>,
    reason: Option<TextComponent>,
) {
    // no player return commands.kick.owner.failed (vanilla)
    if players.is_empty() {
        sender.send_message(&COMMANDS_KICK_OWNER_FAILED.msg().into())
    }

    let real_reason = reason.unwrap_or(MULTIPLAYER_DISCONNECT_KICKED.msg().component());

    for player in players {
        // First, disconnect the player
        player
            .connection
            .disconnect_with_reason(real_reason.clone()); //Sometimes, it's just "Disconnected" instead of the message?

        // Then inform the sender of the command
        sender.send_message(
            &COMMANDS_KICK_SUCCESS
                .message([
                    TextComponent::plain(player.gameprofile.name.clone()),
                    real_reason.clone(),
                ])
                .component(),
        )
    }
}
