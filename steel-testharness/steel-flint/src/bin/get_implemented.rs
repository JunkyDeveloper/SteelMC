//! Prints all blocks or items with a non-default behavior implemented in `SteelMC`.
use std::env;
use std::process::ExitCode;

use steel_flint::adapter::{
    get_implemented_blocks, get_implemented_entities, get_implemented_items,
};

fn main() -> ExitCode {
    let ids = match env::args().nth(1).as_deref() {
        Some("blocks") => {
            steel_flint::init();
            get_implemented_blocks()
        }
        Some("items") => {
            steel_flint::init();
            get_implemented_items()
        }
        Some("entities") => {
            steel_flint::init();
            get_implemented_entities()
        }
        _ => {
            eprintln!("usage: steel-implemented <blocks|items>");
            return ExitCode::FAILURE;
        }
    };
    for id in ids {
        println!("{id}");
    }
    ExitCode::SUCCESS
}
