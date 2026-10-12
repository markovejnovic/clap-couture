//! The categories and messages of each command's help, from the root down.

use core::mem;
use std::ffi::OsString;

use clap::{ArgMatches, Command, error::ErrorKind};

use crate::{
    CommandCategoryMap, Couture, Messages,
    render::{CommandRenderExt as _, HelpBody},
};

/// Starts the help template [`CommandPath::marked`] gives each command, before its path.
const HELP_MARKER: &str = "couture-help-path:";

/// [`Command`], walked by the names of its subcommands.
trait CommandPath: Sized {
    /// The command at `path` below this one; `None` when a name on the way names none.
    fn at_path(&self, path: &[&str]) -> Option<&Self>;

    /// This command with the help of each command in its tree a marker of where it sits:
    /// `marker`, then the index of each subcommand on the way down.
    #[must_use]
    fn marked(self, marker: &str) -> Self;

    /// The names of the subcommands down to the one whose help clap prints for `argv`; `None`
    /// when it prints none.
    fn printed_help_path(&self, argv: &[OsString]) -> Option<Vec<String>>;

    /// This command with the one at `path` passed through `edit`, each level kept in order.
    #[must_use]
    fn with_edited_at<F>(self, path: &[&str], edit: F) -> Self
    where
        F: FnOnce(Self) -> Self;
}

/// A parser's couture help, rendered only for the command whose help clap prints.
#[derive(Clone, Copy)]
pub(crate) struct DeferredHelp {
    categories: CommandCategoryMap,
    /// The command as [`CommandExt::with_couture`](crate::CommandExt::with_couture) would get it.
    declared: fn() -> Command,
    messages: Messages,
    tree: &'static HelpTree,
}

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

    /// The categories and messages of the command at `path`, below this one with `categories`
    /// and `messages`.
    fn help_at(
        &self,
        path: &[&str],
        categories: CommandCategoryMap,
        messages: Messages,
    ) -> (CommandCategoryMap, Messages) {
        let Some((name, rest)) = path.split_first() else { return (categories, messages) };
        let (tree, messages) = self.subcommand(name, messages).unwrap_or((&Self::EMPTY, messages));
        tree.help_at(rest, tree.categories, messages)
    }

    /// `cmd` showing couture's help filed by `categories` and worded by `messages`, and each
    /// subcommand below it showing its own, from this tree.
    ///
    /// `built` is `cmd` built by clap, so each help lists what clap gives a subcommand: global
    /// args, styles and settings from its parents. Each menu wraps to `width`.
    pub(crate) fn install(
        &self,
        cmd: Command,
        built: &Command,
        categories: CommandCategoryMap,
        messages: &Messages,
        width: Option<usize>,
    ) -> Command {
        let cmd = cmd.mut_subcommands(|subcommand| {
            let Some(built_subcommand) = built.find_subcommand(subcommand.get_name()) else {
                return subcommand;
            };
            let (tree, messages) = self
                .subcommand(subcommand.get_name(), *messages)
                .unwrap_or((&Self::EMPTY, *messages));
            tree.install(subcommand, built_subcommand, tree.categories, &messages, width)
        });
        HelpBody::new(built, categories, messages, width).install(cmd)
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

impl CommandPath for Command {
    fn at_path(&self, path: &[&str]) -> Option<&Self> {
        path.iter().try_fold(self, |level, name| level.find_subcommand(name))
    }

    fn marked(self, marker: &str) -> Self {
        let mut index = 0usize;
        self.help_template(marker.to_owned()).mut_subcommands(|subcommand| {
            let below = subcommand.marked(&format!("{marker} {index}"));
            index = index.saturating_add(1);
            below
        })
    }

    fn printed_help_path(&self, argv: &[OsString]) -> Option<Vec<String>> {
        let printed = self.clone().marked(HELP_MARKER).try_get_matches_from(argv).err()?;
        let printed = printed.render().to_string();
        let mut level = self;
        printed
            .trim_end()
            .strip_prefix(HELP_MARKER)?
            .split_whitespace()
            .map(|index| {
                level = level.get_subcommands().nth(index.parse().ok()?)?;
                Some(level.get_name().to_owned())
            })
            .collect()
    }

    fn with_edited_at<F>(self, path: &[&str], edit: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        let Some((name, rest)) = path.split_first() else { return edit(self) };
        // Edited in place: `mut_subcommand` moves the one it edits last.
        let mut edit = Some(edit);
        self.mut_subcommands(|subcommand| {
            if subcommand.get_name() != *name {
                return subcommand;
            }
            match edit.take() {
                Some(edit) => subcommand.with_edited_at(rest, edit),
                None => subcommand,
            }
        })
    }
}

impl DeferredHelp {
    /// Parse `argv` against `cmd` as [`Command::try_get_matches_from_mut`] does, the help it
    /// prints being couture's.
    ///
    /// # Errors
    /// clap's error.
    pub(crate) fn matches(
        &self,
        cmd: &mut Command,
        argv: &[OsString],
    ) -> Result<ArgMatches, clap::Error> {
        let err = match cmd.try_get_matches_from_mut(argv) {
            Ok(matches) => return Ok(matches),
            Err(err) => err,
        };
        let prints_help = matches!(
            err.kind(),
            ErrorKind::DisplayHelp | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
        let Some(path) = prints_help.then(|| cmd.printed_help_path(argv)).flatten() else {
            return Err(err);
        };
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        let declared = (self.declared)();
        // Built as clap builds it to parse, so the help lists what clap propagates down.
        let mut built = declared.clone();
        built.build();
        // clap's own `help` subcommand is not declared, so its help stays clap's.
        let (Some(declared_at), Some(built_at)) = (declared.at_path(&path), built.at_path(&path))
        else {
            return Err(err);
        };
        let (categories, messages) = self.tree.help_at(&path, self.categories, self.messages);
        let body = HelpBody::new(built_at, categories, &messages, built.help_width());
        *cmd = mem::take(cmd).with_edited_at(&path, |printed| {
            // As `with_couture` renders every command's, for a template that lists them.
            let printed = printed.mut_subcommands(|subcommand| {
                if declared_at.find_subcommand(subcommand.get_name()).is_some() {
                    subcommand.with_rendered_about()
                } else {
                    subcommand
                }
            });
            body.install(printed)
        });
        Err(cmd.try_get_matches_from_mut(argv).err().unwrap_or(err))
    }

    /// `T`'s help, rendered from the command `declared` gives.
    pub(crate) fn of<T>(declared: fn() -> Command) -> Self
    where
        T: Couture,
    {
        Self { categories: T::CATEGORIES, declared, messages: T::MESSAGES, tree: &T::HELP_TREE }
    }
}
