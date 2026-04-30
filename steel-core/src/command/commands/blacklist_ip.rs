//! Handler for the "blacklist-ip" command.

use crate::command::arguments::ip::IpArgument;
use crate::command::commands::{CommandHandlerBuilder, CommandHandlerDyn, argument, literal};
use crate::command::context::CommandContext;
use crate::command::error::CommandError;
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
use crate::player::connection::NetworkConnection;
use std::net::IpAddr;
use text_components::format::Color;
use text_components::{Modifier, TextComponent};

/// Handler for the "blacklist-ip" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["blacklist-ip"],
        "Add, remove and see blacklisted IPs on the server",
        "minecraft:command.blacklistip",
    )
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
                    &TextComponent::plain("This IP is already blacklisted.").color(Color::Red),
                );

                return Ok(());
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
    .then(literal("remove").then(argument("ip", IpArgument).executes(
        |((), ip): ((), Option<IpAddr>),
         context: &mut CommandContext|
         -> Result<(), CommandError> {
            //  Check if the IP is blacklisted
            if !IP_ACCESS_POLICY.is_blacklisted(&ip.expect("Invalid IP")) {
                context.sender.send_message(
                    &TextComponent::plain("This IP is not blacklisted.").color(Color::Red),
                );
                return Ok(());
            }

            // Unblacklist
            IP_ACCESS_POLICY.un_blacklist_ip(&ip.expect("Invalid IP"));

            // inform the sender
            context.sender.send_message(&TextComponent::plain(format!(
                "Removed {} from the blacklist",
                ip.expect("Invalid IP")
            )));
            Ok(())
        },
    )))
    .then(literal("list").executes(
        |(): (), context: &mut CommandContext| -> Result<(), CommandError> {
            // Send the IPs, but sort it before
            let mut ips = IP_ACCESS_POLICY.get_blacklisted_ips().to_vec();
            ips.sort();

            context.sender.send_message(&TextComponent::plain(format!(
                "Blacklisted IP: {}",
                ips.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )));

            Ok(())
        },
    ))
}
