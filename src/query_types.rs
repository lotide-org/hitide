use serde_derive::Serialize;

#[derive(Serialize, Default)]
pub struct PostListQuery<'a> {
    pub include_your: Option<bool>,
    pub sort: Option<&'a str>,
    pub search: Option<&'a str>,
    pub in_any_local_community: Option<bool>,
    pub use_aggregate_filters: Option<bool>,
    pub community: Option<i64>,
    pub in_your_follows: Option<bool>,
    pub created_within: Option<&'a str>,
    pub sort_sticky: Option<bool>,
    pub limit: Option<u8>,
    pub page: Option<&'a str>,
}

#[derive(Serialize)]
pub struct FlagListQuery {
    pub to_this_site_admin: Option<bool>,
    pub to_community: Option<i64>,
}
