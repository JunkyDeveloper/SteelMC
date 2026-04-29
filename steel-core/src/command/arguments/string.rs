//! A string argument.
use steel_protocol::packets::game::{
    ArgumentStringTypeBehavior, ArgumentType, SuggestionEntry, SuggestionType,
};

use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;

/// A string argument that can be a single word, a quotable phrase, or greedy.
pub struct StringArgument {
    behavior: ArgumentStringTypeBehavior,
}

impl StringArgument {
    /// Creates a string argument that reads a single word (no spaces).
    #[must_use]
    pub const fn word() -> Self {
        Self {
            behavior: ArgumentStringTypeBehavior::SingleWord,
        }
    }

    /// Creates a string argument that can contain spaces if wrapped in quotes ("like this").
    #[must_use]
    pub const fn quotable() -> Self {
        Self {
            behavior: ArgumentStringTypeBehavior::QuotablePhrase,
        }
    }

    /// Creates a string argument that reads the entire rest of the command.
    #[must_use]
    pub const fn greedy() -> Self {
        Self {
            behavior: ArgumentStringTypeBehavior::GreedyPhrase,
        }
    }
}

impl CommandArgument for StringArgument {
    type Output = String;

    fn parse<'a>(
        &self,
        arg: &'a [&'a str],
        _context: &mut CommandContext,
    ) -> Option<(&'a [&'a str], Self::Output)> {
        if arg.is_empty() {
            return None;
        }

        match self.behavior {
            ArgumentStringTypeBehavior::SingleWord => Some((&arg[1..], arg[0].to_string())),
            ArgumentStringTypeBehavior::GreedyPhrase => {
                let combined = arg.join(" ");
                Some((&[], combined))
            }
            ArgumentStringTypeBehavior::QuotablePhrase => {
                // TODO: implement logic
                Some((&arg[1..], arg[0].to_string()))
            }
        }
    }

    fn usage(&self) -> (ArgumentType, Option<SuggestionType>) {
        (
            ArgumentType::String {
                behavior: self.behavior,
            },
            None,
        )
    }

    fn suggest(
        &self,
        _prefix: &str,
        _suggestion_ctx: &super::SuggestionContext,
    ) -> Vec<SuggestionEntry> {
        vec![]
    }
}
