//! The static tree of prompted and stdin args that `#[derive(Couture)]` emits as
//! `Couture::PROMPTED_ARGS`.

use secrecy::SecretString;

use super::sealed::Sealed;
use crate::Text;

/// The `editor` beside a `#[couture(prompt)]`: ask in the user's editor.
#[derive(Debug)]
pub struct EditorAttr {
    /// The temp file's extension, without the dot.
    pub extension: &'static str,
    /// The text the file starts with when the arg has no default.
    pub initial: Option<Text>,
}

impl EditorAttr {
    /// Evaluated beside every `#[couture(prompt, editor)]`: without the `interactive-editor`
    /// feature, the build fails here instead of the prompt quietly falling back to one line.
    /// Only reachable when `clap-couture-derive`'s feature is on and ours isn't.
    #[cfg(feature = "interactive-editor")]
    pub const FEATURE_CHECK: () = ();
    /// See the version with the feature on.
    #[cfg(not(feature = "interactive-editor"))]
    pub const FEATURE_CHECK: () =
        panic!("`#[couture(editor)]` needs clap-couture's `interactive-editor` feature");
}

/// One `#[couture(prompt)]` or `#[couture(confirm)]`, with or without `= "..."`.
#[derive(Debug)]
pub struct PromptAttr {
    /// The clap id of the arg it is on.
    pub arg_id: &'static str,
    /// `Some` for `#[couture(prompt, editor)]`; only set with the `interactive-editor` feature.
    pub editor: Option<EditorAttr>,
    /// What the answer is for.
    pub purpose: PromptPurpose,
    /// The `= ...` override; `None` asks the arg's help text.
    pub question: Option<Text>,
    /// The `secret` beside it, if any.
    pub secret: Option<SecretAttr>,
}

/// What a prompt's answer is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PromptPurpose {
    /// `#[couture(confirm)]`: a yes sets the flag, a no aborts the parse. Asked after every
    /// [`Value`](Self::Value).
    Confirmation,
    /// `#[couture(prompt)]`: the answer is the arg's value.
    Value,
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
    /// The clap id of this level's `#[couture(accept_stdin)]` arg, if any.
    pub stdin_arg: Option<&'static str>,
}

/// An arg marked `#[couture(accept_stdin)]`, and the subcommand path to the command that owns it.
#[derive(Debug)]
pub(crate) struct StdinArg {
    /// The clap id of the arg.
    pub(crate) arg_id: &'static str,
    /// Subcommand names from the root, empty for a root arg.
    pub(crate) subcommand_path: Vec<&'static str>,
}

/// A `secret` or `secret(allow_argv)` beside `#[couture(prompt)]`.
#[derive(Debug)]
pub struct SecretAttr {
    /// The value may come from the command line.
    allow_argv: bool,
}

/// A field type `#[couture(secret)]` can hold, peeled of its `Option`.
#[diagnostic::on_unimplemented(
    message = "`#[couture(secret)]` holds a `SecretString`, not `{Self}`",
    label = "not a `SecretString`"
)]
pub trait SecretField: Sealed {}

impl SecretField for SecretString {}

impl PromptedArgTree {
    /// A tree with no prompted args, the default for a hand-written `Couture` impl.
    pub const EMPTY: Self = Self { branches: &[], own_args: &[], stdin_arg: None };

    /// Evaluated beside every derived tree: fails the build when one command path holds more than
    /// one `#[couture(accept_stdin)]` arg, as stdin can only be read once.
    ///
    /// # Panics
    /// When it fails the build.
    pub const fn assert_stdin_read_once(&self) {
        assert!(
            self.most_stdin_args_on_a_path() <= 1,
            "`#[couture(accept_stdin)]` marks more than one arg on one command path; stdin can \
             only be read once"
        );
    }

    /// The most stdin args any one subcommand of this level, or of a tree merged into it, reaches.
    const fn most_stdin_args_below(&self) -> usize {
        let mut most = 0;
        let mut rest = self.branches;
        while let Some((branch, tail)) = rest.split_first() {
            let below = match branch {
                PromptedArgBranch::Flattened(tree) => tree.most_stdin_args_below(),
                PromptedArgBranch::Subcommand(_, tree) => tree.most_stdin_args_on_a_path(),
            };
            if below > most {
                most = below;
            }
            rest = tail;
        }
        most
    }

    /// The most stdin args one subcommand chain reaches: this level's, then the most below it.
    const fn most_stdin_args_on_a_path(&self) -> usize {
        self.stdin_args_at_this_level().saturating_add(self.most_stdin_args_below())
    }

    /// Every prompted arg in the tree: a level's own, then its branches', in declaration order.
    #[must_use]
    pub fn prompted_args(&'static self) -> Vec<PromptedArg> {
        let mut prompted_args = Vec::new();
        self.walk(&mut Vec::new(), &mut |tree, subcommand_path| {
            prompted_args.extend(
                tree.own_args
                    .iter()
                    .map(|attr| PromptedArg { attr, subcommand_path: subcommand_path.to_vec() }),
            );
        });
        prompted_args
    }

    /// Every `#[couture(accept_stdin)]` arg in the tree, in declaration order. At most one of them
    /// sits on any one subcommand chain.
    pub(crate) fn stdin_args(&'static self) -> Vec<StdinArg> {
        let mut stdin_args = Vec::new();
        self.walk(&mut Vec::new(), &mut |tree, subcommand_path| {
            stdin_args.extend(
                tree.stdin_arg
                    .map(|arg_id| StdinArg { arg_id, subcommand_path: subcommand_path.to_vec() }),
            );
        });
        stdin_args
    }

    /// The stdin args at this level: its own and those of the trees merged into it.
    const fn stdin_args_at_this_level(&self) -> usize {
        let mut count: usize = match self.stdin_arg {
            Some(_) => 1,
            None => 0,
        };
        let mut rest = self.branches;
        while let Some((branch, tail)) = rest.split_first() {
            if let PromptedArgBranch::Flattened(tree) = branch {
                count = count.saturating_add(tree.stdin_args_at_this_level());
            }
            rest = tail;
        }
        count
    }

    /// Call `visit` on this tree and every tree below it, with the subcommand path to each.
    fn walk(
        &'static self,
        subcommand_path: &mut Vec<&'static str>,
        visit: &mut impl FnMut(&'static Self, &[&'static str]),
    ) {
        visit(self, subcommand_path);
        for branch in self.branches {
            match branch {
                PromptedArgBranch::Flattened(tree) => tree.walk(subcommand_path, visit),
                PromptedArgBranch::Subcommand(subcommand_name, tree) => {
                    subcommand_path.push(subcommand_name);
                    tree.walk(subcommand_path, visit);
                    subcommand_path.pop();
                }
            }
        }
    }
}

impl SecretAttr {
    /// Whether the value may come from the command line: `secret(allow_argv)`.
    #[must_use]
    pub const fn allow_argv(&self) -> bool {
        self.allow_argv
    }

    /// The attr of a secret held in a `T`. The bound turns a field of the wrong type into a
    /// compile error at the field.
    #[must_use]
    pub const fn new<T>(allow_argv: bool) -> Self
    where
        T: SecretField,
    {
        Self { allow_argv }
    }
}
