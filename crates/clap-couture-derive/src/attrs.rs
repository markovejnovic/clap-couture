use serde::Deserialize;
use serde_tokenstream::OrderedMap;

#[derive(Deserialize)]
pub(crate) struct CategoriesSpec {
    #[serde(default)]
    pub(crate) categories: OrderedMap<String, Category>,
    #[serde(default)]
    pub(crate) inherit: Option<Inherit>,
}

#[derive(Deserialize)]
pub(crate) struct Category {
    pub(crate) description: Option<String>,
    pub(crate) title: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Inherit {
    All(bool),
    List(Vec<String>),
}
