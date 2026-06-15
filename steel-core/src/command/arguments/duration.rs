//! A duration argument.
//! Same as time argument but for irl days.
use steel_protocol::packets::game::{
    ArgumentStringTypeBehavior, ArgumentType, SuggestionEntry, SuggestionType,
};

use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;

/// A duration argument.
/// the duration time will be in:
/// - hour
/// - day
/// - week
/// - month
/// - years
pub struct DurationArgument;

impl CommandArgument for DurationArgument {
    type Output = i64;

    fn parse<'a>(
        &self,
        arg: &'a [&'a str],
        _context: &mut CommandContext,
    ) -> Option<(&'a [&'a str], Self::Output)> {
        let s = arg.first()?;

        let (number, unit) = s
            .find(|c: char| c.is_alphabetic())
            .map_or((*s, "d"), |pos| (&s[..pos], &s[pos..]));

        let number: i64 = number.parse().ok()?;
        // -1 is forever
        if number < 0 {
            return None;
        }

        let days = match unit {
            // "min" => number,
            "h" => number * 60,
            "d" => number * 60 * 24,
            "w" => number * 60 * 24 * 7,
            "m" => number * 60 * 24 * 30,
            "y" => number * 60 * 24 * 365,
            _ => return None,
        };

        Some((&arg[1..], days))
    }

    fn usage(&self) -> (ArgumentType, Option<SuggestionType>) {
        (
            ArgumentType::String {
                behavior: ArgumentStringTypeBehavior::SingleWord,
            },
            Some(SuggestionType::AskServer),
        )
    }

    fn suggest(
        &self,
        prefix: &str,
        _suggestion_ctx: &super::SuggestionContext,
    ) -> Vec<SuggestionEntry> {
        // We want to suggest after a number is entered
        if prefix.is_empty() {
            return vec![];
        }

        // Check if prefix already has a unit suffix
        if prefix.parse::<i64>().is_ok() {
            return vec![
                // If someone want to use minute one day...
                // need to change thing cause min is not a char
                // SuggestionEntry::new(format!("{prefix}min")),
                SuggestionEntry::new(format!("{prefix}h")),
                SuggestionEntry::new(format!("{prefix}d")),
                SuggestionEntry::new(format!("{prefix}w")),
                SuggestionEntry::new(format!("{prefix}m")),
                SuggestionEntry::new(format!("{prefix}y")),
            ];
        }
        vec![]
    }
}
