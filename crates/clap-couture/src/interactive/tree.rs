//! The static tree of marked args that `#[derive(Couture)]` emits as `Couture::PROMPTS`.

/// A marked arg and the subcommand path to the command that owns it.
#[derive(Debug)]
pub struct Mark {
    /// Subcommand names from the root, empty for a root arg.
    pub path: Vec<&'static str>,
    /// The mark itself.
    pub spec: &'static PromptSpec,
}

/// What hangs off one command level besides its own marked args.
#[derive(Debug)]
#[non_exhaustive]
pub enum PromptChild {
    /// A node merged into this level: a flattened `Args` type, or the subcommand enum.
    Flatten(&'static PromptNode),
    /// The subcommand with this name.
    Subcommand(&'static str, &'static PromptNode),
}

/// The marked args of one command level, and the tree below it.
#[derive(Debug)]
pub struct PromptNode {
    /// The args marked at this level.
    pub args: &'static [PromptSpec],
    /// Merged nodes and subcommands.
    pub children: &'static [PromptChild],
}

/// One `#[couture(prompt)]`.
#[derive(Debug)]
pub struct PromptSpec {
    /// The clap id of the marked arg.
    pub id: &'static str,
    /// The `prompt = "..."` override; `None` asks the arg's help text.
    pub question: Option<&'static str>,
}

impl PromptNode {
    /// A node with no marks, the default for a hand-written `Couture` impl.
    pub const EMPTY: Self = Self { args: &[], children: &[] };

    fn collect(&'static self, path: &mut Vec<&'static str>, marks: &mut Vec<Mark>) {
        marks.extend(self.args.iter().map(|spec| Mark { path: path.clone(), spec }));
        for child in self.children {
            match child {
                PromptChild::Flatten(node) => node.collect(path, marks),
                PromptChild::Subcommand(name, node) => {
                    path.push(name);
                    node.collect(path, marks);
                    path.pop();
                }
            }
        }
    }

    /// Every mark in the tree: a level's own marks, then its children's, in declaration order.
    #[must_use]
    pub fn marks(&'static self) -> Vec<Mark> {
        let mut marks = Vec::new();
        self.collect(&mut Vec::new(), &mut marks);
        marks
    }
}
