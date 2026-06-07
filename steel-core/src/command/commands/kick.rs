//! Handler for the "kick" command.
use std::sync::Arc;

use crate::command::arguments::text_component::TextComponentArgument;
use crate::command::sender::CommandSender;
use crate::player::connection::NetworkConnection;
use crate::{
    command::{
        arguments::player::PlayerArgument,
        commands::{CommandHandlerBuilder, CommandHandlerDyn, argument},
        context::CommandContext,
        error::CommandError,
    },
    player::Player,
};
use steel_utils::translations::{
    COMMANDS_KICK_OWNER_FAILED, COMMANDS_KICK_SUCCESS, MULTIPLAYER_DISCONNECT_KICKED,
};
use text_components::TextComponent;

/// Handler for the "kick" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["kick"],
        "Kick a player from the server",
        "minecraft:command.kick",
    )
    .then(
        argument("targets", PlayerArgument::multiple())
            .executes(
                |((), targets): ((), Vec<Arc<Player>>), ctx: &mut CommandContext| {
                    kick_player(&mut ctx.sender, targets, None)
                },
            )
            .then(argument("reason", TextComponentArgument).executes(
                |(((), targets), reason): (((), Vec<Arc<Player>>), TextComponent),
                 ctx: &mut CommandContext| {
                    kick_player(&mut ctx.sender, targets, Some(reason))
                },
            )),
    )
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "executes() callback API requires Fn(...) -> Result<(), CommandError>"
)]
fn kick_player(
    sender: &mut CommandSender,
    players: Vec<Arc<Player>>,
    reason: Option<TextComponent>,
) -> Result<(), CommandError> {
    if players.is_empty() {
        sender.send_message(&COMMANDS_KICK_OWNER_FAILED.msg().into());
    }

    let real_reason = reason.unwrap_or(MULTIPLAYER_DISCONNECT_KICKED.msg().component());

    for player in players {
        player
            .connection
            .disconnect_with_reason(real_reason.clone());

        sender.send_message(
            &COMMANDS_KICK_SUCCESS
                .message([
                    TextComponent::plain(player.gameprofile.name.clone()),
                    real_reason.clone(),
                ])
                .component(),
        );
    }

    Ok(())
}
