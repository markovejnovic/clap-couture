//! What `#[derive(Couture)]` expands to. Not API: any of it may change in any release.

#[cfg(feature = "interactive")]
pub use crate::interactive::prompted_args::{
    DashAttr, EditorAttr, PromptAttr, PromptPurpose, PromptedArg, PromptedArgBranch,
    PromptedArgTree, SecretAttr, SecretField, StdinAttr,
};
pub use crate::{
    help_tree::{HelpBranch, HelpTree},
    maybe_couture::{MaybeCouture, NotCouture},
};
