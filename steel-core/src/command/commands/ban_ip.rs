//! Handler for the "ban-ip" command.

use std::net::IpAddr;
use std::sync::Arc;

use crate::command::arguments::duration::DurationArgument;
use crate::command::arguments::ip::IpArgument;
use crate::command::arguments::string::StringArgument;
use crate::command::sender::CommandSender;
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
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
    COMMANDS_BANIP_FAILED, COMMANDS_BANIP_INFO, COMMANDS_BANIP_INVALID, COMMANDS_BANIP_SUCCESS,
    MULTIPLAYER_DISCONNECT_BANNED_IP_REASON,
};
use text_components::TextComponent;

/// Handler for the "ban-ip" command.
#[must_use]
#[expect(
    clippy::type_complexity,
    reason = "The command builder API relies on nested closures and tuples, which naturally creates complex types."
)]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["ban-ip"],
        "Ban a player IP from the server",
        "minecraft:command.banip",
    )
    .then(
        // IP
        argument("ip", IpArgument)
            .executes(|((), ip): ((), Option<IpAddr>), ctx: &mut CommandContext| {
                ban_ip(ctx, ip, None, None)
            })
            // IP + duration
            .then(
                argument("duration", DurationArgument)
                    .executes(
                        |(((), targets), duration): (((), Option<IpAddr>), i64),
                         ctx: &mut CommandContext| {
                            ban_ip(ctx, targets, None, Some(duration))
                        },
                    ) // IP + duration + reason.
                    // Reason last because the StringArgument::greedy() consume to the end
                    .then(argument("reason", StringArgument::greedy()).executes(
                        |((((), targets), duration), reason): (
                            (((), Option<IpAddr>), i64),
                            String,
                        ),
                         ctx: &mut CommandContext| {
                            ban_ip(ctx, targets, Some(reason), Some(duration))
                        },
                    )),
            )
            // IP + reason
            .then(argument("reason", StringArgument::greedy()).executes(
                |(((), ip), reason): (((), Option<IpAddr>), String), ctx: &mut CommandContext| {
                    ban_ip(ctx, ip, Some(reason), None)
                },
            )),
    )
    .then(
        //Players
        argument("targets", PlayerArgument::one())
            .executes(
                |((), targets): ((), Vec<Arc<Player>>), ctx: &mut CommandContext| {
                    ban_ip_players(&mut ctx.sender, targets, None, None)
                },
            )
            // Player + duration
            .then(
                argument("duration", DurationArgument)
                    .executes(
                        |(((), targets), duration): (((), Vec<Arc<Player>>), i64),
                         ctx: &mut CommandContext| {
                            ban_ip_players(&mut ctx.sender, targets, None, Some(duration))
                        },
                    )
                    // Player + duration + reason
                    .then(argument("reason", StringArgument::greedy()).executes(
                        |((((), targets), duration), reason): (
                            (((), Vec<Arc<Player>>), i64),
                            String,
                        ),
                         ctx: &mut CommandContext| {
                            ban_ip_players(&mut ctx.sender, targets, Some(reason), Some(duration))
                        },
                    )),
            )
            // Player + reason
            .then(argument("reason", StringArgument::greedy()).executes(
                |(((), targets), reason): (((), Vec<Arc<Player>>), String),
                 ctx: &mut CommandContext| {
                    ban_ip_players(&mut ctx.sender, targets, Some(reason), None)
                },
            )),
    )
}

/// Ban IP a list of player
#[expect(
    clippy::unnecessary_wraps,
    reason = "executes() callback API requires Fn(...) -> Result<(), CommandError>"
)]
fn ban_ip_players(
    sender: &mut CommandSender,
    players: Vec<Arc<Player>>,
    reason: Option<String>,
    duration_in_days: Option<i64>,
) -> Result<(), CommandError> {
    // Check if this is a player, or the console / command block or something else
    let final_sender = sender.get_player().map_or_else(
        || "Server".to_string(),
        |sender| sender.gameprofile.name.clone(),
    );
    // The reason is a string because a TextComponentArgument can't work with the duration (it consume it)
    // So we need to check if the reason is empty or not and then have the real reason
    // Then, try to get it from the snbt format. If this is compatible, use it, else use TextComponent::plain
    let real_reason = reason.unwrap_or("Banned by an operator.".to_string());
    let text_component_reason = match TextComponent::from_snbt(real_reason.as_str()) {
        Ok(reason) => reason,
        Err(_) => TextComponent::plain(real_reason),
    };

    // Adapt the duration to the prototype of IP_ACCESS_POLICY.ban_ip
    let duration = duration_in_days.map(|d| chrono::Utc::now() + chrono::Duration::minutes(d));

    // Ban every player
    for player in &players {
        if !IP_ACCESS_POLICY.ban_ip(
            player.connection.ip_address(),
            final_sender.clone(),
            text_component_reason.clone(),
            duration,
        ) {
            // Send a fail message to the sender if the player is already banned
            sender.send_message(&COMMANDS_BANIP_FAILED.msg().component());
            continue;
        }
        player.connection.disconnect_with_reason(
            MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                .message([text_component_reason.clone()])
                .component(),
        );
    }

    let joined_names = players
        .iter()
        .map(|player| player.gameprofile.name.as_str())
        .collect::<Vec<&str>>()
        .join(", ");

    // Finally inform the sender how many player were ban and there pseudos
    sender.send_message(
        &COMMANDS_BANIP_INFO
            .message([
                TextComponent::plain(players.len().to_string()),
                TextComponent::plain(joined_names),
            ])
            .component(),
    );

    Ok(())
}

/// ban an IP
fn ban_ip(
    ctx: &mut CommandContext,
    ip: Option<IpAddr>,
    reason: Option<String>,
    duration_in_days: Option<i64>,
) -> Result<(), CommandError> {
    // Check if this is a player, or the console / command block or something else
    let final_sender = ctx.sender.get_player().map_or_else(
        || "Server".to_string(),
        |sender| sender.gameprofile.name.clone(),
    );
    // Wrong IP = custom Error
    let valid_ip = ip.ok_or_else(|| {
        CommandError::CommandFailed(Box::new(COMMANDS_BANIP_INVALID.msg().component()))
    })?;

    // The reason is a string because a TextComponentArgument can't work with the duration (it consume it)
    // So we need to check if the reason is empty or not and then have the real reason
    // Then, try to get it from the snbt format. If this is compatible, use it, else use TextComponent::plain
    let real_reason = reason.unwrap_or("Banned by an operator.".to_string());
    let text_component_reason = match TextComponent::from_snbt(real_reason.as_str()) {
        Ok(reason) => reason,
        Err(_) => TextComponent::plain(real_reason),
    };

    // Adapt the duration to the prototype of IP_ACCESS_POLICY.ban_ip
    let duration = duration_in_days.map(|d| chrono::Utc::now() + chrono::Duration::minutes(d));

    //BAN IP + verify if the ip is not already banned
    if !IP_ACCESS_POLICY.ban_ip(
        valid_ip,
        final_sender.clone(),
        text_component_reason.clone(),
        duration,
    ) {
        ctx.sender
            .send_message(&COMMANDS_BANIP_FAILED.msg().component());
        return Ok(());
    }

    // Disconnect every player with this IP
    let mut player_list: Vec<Arc<Player>> = Vec::new();
    for player in ctx.server.get_players() {
        if player.connection.ip_address() == valid_ip {
            // Disconnect the player with the right message
            player.connection.disconnect_with_reason(
                MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                    .message([text_component_reason.clone()])
                    .component(),
            );
            player_list.push(player);
        }
    }

    // Then inform the sender of the command what ip is banned and the reason why
    ctx.sender.send_message(
        &COMMANDS_BANIP_SUCCESS
            .message([
                TextComponent::plain(valid_ip.to_string()),
                text_component_reason.clone(),
            ])
            .component(),
    );

    if !player_list.is_empty() {
        // Finally inform the sender how many player were ban and there pseudos
        let joined_names = player_list
            .iter()
            .map(|player| player.gameprofile.name.as_str())
            .collect::<Vec<&str>>()
            .join(", ");

        ctx.sender.send_message(
            &COMMANDS_BANIP_INFO
                .message([
                    TextComponent::plain(player_list.len().to_string()),
                    TextComponent::plain(joined_names),
                ])
                .component(),
        );
    }

    Ok(())
}
