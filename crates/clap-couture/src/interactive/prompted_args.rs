//! The static tree of prompted args that `#[derive(Couture)]` emits as `Couture::PROMPTED_ARGS`.

/// One `#[couture(prompt)]` or `#[couture(prompt = "...")]`.
#[derive(Debug)]
pub struct PromptAttr {
    /// The clap id of the arg it is on.
    pub arg_id: &'static str,
    /// The `prompt = "..."` override; `None` asks the arg's help text.
    pub question: Option<&'static str>,
}

/// An arg to prompt for, and the subcommand path to the command that owns it.
#[derive(Debug)]
pub struct PromptedArg {
    /// Its `#[couture(prompt)]`.
    pub attr: &'static PromptAttr,
    /// Subcommand names from the root, empty for a root arg.
    pub subcommand_path: Vec<&'static str>,
}

/// What hangs off one command level besides its own prompted args.
#[derive(Debug)]
#[non_exhaustive]
pub enum PromptedArgBranch {
    /// A tree merged into this level: a flattened `Args` type, or the subcommand enum.
    Flattened(&'static PromptedArgTree),
    /// The subcommand with this name.
    Subcommand(&'static str, &'static PromptedArgTree),
}

/// The prompted args of one command level, and the tree below it.
#[derive(Debug)]
pub struct PromptedArgTree {
    /// Merged trees and subcommands.
    pub branches: &'static [PromptedArgBranch],
    /// The args prompted for at this level.
    pub own_args: &'static [PromptAttr],
}

impl PromptedArgTree {
    /// A tree with no prompted args, the default for a hand-written `Couture` impl.
    pub const EMPTY: Self = Self { branches: &[], own_args: &[] };

    fn collect(
        &'static self,
        subcommand_path: &mut Vec<&'static str>,
        prompted_args: &mut Vec<PromptedArg>,
    ) {
        prompted_args.extend(
            self.own_args
                .iter()
                .map(|attr| PromptedArg { attr, subcommand_path: subcommand_path.clone() }),
        );
        for branch in self.branches {
            match branch {
                PromptedArgBranch::Flattened(tree) => tree.collect(subcommand_path, prompted_args),
                PromptedArgBranch::Subcommand(subcommand_name, tree) => {
                    subcommand_path.push(subcommand_name);
                    tree.collect(subcommand_path, prompted_args);
                    subcommand_path.pop();
                }
            }
        }
    }

    /// Every prompted arg in the tree: a level's own, then its branches', in declaration order.
    #[must_use]
    pub fn prompted_args(&'static self) -> Vec<PromptedArg> {
        let mut prompted_args = Vec::new();
        self.collect(&mut Vec::new(), &mut prompted_args);
        prompted_args
    }
}
