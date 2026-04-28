//! Un argument pour parser les adresses IP.
use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;
use std::net::IpAddr;
use steel_protocol::packets::game::{ArgumentStringTypeBehavior, ArgumentType, SuggestionType};

/// Un argument d'adresse IP.
pub struct IpArgument;

impl CommandArgument for IpArgument {
    type Output = Option<IpAddr>;

    fn parse<'a>(
        &self,
        arg: &'a [&'a str],
        _context: &mut CommandContext,
    ) -> Option<(&'a [&'a str], Self::Output)> {
        let s = arg.first()?;

        let ip: IpAddr = s.parse().ok()?;

        Some((&arg[1..], Some(ip)))
    }

    fn usage(&self) -> (ArgumentType, Option<SuggestionType>) {
        (
            ArgumentType::String {
                behavior: ArgumentStringTypeBehavior::SingleWord,
            },
            None,
        )
    }
}
