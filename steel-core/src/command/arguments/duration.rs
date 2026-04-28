//! A duration argument.
//! Same as time argument but for irl days.
use steel_protocol::packets::game::{ArgumentType, SuggestionEntry, SuggestionType};

use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;

/// A duration argument.
/// the duration time will be in:
/// - day
/// - week
/// - month
/// - years
pub struct DurationArgument;

impl CommandArgument for DurationArgument {
    type Output = i64;

    // TODO not the right letter after the number
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
            "d" => number,
            "w" => number * 7,
            "m" => number * 30,
            "y" => number * 365,
            "f" => -1,
            _ => return None,
        };

        Some((&arg[1..], days))
    }

    fn usage(&self) -> (ArgumentType, Option<SuggestionType>) {
        (ArgumentType::Time { min: 0 }, None)
    }

    /// ONLY FOR THE CONSOLE\
    /// (If you want to also suggest to the client,
    /// put the `SuggestionType` to `AskServer`)
    fn suggest(
        &self,
        prefix: &str,
        _suggestion_ctx: &super::SuggestionContext,
    ) -> Vec<SuggestionEntry> {
        // Check if prefix already has a unit suffix
        let has_unit = prefix.chars().any(char::is_alphabetic);
        if !prefix.is_empty() && !has_unit {
            return vec![
                SuggestionEntry::new(format!("{prefix}d")),
                SuggestionEntry::new(format!("{prefix}w")),
                SuggestionEntry::new(format!("{prefix}m")),
                SuggestionEntry::new(format!("{prefix}y")),
            ];
        }
        vec![]
    }
}
