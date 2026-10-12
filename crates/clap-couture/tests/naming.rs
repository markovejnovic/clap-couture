//! `#[derive(Couture)]` must name every variant exactly as `#[derive(Subcommand)]` does, or the
//! variant silently drops out of its category. clap is the reference.

use clap::{Command, Subcommand};
use clap_couture::Couture;
use rstest::rstest;

/// The name clap evaluates, which couture can't read off the attribute.
const LIST_NAME: &str = "ls";

#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum DefaultCasing {
    #[couture(category = "all")]
    DryRun,
    #[command(name = "ls")]
    #[couture(category = "all")]
    List,
    #[couture(category = "all")]
    r#Type,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
#[couture(categories = { "all" = {} })]
enum EnumCasing {
    #[couture(category = "all")]
    DryRun,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
#[couture(categories = { "all" = {} })]
enum VariantCasing {
    #[command(rename_all = "kebab-case")]
    #[couture(category = "all")]
    DryRun,
    #[command(name = "ls", rename_all = "SCREAMING_SNAKE_CASE")]
    #[couture(category = "all")]
    List,
}

/// Every casing clap accepts, each spelled the ways clap parses it.
#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum EveryCasing {
    #[command(rename_all = "camelCase")]
    #[couture(category = "all")]
    CamelCase,
    #[command(rename_all = "kebab")]
    #[couture(category = "all")]
    KebabShort,
    #[command(rename_all = "lower")]
    #[couture(category = "all")]
    LowerCase,
    #[command(rename_all = "PascalCase")]
    #[couture(category = "all")]
    PascalCase,
    #[command(rename_all = "screaming_snake")]
    #[couture(category = "all")]
    ScreamingSnake,
    #[command(rename_all = "SnakeCase")]
    #[couture(category = "all")]
    SnakeCase,
    #[command(rename_all = "UPPERCASE")]
    #[couture(category = "all")]
    UpperCase,
    #[command(rename_all = "verbatim")]
    #[couture(category = "all")]
    VerbatimCase,
}

/// clap keeps the last of several `name`s.
#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum RenamedTwice {
    #[command(name = "first")]
    #[command(name = "second")]
    #[couture(category = "all")]
    List,
}

/// clap reads the deprecated `id` of a command as its name.
#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum RenamedById {
    #[command(id = "via-id")]
    #[couture(category = "all")]
    List,
    #[command(name = "first", id = "id-last")]
    #[couture(category = "all")]
    Show,
}

#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum NamedByConst {
    #[command(name = LIST_NAME)]
    #[couture(category = "all")]
    List,
}

/// The flattened variant goes last: `CATEGORIES` lists an enum's own commands before its
/// flattened ones, while clap lists them in variant order.
#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum Flattening {
    #[couture(category = "all")]
    Apply,
    #[command(flatten)]
    Merged(DefaultCasing),
}

/// The names clap gives `T`'s variants, and the names `T::CATEGORIES` files them under.
fn clap_and_couture_names<T>() -> (Vec<String>, Vec<String>)
where
    T: Subcommand + Couture,
{
    let clap = T::augment_subcommands(Command::new("app"))
        .get_subcommands()
        .map(|cmd| cmd.get_name().to_owned())
        .collect();
    let couture = T::CATEGORIES.iter().map(|(name, _)| name.as_str().to_owned()).collect();
    (clap, couture)
}

#[rstest]
#[case::default_casing(clap_and_couture_names::<DefaultCasing>())]
#[case::enum_rename_all(clap_and_couture_names::<EnumCasing>())]
#[case::variant_rename_all(clap_and_couture_names::<VariantCasing>())]
#[case::every_casing(clap_and_couture_names::<EveryCasing>())]
#[case::renamed_twice(clap_and_couture_names::<RenamedTwice>())]
#[case::renamed_by_id(clap_and_couture_names::<RenamedById>())]
#[case::flatten(clap_and_couture_names::<Flattening>())]
#[case::name_from_a_const(clap_and_couture_names::<NamedByConst>())]
fn categories_name_variants_the_way_clap_does(#[case] names: (Vec<String>, Vec<String>)) {
    let (clap, couture) = names;
    assert_eq!(couture, clap);
}
