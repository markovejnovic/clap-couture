//! The static tree of prompted args that `#[derive(Couture)]` emits as `Couture::PROMPTS`.

use core::marker::PhantomData;

use crate::Couture;

/// An arg to prompt for, and the subcommand path to the command that owns it.
#[derive(Debug)]
pub struct PromptedArg {
    /// Its `#[couture(prompt)]`.
    pub prompt: &'static PromptSpec,
    /// Subcommand names from the root, empty for a root arg.
    pub subcommand_path: Vec<&'static str>,
}

/// `T`'s `PROMPTS`, or an empty tree for a `T` that does not implement `Couture`, so a flattened
/// `Args` type from another crate needs no impl.
///
/// The inherent const applies when `T: Couture`; otherwise [`ProbeFallback`]'s does.
#[derive(Debug)]
pub struct Probe<T>(PhantomData<T>);

/// See [`Probe`].
pub trait ProbeFallback {
    /// No prompted args.
    const PROMPTS: &'static PromptNode = &PromptNode::EMPTY;
}

/// What hangs off one command level besides its own prompted args.
#[derive(Debug)]
#[non_exhaustive]
pub enum PromptChild {
    /// A node merged into this level: a flattened `Args` type, or the subcommand enum.
    Flatten(&'static PromptNode),
    /// The subcommand with this name.
    Subcommand(&'static str, &'static PromptNode),
}

/// The prompted args of one command level, and the tree below it.
#[derive(Debug)]
pub struct PromptNode {
    /// The args prompted for at this level.
    pub args: &'static [PromptSpec],
    /// Merged nodes and subcommands.
    pub children: &'static [PromptChild],
}

/// One `#[couture(prompt)]`.
#[derive(Debug)]
pub struct PromptSpec {
    /// The clap id of the prompted arg.
    pub id: &'static str,
    /// The `prompt = "..."` override; `None` asks the arg's help text.
    pub question: Option<&'static str>,
}

impl<T> Probe<T>
where
    T: Couture,
{
    /// `T`'s prompted args.
    pub const PROMPTS: &'static PromptNode = &T::PROMPTS;
}

impl<T> ProbeFallback for Probe<T> {}

impl PromptNode {
    /// A node with no prompted args, the default for a hand-written `Couture` impl.
    pub const EMPTY: Self = Self { args: &[], children: &[] };

    fn collect(
        &'static self,
        subcommand_path: &mut Vec<&'static str>,
        prompted_args: &mut Vec<PromptedArg>,
    ) {
        prompted_args.extend(
            self.args
                .iter()
                .map(|prompt| PromptedArg { subcommand_path: subcommand_path.clone(), prompt }),
        );
        for child in self.children {
            match child {
                PromptChild::Flatten(node) => node.collect(subcommand_path, prompted_args),
                PromptChild::Subcommand(subcommand_name, node) => {
                    subcommand_path.push(subcommand_name);
                    node.collect(subcommand_path, prompted_args);
                    subcommand_path.pop();
                }
            }
        }
    }

    /// Every prompted arg in the tree: a level's own, then its children's, in declaration order.
    #[must_use]
    pub fn prompted_args(&'static self) -> Vec<PromptedArg> {
        let mut prompted_args = Vec::new();
        self.collect(&mut Vec::new(), &mut prompted_args);
        prompted_args
    }
}
