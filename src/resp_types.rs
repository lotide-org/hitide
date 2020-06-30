use serde_derive::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
pub struct RespMinimalAuthorInfo<'a> {
    pub id: i64,
    pub username: Cow<'a, str>,
    pub local: bool,
    pub host: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostListPost<'a> {
    pub id: i64,
    pub title: Cow<'a, str>,
    pub href: Option<Cow<'a, str>>,
    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    #[serde(borrow)]
    pub community: RespMinimalCommunityInfo<'a>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostCommentInfo<'a> {
    pub id: i64,
    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub replies: Option<Vec<RespPostCommentInfo<'a>>>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostInfo<'a> {
    #[serde(flatten, borrow)]
    pub base: RespPostListPost<'a>,
    pub score: i64,
    #[serde(borrow)]
    pub comments: Vec<RespPostCommentInfo<'a>>,
}

impl<'a> AsRef<RespPostListPost<'a>> for RespPostInfo<'a> {
    fn as_ref(&self) -> &RespPostListPost<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespMinimalCommunityInfo<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub local: bool,
    pub host: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfoUser {
    pub id: i64,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfo {
    pub user: RespLoginInfoUser,
}
