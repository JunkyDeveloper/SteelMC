//! An IP argument.
use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;
use std::net::IpAddr;
use steel_protocol::packets::game::{ArgumentStringTypeBehavior, ArgumentType, SuggestionType};

/// The struct for the IP argument.
pub struct IpArgument;

impl CommandArgument for IpArgument {
    type Output = Option<IpAddr>;

    fn parse<'a>(
        &self,
        arg: &'a [&'a str],
        _context: &mut CommandContext,
    ) -> Option<(&'a [&'a str], Self::Output)> {
        let s = arg.first()?;

        let ip: Option<IpAddr> = s.parse().ok();
        if ip.is_some() {
            return Some((&arg[1..], ip));
        }
        None
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
