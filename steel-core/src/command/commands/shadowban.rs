//! Handler for the "shadowban" command.

use crate::command::arguments::ip::IpArgument;
use crate::command::commands::{
    CommandExecutor, CommandHandlerBuilder, CommandHandlerDyn, argument, literal,
};
use crate::command::context::CommandContext;
use crate::command::error::CommandError;
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
use crate::player::connection::NetworkConnection;
use std::net::IpAddr;
use text_components::format::Color;
use text_components::{Modifier, TextComponent};

/// Handler for the "shadowban" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["shadowban"],
        "Add, remove and see shadowbanned IPs on the server",
        "minecraft:command.shadowban",
    )
    .then(literal("add").then(argument("ip", IpArgument).executes(BlacklistIpExecutor::Add)))
    .then(literal("remove").then(argument("ip", IpArgument).executes(BlacklistIpExecutor::Remove)))
    .then(literal("list").executes(BlacklistIpListExecutor))
}

/// An enum with the option "Add" and "Remove" from the shadowban command
enum BlacklistIpExecutor {
    Add,
    Remove,
}

impl CommandExecutor<((), Option<IpAddr>)> for BlacklistIpExecutor {
    fn execute(
        &self,
        args: ((), Option<IpAddr>),
        context: &mut CommandContext,
    ) -> Result<(), CommandError> {
        let ip = args.1.expect("Invalid IP");

        match self {
            BlacklistIpExecutor::Add => {
                // Blacklist the IP
                if !IP_ACCESS_POLICY.shadowban_ip(&ip) {
                    context.sender.send_message(
                        &TextComponent::plain("This IP is already shadowbaned.").color(Color::Red),
                    );
                    return Ok(());
                }

                // Send a message to the sender
                context.sender.send_message(&TextComponent::plain(format!(
                    "The IP {ip} is now shadowbaned"
                )));

                // Disconnect the players
                for player in context.server.get_players() {
                    if player.connection.ip_address() == ip {
                        player
                            .connection
                            .disconnect_with_reason(TextComponent::plain("Disconnected"));
                    }
                }

                Ok(())
            }
            BlacklistIpExecutor::Remove => {
                // Check if the IP is shadowbaned
                if !IP_ACCESS_POLICY.is_shadowbanned(&ip) {
                    context.sender.send_message(
                        &TextComponent::plain("This IP is not shadowbaned.").color(Color::Red),
                    );
                    return Ok(());
                }

                // Unshadowban
                IP_ACCESS_POLICY.un_shadowban_ip(&ip);

                // inform the sender
                context.sender.send_message(&TextComponent::plain(format!(
                    "Removed {ip} from the shadowban"
                )));
                Ok(())
            }
        }
    }
}

struct BlacklistIpListExecutor;

impl CommandExecutor<()> for BlacklistIpListExecutor {
    fn execute(&self, _args: (), context: &mut CommandContext) -> Result<(), CommandError> {
        // Send the IPs, but sort it before
        let mut ips = IP_ACCESS_POLICY.get_shadowbanned_ips();
        ips.sort();

        context.sender.send_message(&TextComponent::plain(format!(
            "Blacklisted IP: {}",
            ips.iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(", ")
        )));

        Ok(())
    }
}
