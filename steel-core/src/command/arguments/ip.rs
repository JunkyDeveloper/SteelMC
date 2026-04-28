//! Un argument pour parser les adresses IP.
use crate::command::arguments::CommandArgument;
use crate::command::context::CommandContext;
use std::net::IpAddr;
use steel_protocol::packets::game::{ArgumentStringTypeBehavior, ArgumentType, SuggestionType};

/// Un argument d'adresse IP.
pub struct IpArgument;

impl CommandArgument for IpArgument {
    // On définit le type de sortie comme l'IpAddr standard de Rust
    type Output = Option<IpAddr>;

    fn parse<'a>(
        &self,
        arg: &'a [&'a str],
        _context: &mut CommandContext,
    ) -> Option<(&'a [&'a str], Self::Output)> {
        // On récupère le premier élément de la liste des arguments
        let s = arg.first()?;

        // On utilise le parsing natif de Rust pour les IP.
        // `s.parse()` va automatiquement essayer de lire une IPv4 ou IPv6.
        // Si c'est invalide, ça renvoie une erreur qu'on convertit en None avec `.ok()?`
        let ip: IpAddr = s.parse().ok()?;

        // Si le parsing a réussi, on renvoie le reste des arguments et l'IP parsée
        Some((&arg[1..], Some(ip)))
    }

    fn usage(&self) -> (ArgumentType, Option<SuggestionType>) {
        // Minecraft Vanilla n'a pas de vrai type d'argument "IP" dans son protocole
        // (Brigadier). On utilise généralement un argument de type "String" (un seul mot).
        // Attention : Adapte le `ArgumentType::String` selon les variantes exactes
        // disponibles dans l'enum de ton `steel_protocol`.
        (
            ArgumentType::String {
                behavior: ArgumentStringTypeBehavior::SingleWord,
            },
            None,
        )
    }

    // Pas besoin d'implémenter `suggest` ici, car suggérer des IP aux joueurs
    // n'est pas très pertinent (et potentiellement mauvais pour la confidentialité).
    // On garde donc l'implémentation par défaut du trait qui renvoie un Vec vide.
}
