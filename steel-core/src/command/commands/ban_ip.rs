//! Handler for the "ban-ip" command.

use std::sync::Arc;

use crate::command::arguments::text_component::TextComponentArgument;
use crate::command::sender::CommandSender;
use crate::network::ban::IP_ACCESS_POLICY;
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
    COMMANDS_BANIP_FAILED, COMMANDS_KICK_SUCCESS, MULTIPLAYER_DISCONNECT_BANNED_IP_REASON,
};
use text_components::TextComponent;
use text_components::resolving::NoResolutor;

/// Handler for the "ban-ip" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["ban-ip"],
        "Ban a player IP from the server",
        "minecraft:command.banip",
    )
    .then(
        // TODO accept ip arguments
        argument("targets", PlayerArgument::multiple())
            .executes(
                |((), targets): ((), Vec<Arc<Player>>), ctx: &mut CommandContext| {
                    ban_ip_player(&mut ctx.sender, targets, None)
                },
            )
            .then(argument("reason", TextComponentArgument).executes(
                |(((), targets), reason): (((), Vec<Arc<Player>>), TextComponent),
                 ctx: &mut CommandContext| {
                    ban_ip_player(&mut ctx.sender, targets, Some(reason))
                },
            )),
    )
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "executes() callback API requires Fn(...) -> Result<(), CommandError>"
)]
fn ban_ip_player(
    sender: &mut CommandSender,
    players: Vec<Arc<Player>>,
    reason: Option<TextComponent>,
) -> Result<(), CommandError> {
    // no player return commands.banip.failed (vanilla)
    if players.is_empty() {
        sender.send_message(&COMMANDS_BANIP_FAILED.msg().into());
    }

    let real_reason = reason.unwrap_or(TextComponent::plain("Banned by an operator."));
    let final_sender = sender.get_player().map_or_else(
        || "Server".to_string(),
        |sender| sender.gameprofile.name.clone(),
    );

    for player in players {
        // First, disconnect the player
        player.connection.disconnect_with_reason(
            MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                .message([real_reason.clone()])
                .component(),
        );

        // And apply the ban
        IP_ACCESS_POLICY.ban_ip(
            player.connection.remote_address().ip(),
            final_sender.clone(),
            real_reason.to_plain(&NoResolutor),
            None,
        );

        // Then inform the sender of the command
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
