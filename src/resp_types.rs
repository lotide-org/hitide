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
pub struct RespMinimalPostInfo<'a> {
    pub id: i64,
    pub title: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostListPost<'a> {
    #[serde(flatten)]
    pub base: RespMinimalPostInfo<'a>,
    pub href: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    #[serde(borrow)]
    pub community: RespMinimalCommunityInfo<'a>,
}

impl<'a> AsRef<RespMinimalPostInfo<'a>> for RespPostListPost<'a> {
    fn as_ref(&self) -> &RespMinimalPostInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RespThingInfo<'a> {
    #[serde(rename = "post")]
    Post(RespPostListPost<'a>),
    #[serde(rename = "comment")]
    #[serde(borrow)]
    Comment(RespThingComment<'a>),
}

#[derive(Deserialize, Debug)]
pub struct RespThingComment<'a> {
    pub id: i64,
    pub created: Cow<'a, str>,
    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub post: RespMinimalPostInfo<'a>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostCommentInfo<'a> {
    pub id: i64,
    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    pub your_vote: Option<Empty>,
    #[serde(borrow)]
    pub replies: Option<Vec<RespPostCommentInfo<'a>>>,
    pub has_replies: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespPostInfo<'a> {
    #[serde(flatten, borrow)]
    pub base: RespPostListPost<'a>,

    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    pub score: i64,
    #[serde(borrow)]
    pub comments: Vec<RespPostCommentInfo<'a>>,
    pub your_vote: Option<Empty>,
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
pub struct RespUserInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalAuthorInfo<'a>,
    pub description: Cow<'a, str>,
}

impl<'a> AsRef<RespMinimalAuthorInfo<'a>> for RespUserInfo<'a> {
    fn as_ref(&self) -> &RespMinimalAuthorInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfoUser {
    pub id: i64,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfo {
    pub user: RespLoginInfoUser,
}

#[derive(Deserialize, Debug)]
pub struct Empty {}

#[derive(Deserialize, Debug)]
pub struct RespYourFollow {
    pub accepted: bool,
}

#[derive(Deserialize)]
pub struct RespCommunityInfoMaybeYour<'a> {
    #[serde(flatten)]
    pub base: RespMinimalCommunityInfo<'a>,

    pub description: Cow<'a, str>,

    pub you_are_moderator: Option<bool>,
    pub your_follow: Option<RespYourFollow>,
}

impl<'a> AsRef<RespMinimalCommunityInfo<'a>> for RespCommunityInfoMaybeYour<'a> {
    fn as_ref(&self) -> &RespMinimalCommunityInfo<'a> {
        &self.base
    }
}
