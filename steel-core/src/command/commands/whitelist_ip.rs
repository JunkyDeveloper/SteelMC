//! Handler for the "whitelist-ip" command.

use crate::command::commands::{CommandHandlerBuilder, CommandHandlerDyn, literal};
use crate::command::context::CommandContext;
use crate::command::error::CommandError;
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
use text_components::TextComponent;

/// Handler for the "whitelist-ip" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["whitelist-ip"],
        "See what IPs are whitelisted on the server",
        "minecraft:command.whitelistip",
    )
    .then(literal("list").executes(
        |(): (), context: &mut CommandContext| -> Result<(), CommandError> {
            // Send the IPs, but sort it before
            let mut ips = IP_ACCESS_POLICY.get_whitelist_ips().to_vec();
            ips.sort();

            context.sender.send_message(&TextComponent::plain(format!(
                "Whitelisted IP: {}",
                ips.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )));

            Ok(())
        },
    ))
}
