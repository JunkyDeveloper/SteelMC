//! Handler for the "ban-ip" command.

use std::net::IpAddr;
use std::sync::Arc;

use crate::command::arguments::duration::DurationArgument;
use crate::command::arguments::ip::IpArgument;
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
    COMMANDS_BANIP_FAILED, COMMANDS_BANIP_INFO, COMMANDS_BANIP_INVALID, COMMANDS_BANIP_SUCCESS,
    MULTIPLAYER_DISCONNECT_BANNED_IP_REASON,
};
use text_components::TextComponent;

/// Handler for the "ban-ip" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["ban-ip"],
        "Ban a player IP from the server",
        "minecraft:command.banip",
    )
    .then(
        //Players
        argument("targets", PlayerArgument::multiple())
            .executes(
                |((), targets): ((), Vec<Arc<Player>>), ctx: &mut CommandContext| {
                    ban_ip_player(&mut ctx.sender, targets, None, None)
                },
            )
            // Player + duration
            .then(
                argument("duration", DurationArgument)
                    .executes(
                        |(((), targets), duration): (((), Vec<Arc<Player>>), i64),
                         ctx: &mut CommandContext| {
                            ban_ip_player(&mut ctx.sender, targets, None, Some(duration))
                        },
                    )
                    // Player + duration + reason
                    .then(argument("reason", TextComponentArgument).executes(
                        |((((), targets), duration), reason): (
                            (((), Vec<Arc<Player>>), i64),
                            TextComponent,
                        ),
                         ctx: &mut CommandContext| {
                            ban_ip_player(&mut ctx.sender, targets, Some(reason), Some(duration))
                        },
                    )),
            )
            // Player + reason
            .then(argument("reason", TextComponentArgument).executes(
                |(((), targets), reason): (((), Vec<Arc<Player>>), TextComponent),
                 ctx: &mut CommandContext| {
                    ban_ip_player(&mut ctx.sender, targets, Some(reason), None)
                },
            )),
    )
    .then(
        // IP
        argument("ip", IpArgument)
            .executes(|((), ip): ((), Option<IpAddr>), ctx: &mut CommandContext| {
                ban_ip_player_by_ip(ctx, ip, None, None)
            })
            // IP + reason
            .then(argument("reason", TextComponentArgument).executes(
                |(((), ip), reason): (((), Option<IpAddr>), TextComponent),
                 ctx: &mut CommandContext| {
                    ban_ip_player_by_ip(ctx, ip, Some(reason), None)
                },
            ))
            // IP + duration
            .then(
                argument("duration", DurationArgument)
                    .executes(
                        |(((), targets), duration): (((), Option<IpAddr>), i64),
                         ctx: &mut CommandContext| {
                            ban_ip_player_by_ip(ctx, targets, None, Some(duration))
                        },
                    ) // IP + duration + reason
                    // Reason last if later an API like MiniMessage is going to be used
                    .then(argument("reason", TextComponentArgument).executes(
                        |((((), targets), duration), reason): (
                            (((), Option<IpAddr>), i64),
                            TextComponent,
                        ),
                         ctx: &mut CommandContext| {
                            ban_ip_player_by_ip(ctx, targets, Some(reason), Some(duration))
                        },
                    )),
            ),
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
    duration_in_days: Option<i64>,
) -> Result<(), CommandError> {
    let final_sender = sender.get_player().map_or_else(
        || "Server".to_string(),
        |sender| sender.gameprofile.name.clone(),
    );
    let real_reason = reason.unwrap_or(TextComponent::plain("Banned by an operator."));
    let duration = duration_in_days.map(|d| chrono::Utc::now() + chrono::Duration::days(d));

    for player in &players {
        // And apply the ban
        if !IP_ACCESS_POLICY.ban_ip(
            player.connection.ip_address(),
            final_sender.clone(),
            real_reason.clone(),
            duration,
        ) {
            sender.send_message(&COMMANDS_BANIP_FAILED.msg().component());
            continue;
        }
        player.connection.disconnect_with_reason(
            MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                .message([real_reason.clone()])
                .component(),
        );
    }

    let joined_names = players
        .iter() // On parcourt la liste des joueurs
        .map(|player| player.gameprofile.name.as_str()) // On extrait juste le pseudo (en &str)
        .collect::<Vec<&str>>() // On rassemble ces pseudos dans un nouveau vecteur
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

fn ban_ip_player_by_ip(
    ctx: &mut CommandContext,
    ip: Option<IpAddr>,
    reason: Option<TextComponent>,
    duration_in_days: Option<i64>,
) -> Result<(), CommandError> {

    // Wrong IP = custom Error
    let final_sender = ctx.sender.get_player().map_or_else(
        || "Server".to_string(),
        |sender| sender.gameprofile.name.clone(),
    );
    let valid_ip = ip.ok_or_else(|| {
        CommandError::CommandFailed(Box::new(COMMANDS_BANIP_INVALID.msg().component()))
    })?;
    let real_reason = reason.unwrap_or(TextComponent::plain("Banned by an operator."));
    let duration = duration_in_days.map(|d| chrono::Utc::now() + chrono::Duration::days(d));

    //BAN IP + verify if the ip is not already banned
    if !IP_ACCESS_POLICY.ban_ip(
        valid_ip,
        final_sender.clone(),
        real_reason.clone(),
        duration,
    ) {
        ctx.sender
            .send_message(&COMMANDS_BANIP_FAILED.msg().component());
        return Ok(());
    }
    let mut player_list: Vec<Arc<Player>> = Vec::new();
    for player in ctx.server.get_players() {
        if player.connection.ip_address() == valid_ip {
            // Disconnect the player with the right message
            player.connection.disconnect_with_reason(
                MULTIPLAYER_DISCONNECT_BANNED_IP_REASON
                    .message([real_reason.clone()])
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
                real_reason.clone(),
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
