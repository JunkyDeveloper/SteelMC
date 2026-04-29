//! Steel server commands:
//! /steel tp <targets> <world>
//! /steel blacklist-ip add <ip>
//! /steel blacklist-ip remove <ip>
//! /steel blacklist-ip list
//! /steel whitelist-ip list
//! /steel convert banned-ip

use std::net::IpAddr;
use std::sync::Arc;

use crate::command::arguments::ip::IpArgument;
use crate::command::arguments::player::PlayerArgument;
use crate::command::arguments::world::WorldArgument;
use crate::command::commands::{CommandHandlerBuilder, CommandHandlerDyn, argument, literal};
use crate::command::context::CommandContext;
use crate::command::error::CommandError;
use crate::entity::SharedEntity;
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
use crate::player::Player;
use crate::player::connection::NetworkConnection;
use crate::portal::WorldChangeRequest;
use crate::world::World;
use text_components::format::Color;
use text_components::{Modifier, TextComponent};

/// Handler for the "steel" command group.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["steel"],
        "Steel server commands.",
        "minecraft:command.steel",
    )
    .then(
        literal("tp").then(argument("targets", PlayerArgument::multiple()).then(
            argument("world", WorldArgument).executes(
                |(((), targets), world): (((), Vec<Arc<Player>>), Arc<World>),
                 context: &mut CommandContext|
                 -> Result<(), CommandError> {
                    let dim_name = &world.key;
                    let count = targets.len();

                    for target in &targets {
                        if target.is_domain_switching() {
                            return Err(CommandError::CommandFailed(Box::new(
                                TextComponent::plain(format!(
                                    "{} is already switching domains",
                                    target.gameprofile.name
                                )),
                            )));
                        }
                    }

                    for target in &targets {
                        let current_world = target.get_world();
                        if current_world.domain() == world.domain() {
                            context.server.queue_world_change(
                                target.clone() as SharedEntity,
                                WorldChangeRequest::WorldSpawn {
                                    target_world: world.clone(),
                                },
                            );
                        } else {
                            context
                                .server
                                .queue_domain_switch_to_world(target.clone(), world.clone())
                                .map_err(|error| {
                                    CommandError::CommandFailed(Box::new(TextComponent::plain(
                                        error,
                                    )))
                                })?;
                        }
                    }

                    let msg = if count == 1 {
                        format!(
                            "Teleporting {} to {}",
                            targets[0].gameprofile.name, dim_name
                        )
                    } else {
                        format!("Teleporting {count} players to {dim_name}")
                    };
                    context.sender.send_message(&TextComponent::from(msg));

                    Ok(())
                },
            ),
        )),
    )
    .then(
        literal("blacklist-ip")
            .then(literal("add").then(argument("ip", IpArgument).executes(
                |((), ip): ((), Option<IpAddr>),
                 context: &mut CommandContext|
                 -> Result<(), CommandError> {
                    // Check if the IP is valid
                    let valid_ip = ip.ok_or_else(|| {
                        CommandError::CommandFailed(Box::new(
                            TextComponent::plain("This IP is invalid.").color(Color::Red),
                        ))
                    })?;

                    // Blacklist the IP
                    if !IP_ACCESS_POLICY.blacklist_ip(&valid_ip) {
                        context.sender.send_message(
                            &TextComponent::plain("This IP is already blacklisted.")
                                .color(Color::Red),
                        );
                    }

                    // Disconnect the player
                    for player in context.server.get_players() {
                        if player.connection.ip_address() == valid_ip {
                            // Disconnect the player with the right message
                            player
                                .connection
                                .disconnect_with_reason(TextComponent::plain("Disconnected"));
                        }
                    }

                    // Send a message to the sender
                    context.sender.send_message(&TextComponent::plain(format!(
                        "The IP {valid_ip} is now Blacklisted"
                    )));
                    Ok(())
                },
            )))
            .then(literal("remove").then(
                argument("ip", IpArgument).executes(
                    |((), ip): ((), Option<IpAddr>),
                     context: &mut CommandContext|
                     -> Result<(), CommandError> {
                        //Check if the IP is valid
                        let valid_ip = ip.ok_or_else(|| {
                            CommandError::CommandFailed(Box::new(
                                TextComponent::plain("This IP is invalid.").color(Color::Red),
                            ))
                        })?;

                        //  Check if the IP is blacklisted
                        if !IP_ACCESS_POLICY.is_blacklisted(&valid_ip) {
                            context.sender.send_message(
                                &TextComponent::plain("This IP is not blacklisted.")
                                    .color(Color::Red),
                            );
                        }

                        // Unblacklist
                        IP_ACCESS_POLICY.un_blacklist_ip(&valid_ip);

                        // inform the sender
                        context.sender.send_message(&TextComponent::plain(format!(
                            "The IP {valid_ip} isn't blacklisted anymore Blacklisted",
                        )));
                        Ok(())
                    },
                ), /*
                   .then(literal("list"))
                   .then(literal("refresh"))
                   .then(literal("save")*/
            )),
    )
}
