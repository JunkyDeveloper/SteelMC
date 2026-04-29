//! Handler for the "pardon-ip" command.

use std::net::IpAddr;

use crate::command::arguments::ip::IpArgument;
use crate::command::{
    commands::{CommandHandlerBuilder, CommandHandlerDyn, argument},
    context::CommandContext,
    error::CommandError,
};
use crate::network::ip_access_policy::IP_ACCESS_POLICY;
use steel_utils::translations::{
    COMMANDS_PARDONIP_FAILED, COMMANDS_PARDONIP_INVALID, COMMANDS_PARDONIP_SUCCESS,
};
use text_components::TextComponent;

/// Handler for the "pardon-ip" command.
#[must_use]
pub fn command_handler() -> impl CommandHandlerDyn {
    CommandHandlerBuilder::new(
        &["pardon-ip"],
        "Unban an IP from the server",
        "minecraft:command.pardonip",
    )
    .then(
        argument("ip", IpArgument).executes(
            |((), ip): ((), Option<IpAddr>), ctx: &mut CommandContext| pardon_ip(ctx, ip),
        ),
    )
}

fn pardon_ip(ctx: &mut CommandContext, ip: Option<IpAddr>) -> Result<(), CommandError> {
    // Wrong IP
    let valid_ip = ip.ok_or_else(|| {
        CommandError::CommandFailed(Box::new(COMMANDS_PARDONIP_INVALID.msg().component()))
    })?;

    //Check if ip is banned or not
    if !IP_ACCESS_POLICY.is_banned(&valid_ip) {
        ctx.sender
            .send_message(&COMMANDS_PARDONIP_FAILED.msg().component());
        return Ok(());
    }

    // Unban
    IP_ACCESS_POLICY.un_ban_ip(&valid_ip);
    ctx.sender.send_message(
        &COMMANDS_PARDONIP_SUCCESS
            .message([TextComponent::plain(valid_ip.to_string())])
            .component(),
    );

    Ok(())
}
