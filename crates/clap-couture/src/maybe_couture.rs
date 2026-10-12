//! Read a type's `Couture` consts whether or not it implements `Couture`.

use core::marker::PhantomData;

#[cfg(feature = "interactive")]
use crate::interactive::PromptedArgTree;
use crate::{CommandCategoryMap, Couture, help_tree::HelpTree};

/// `T`, which may or may not implement `Couture`. Its consts are `T`'s, or empty for a `T`
/// without the derive, so a flattened `Args` type from another crate needs no impl.
///
/// The inherent consts apply when `T: Couture`; otherwise [`NotCouture`]'s do.
#[derive(Debug)]
pub struct MaybeCouture<T>(PhantomData<T>);

/// The consts of a [`MaybeCouture`] whose `T` does not implement `Couture`.
pub trait NotCouture {
    /// No categories.
    const CATEGORIES: CommandCategoryMap = CommandCategoryMap::EMPTY;

    /// No categories or messages below.
    const HELP_TREE: &'static HelpTree = &HelpTree::EMPTY;

    /// No prompted args.
    #[cfg(feature = "interactive")]
    const PROMPTED_ARGS: &'static PromptedArgTree = &PromptedArgTree::EMPTY;
}

impl<T> MaybeCouture<T>
where
    T: Couture,
{
    /// `T`'s categories.
    pub const CATEGORIES: CommandCategoryMap = T::CATEGORIES;
    /// `T`'s help tree.
    pub const HELP_TREE: &'static HelpTree = &T::HELP_TREE;
    /// `T`'s prompted args.
    #[cfg(feature = "interactive")]
    pub const PROMPTED_ARGS: &'static PromptedArgTree = &T::PROMPTED_ARGS;
}

impl<T> NotCouture for MaybeCouture<T> {}
