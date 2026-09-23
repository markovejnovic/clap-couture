//! `#[derive(Couture)]` must name every variant exactly as `#[derive(Subcommand)]` does, or the
//! variant silently drops out of its category. clap is the reference.

use clap::{Command, Subcommand};
use clap_couture::Couture;
use rstest::rstest;

#[derive(Subcommand, Couture)]
#[couture(categories = { "all" = {} })]
enum DefaultCasing {
    #[category("all")]
    DryRun,
    #[command(name = "ls")]
    #[category("all")]
    List,
    #[category("all")]
    r#Type,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
#[couture(categories = { "all" = {} })]
enum EnumCasing {
    #[category("all")]
    DryRun,
}

#[derive(Subcommand, Couture)]
#[command(rename_all = "snake_case")]
#[couture(categories = { "all" = {} })]
enum VariantCasing {
    #[command(rename_all = "kebab-case")]
    #[category("all")]
    DryRun,
    #[command(name = "ls", rename_all = "SCREAMING_SNAKE_CASE")]
    #[category("all")]
    List,
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
fn categories_name_variants_the_way_clap_does(#[case] names: (Vec<String>, Vec<String>)) {
    let (clap, couture) = names;
    assert_eq!(couture, clap);
}
