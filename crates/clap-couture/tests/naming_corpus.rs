//! `#[derive(Couture)]` recomputes the name `clap_derive` gives each subcommand and the id it gives
//! each arg; on a mismatch a variant drops out of its category, its help loses couture's, and its
//! prompts are never asked. Macros stamp out every `rename_all` spelling on enums and structs,
//! crossed with every casing on variants, over idents `heck` splits differently, and every way an
//! attribute names a variant or an arg; clap is the reference for each.
#![cfg(feature = "interactive")]
#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "variants and fields are grouped by the naming rule they check"
)]

use clap::{Args, Command, CommandFactory as _, Parser, Subcommand};
use clap_couture::{
    __private::{HelpBranch, HelpTree},
    Couture,
};

/// An id clap evaluates, which couture can't read off the attribute.
const EVALUATED_ID: &str = "evaluated-id";
/// A name clap evaluates, which couture can't read off the attribute.
const EVALUATED_NAME: &str = "evaluated-name";

/// An enum whose variants clap names by `$variant_casing`, under `$enum_casing`, each casing
/// optional. Its variants cover the idents casings split differently, and every variant shape.
macro_rules! renamed_variants {
    ($module:ident, [$($enum_casing:tt)?], [$($variant_casing:tt)?]) => {
        mod $module {
            #[derive(clap::Subcommand, clap_couture::Couture)]
            $(#[command(rename_all = $enum_casing)])?
            #[couture(categories = { "all" = {} })]
            #[expect(clippy::upper_case_acronyms, reason = "how `heck` splits these is checked")]
            pub(crate) enum Cmd {
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                DryRun,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                HTTPServer,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                Foo2Bar,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                R2D2,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                r#Type,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                ABC,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                XmlHTTPRequest,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                A,
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                LeafArgs(crate::Leaf),
                $(#[command(rename_all = $variant_casing)])?
                #[couture(category = "all")]
                InlineFields {
                    #[arg(long)]
                    #[couture(prompt)]
                    depth_level: String,
                },
                $(#[command(rename_all = $variant_casing)])?
                #[command(subcommand)]
                #[couture(category = "all")]
                NestedEnum(crate::Nested),
            }
        }
    };
}

/// Under `$enum_casing`, one enum per variant casing, and one with none.
macro_rules! variant_casings {
    ($module:ident, [$($enum_casing:tt)?]) => {
        mod $module {
            renamed_variants!(unset, [$($enum_casing)?], []);
            renamed_variants!(camel, [$($enum_casing)?], ["camelCase"]);
            renamed_variants!(kebab, [$($enum_casing)?], ["kebab-case"]);
            renamed_variants!(lower, [$($enum_casing)?], ["lowercase"]);
            renamed_variants!(pascal, [$($enum_casing)?], ["PascalCase"]);
            renamed_variants!(screaming_snake, [$($enum_casing)?], ["SCREAMING_SNAKE_CASE"]);
            renamed_variants!(snake, [$($enum_casing)?], ["snake_case"]);
            renamed_variants!(upper, [$($enum_casing)?], ["UPPERCASE"]);
            renamed_variants!(verbatim, [$($enum_casing)?], ["verbatim"]);

            pub(crate) fn check() -> usize {
                [
                    crate::check_subcommands::<unset::Cmd>(module_path!()),
                    crate::check_subcommands::<camel::Cmd>(module_path!()),
                    crate::check_subcommands::<kebab::Cmd>(module_path!()),
                    crate::check_subcommands::<lower::Cmd>(module_path!()),
                    crate::check_subcommands::<pascal::Cmd>(module_path!()),
                    crate::check_subcommands::<screaming_snake::Cmd>(module_path!()),
                    crate::check_subcommands::<snake::Cmd>(module_path!()),
                    crate::check_subcommands::<upper::Cmd>(module_path!()),
                    crate::check_subcommands::<verbatim::Cmd>(module_path!()),
                ]
                .iter()
                .sum()
            }
        }
    };
}

/// A struct whose fields clap ids by their idents and attributes, under `$casing`, which renames
/// longs but never ids.
macro_rules! renamed_fields {
    ($module:ident, [$($casing:tt)?]) => {
        mod $module {
            #[derive(clap::Parser, clap_couture::Couture)]
            #[command(name = "fields")]
            $(#[command(rename_all = $casing)])?
            #[expect(non_snake_case, reason = "a field clap ids as written is checked")]
            pub(crate) struct Fields {
                #[arg(long)]
                #[couture(prompt)]
                dry_run: String,
                #[arg(long)]
                #[couture(prompt)]
                r#type: String,
                #[arg(short, long)]
                #[couture(prompt)]
                http_server2: String,
                #[arg(long = "out-dir", value_name = "DIR")]
                #[couture(prompt)]
                output: String,
                #[arg(long, id = "zone")]
                #[couture(prompt)]
                region: String,
                #[arg(long, name = "legacy")]
                #[couture(prompt)]
                old: String,
                #[arg(long, id = crate::EVALUATED_ID)]
                #[couture(prompt)]
                evaluated: String,
                #[arg(long, id = "first", id = "second-id")]
                #[couture(prompt)]
                twice: String,
                #[arg(long, id = "first")]
                #[arg(name = "second-name")]
                #[couture(prompt)]
                across_attrs: String,
                #[arg(long, name = "first", id = "id-last")]
                #[couture(prompt)]
                name_then_id: String,
                #[clap(long, id = "clap-id")]
                #[couture(prompt)]
                clap_attr_id: String,
                #[clap(long, name = "clap-name")]
                #[couture(prompt)]
                clap_attr_name: String,
                #[arg(long, value_name = "N", alias = "count-alias")]
                #[couture(prompt)]
                count: Option<u8>,
                #[arg(long)]
                #[couture(prompt)]
                XmlHTTPRequest: Option<String>,
                #[couture(prompt)]
                source: String,
            }

            pub(crate) fn check() -> usize {
                crate::check_args::<Fields>(module_path!())
            }
        }
    };
}

variant_casings!(enum_unset, []);
variant_casings!(enum_camel_case, ["camelCase"]);
variant_casings!(enum_camel, ["camel"]);
variant_casings!(enum_kebab_case, ["kebab-case"]);
variant_casings!(enum_kebab, ["kebab"]);
variant_casings!(enum_kebab_pascal_spelled, ["KebabCase"]);
variant_casings!(enum_pascal_case, ["PascalCase"]);
variant_casings!(enum_pascal, ["pascal"]);
variant_casings!(enum_screaming_snake_case, ["SCREAMING_SNAKE_CASE"]);
variant_casings!(enum_screaming_snake, ["screaming_snake"]);
variant_casings!(enum_snake_case, ["snake_case"]);
variant_casings!(enum_snake_pascal_spelled, ["SnakeCase"]);
variant_casings!(enum_snake, ["snake"]);
variant_casings!(enum_lowercase, ["lowercase"]);
variant_casings!(enum_lower, ["lower"]);
variant_casings!(enum_uppercase, ["UPPERCASE"]);
variant_casings!(enum_upper, ["upper"]);
variant_casings!(enum_verbatim, ["verbatim"]);
variant_casings!(enum_verbatim_case, ["VerbatimCase"]);

renamed_fields!(struct_unset, []);
renamed_fields!(struct_camel_case, ["camelCase"]);
renamed_fields!(struct_kebab_case, ["kebab-case"]);
renamed_fields!(struct_lowercase, ["lowercase"]);
renamed_fields!(struct_pascal_case, ["PascalCase"]);
renamed_fields!(struct_screaming_snake_case, ["SCREAMING_SNAKE_CASE"]);
renamed_fields!(struct_snake_case, ["snake_case"]);
renamed_fields!(struct_uppercase, ["UPPERCASE"]);
renamed_fields!(struct_verbatim, ["verbatim"]);

/// The args of a tuple variant, one of them prompted.
#[derive(Args, Couture)]
struct Leaf {
    #[arg(long)]
    #[couture(prompt)]
    leaf_arg: String,
}

/// A subcommand enum below another, with its own casing.
#[derive(Subcommand, Couture)]
#[command(rename_all = "SCREAMING_SNAKE_CASE")]
#[couture(categories = { "all" = {} })]
enum Nested {
    #[couture(category = "all")]
    DeepInline {
        #[arg(long)]
        #[couture(prompt)]
        deep_arg: String,
    },
    #[command(name = "deep-named")]
    #[couture(category = "all")]
    DeepNamed(Leaf),
}

/// Every way an attribute names a variant, and the variants clap makes no command of.
#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
#[couture(categories = { "all" = {} })]
enum NamedByAttrs {
    #[command(name = "literal")]
    #[couture(category = "all")]
    Literal,
    #[command(name = EVALUATED_NAME)]
    #[couture(category = "all")]
    Evaluated,
    #[command(name = "first")]
    #[command(name = "second")]
    #[couture(category = "all")]
    TwoAttrs,
    #[command(name = "first", name = "second-key")]
    #[couture(category = "all")]
    TwoKeys,
    #[command(name = "beats-casing", rename_all = "UPPERCASE")]
    #[couture(category = "all")]
    NameAndCasing,
    #[command(rename_all = "UPPERCASE", name = "beats-casing-after")]
    #[couture(category = "all")]
    CasingAndName,
    #[clap(name = "clap-attr")]
    #[couture(category = "all")]
    ClapAttr,
    #[command(name = "tuple-named")]
    #[couture(category = "all")]
    TupleNamed(Leaf),
    #[command(name = "inline-named")]
    #[couture(category = "all")]
    InlineNamed {
        #[arg(long)]
        #[couture(prompt)]
        inline_arg: String,
    },
    #[command(flatten)]
    Merged(Nested),
    #[command(skip)]
    #[expect(dead_code, reason = "clap makes no command of a skipped variant")]
    Skipped,
    #[command(external_subcommand)]
    #[expect(dead_code, reason = "only how clap names the other variants is checked")]
    External(Vec<String>),
}

/// Every way an attribute names a variant through the deprecated `id`, which clap reads as its
/// name.
#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum NamedById {
    #[command(id = "via-id")]
    #[couture(category = "all")]
    ById,
    #[command(name = "first", id = "id-last")]
    #[couture(category = "all")]
    NameThenId,
    #[command(id = "first", name = "name-last")]
    #[couture(category = "all")]
    IdThenName,
    #[clap(id = "via-clap-id")]
    #[couture(category = "all")]
    ClapId,
    #[command(id = "inline-via-id")]
    #[couture(category = "all")]
    InlineById {
        #[arg(long)]
        #[couture(prompt)]
        by_id_arg: String,
    },
}

/// Fields, a flattened struct, and the named subcommands, in one tree.
#[derive(Parser, Couture)]
#[command(name = "tree")]
struct Tree {
    #[command(flatten)]
    fields: struct_kebab_case::Fields,

    #[command(subcommand)]
    cmd: NamedByAttrs,
}

/// The arg of `cmd` at `path` with `id`.
fn arg_at<'cmd>(cmd: &'cmd Command, path: &[&str], id: &str) -> Option<&'cmd clap::Arg> {
    let level = path.iter().try_fold(cmd, |level, name| level.find_subcommand(name))?;
    level.get_arguments().find(|arg| arg.get_id() == id)
}

/// Check that `T`'s prompted args are clap's args of `cmd`, in clap's order; the count checked.
fn check_args<T>(label: &str) -> usize
where
    T: clap::CommandFactory + Couture,
{
    let cmd = T::command();
    let clap: Vec<&str> = cmd.get_arguments().map(|arg| arg.get_id().as_str()).collect();
    let prompted: Vec<&str> =
        T::PROMPTED_ARGS.prompted_args().iter().map(|prompted| prompted.attr.arg_id).collect();
    assert_eq!(prompted, clap, "{label}");
    clap.len()
}

/// Check that couture names `T`'s variants as clap does, for its categories, its help tree and its
/// prompts; the count of names checked.
fn check_subcommands<T>(label: &str) -> usize
where
    T: Subcommand + Couture,
{
    let mut root = T::augment_subcommands(Command::new("root"));
    root.build();
    let clap: Vec<&str> =
        root.get_subcommands().map(Command::get_name).filter(|name| *name != "help").collect();
    let categorized: Vec<&str> = T::CATEGORIES.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(categorized, clap, "categories, in {label}");
    let tree_names = check_help_tree(&T::HELP_TREE, &root, label);
    for prompted in T::PROMPTED_ARGS.prompted_args() {
        let (path, id) = (&prompted.subcommand_path, prompted.attr.arg_id);
        assert!(arg_at(&root, path, id).is_some(), "no `{id}` at {path:?}, in {label}");
    }
    clap.len().saturating_add(tree_names)
}

/// Check that every subcommand `tree` names is one of built `level`'s; the count checked.
fn check_help_tree(tree: &HelpTree, level: &Command, label: &str) -> usize {
    tree.branches
        .iter()
        .map(|branch| match branch {
            HelpBranch::Flattened(merged) => check_help_tree(merged, level, label),
            HelpBranch::Subcommand(name, below) => {
                let sub = level.find_subcommand(name);
                assert!(
                    sub.is_some(),
                    "no subcommand `{name}` below `{}`, in {label}",
                    level.get_name()
                );
                sub.map_or(0, |sub| check_help_tree(below, sub, label).saturating_add(1))
            }
            _ => 0,
        })
        .sum()
}

#[test]
fn every_casing_names_variants_as_clap_does() {
    let checked: usize = [
        enum_unset::check(),
        enum_camel_case::check(),
        enum_camel::check(),
        enum_kebab_case::check(),
        enum_kebab::check(),
        enum_kebab_pascal_spelled::check(),
        enum_pascal_case::check(),
        enum_pascal::check(),
        enum_screaming_snake_case::check(),
        enum_screaming_snake::check(),
        enum_snake_case::check(),
        enum_snake_pascal_spelled::check(),
        enum_snake::check(),
        enum_lowercase::check(),
        enum_lower::check(),
        enum_uppercase::check(),
        enum_upper::check(),
        enum_verbatim::check(),
        enum_verbatim_case::check(),
    ]
    .iter()
    .sum();
    assert!(checked > 3000, "only {checked} names were checked");
}

#[test]
fn every_casing_ids_args_as_clap_does() {
    let checked: usize = [
        struct_unset::check(),
        struct_camel_case::check(),
        struct_kebab_case::check(),
        struct_lowercase::check(),
        struct_pascal_case::check(),
        struct_screaming_snake_case::check(),
        struct_snake_case::check(),
        struct_uppercase::check(),
        struct_verbatim::check(),
    ]
    .iter()
    .sum();
    assert!(checked > 100, "only {checked} ids were checked");
}

#[test]
fn every_naming_attribute_names_a_variant_as_clap_does() {
    check_subcommands::<NamedByAttrs>("NamedByAttrs");
}

#[test]
fn the_deprecated_id_names_a_variant_as_clap_does() {
    check_subcommands::<NamedById>("NamedById");
}

#[test]
fn a_whole_tree_prompts_args_clap_defines() {
    let cmd = Tree::command();
    let prompted = Tree::PROMPTED_ARGS.prompted_args();
    assert!(prompted.len() > 15, "{prompted:?}");
    for prompted in prompted {
        let (path, id) = (&prompted.subcommand_path, prompted.attr.arg_id);
        assert!(arg_at(&cmd, path, id).is_some(), "no `{id}` at {path:?}");
    }
}
