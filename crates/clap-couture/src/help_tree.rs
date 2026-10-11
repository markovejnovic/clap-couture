//! The categories and messages of each command's help, from the root down.

use clap::Command;

use crate::{CommandCategoryMap, Messages, render::HelpBody};

/// What hangs off one command level of a [`HelpTree`].
#[non_exhaustive]
pub enum HelpBranch {
    /// A tree merged into this level: a flattened type, or the subcommand enum.
    Flattened(&'static HelpTree),
    /// The subcommand with this name.
    Subcommand(&'static str, &'static HelpTree),
}

/// One command's categories and messages, and the trees of the commands below it.
pub struct HelpTree {
    /// Merged trees and subcommands.
    pub branches: &'static [HelpBranch],
    /// The categories its subcommands are filed under.
    pub categories: CommandCategoryMap,
    /// The messages its type sets, for its help and the help below it; `None` to keep its
    /// parent's.
    pub messages: Option<Messages>,
}

impl HelpTree {
    /// A command with no categories, no messages of its own, and nothing below it.
    pub const EMPTY: Self =
        Self { branches: &[], categories: CommandCategoryMap::EMPTY, messages: None };

    /// `cmd` showing couture's help filed by `categories` and worded by `messages`, and each
    /// subcommand below it showing its own, from this tree.
    ///
    /// `built` is `cmd` built by clap, so each help lists what clap gives a subcommand: global
    /// args, styles and settings from its parents.
    pub(crate) fn install(
        &self,
        cmd: Command,
        built: &Command,
        categories: CommandCategoryMap,
        messages: &Messages,
    ) -> Command {
        let cmd = cmd.mut_subcommands(|subcommand| {
            let Some(built_subcommand) = built.find_subcommand(subcommand.get_name()) else {
                return subcommand;
            };
            let (tree, messages) = self
                .subcommand(subcommand.get_name(), *messages)
                .unwrap_or((&Self::EMPTY, *messages));
            tree.install(subcommand, built_subcommand, tree.categories, &messages)
        });
        HelpBody::new(built, categories, messages).install(cmd, messages)
    }

    /// The tree of the subcommand `name`, and the messages it inherits from `inherited` down
    /// through the trees merged on the way; `None` when no branch names it.
    fn subcommand(&self, name: &str, inherited: Messages) -> Option<(&Self, Messages)> {
        self.branches.iter().find_map(|branch| match branch {
            HelpBranch::Flattened(tree) => {
                tree.subcommand(name, tree.messages.unwrap_or(inherited))
            }
            HelpBranch::Subcommand(subcommand_name, tree) => {
                (*subcommand_name == name).then(|| (*tree, tree.messages.unwrap_or(inherited)))
            }
        })
    }
}
