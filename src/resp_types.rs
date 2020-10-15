use serde_derive::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
pub struct RespMinimalAuthorInfo<'a> {
    pub id: i64,
    pub username: Cow<'a, str>,
    pub local: bool,
    pub host: Cow<'a, str>,
    pub remote_url: Option<Cow<'a, str>>,
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
pub struct RespMinimalCommentInfo<'a> {
    pub id: i64,
    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
}

#[derive(Deserialize, Debug)]
pub struct RespThingComment<'a> {
    #[serde(flatten)]
    pub base: RespMinimalCommentInfo<'a>,

    pub created: Cow<'a, str>,
    #[serde(borrow)]
    pub post: RespMinimalPostInfo<'a>,
}

impl<'a> AsRef<RespMinimalCommentInfo<'a>> for RespThingComment<'a> {
    fn as_ref(&self) -> &RespMinimalCommentInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespPostCommentInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalCommentInfo<'a>,

    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    pub your_vote: Option<Empty>,
    #[serde(borrow)]
    pub replies: Option<Vec<RespPostCommentInfo<'a>>>,
    pub has_replies: bool,
}

impl<'a> AsRef<RespMinimalCommentInfo<'a>> for RespPostCommentInfo<'a> {
    fn as_ref(&self) -> &RespMinimalCommentInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespCommentInfo<'a> {
    #[serde(flatten)]
    pub base: RespPostCommentInfo<'a>,

    pub parent: Option<JustID>,
    #[serde(borrow)]
    pub post: Option<RespMinimalPostInfo<'a>>,
}

impl<'a> AsRef<RespPostCommentInfo<'a>> for RespCommentInfo<'a> {
    fn as_ref(&self) -> &RespPostCommentInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespPostInfo<'a> {
    #[serde(flatten, borrow)]
    pub base: RespPostListPost<'a>,

    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    pub approved: bool,
    pub score: i64,
    #[serde(borrow)]
    pub replies: Vec<RespPostCommentInfo<'a>>,
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
    pub remote_url: Option<Cow<'a, str>>,
}

#[derive(Deserialize, Debug)]
pub struct RespUserInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalAuthorInfo<'a>,
    pub description: Cow<'a, str>,
    pub your_note: Option<JustContentText<'a>>,
}

impl<'a> AsRef<RespMinimalAuthorInfo<'a>> for RespUserInfo<'a> {
    fn as_ref(&self) -> &RespMinimalAuthorInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfoUser {
    pub id: i64,
    pub has_unread_notifications: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfo {
    pub user: RespLoginInfoUser,
}

#[derive(Deserialize, Debug)]
pub struct Empty {}

#[derive(Deserialize, Debug)]
pub struct JustID {
    pub id: i64,
}

#[derive(Deserialize, Debug)]
pub struct JustStringID<'a> {
    pub id: &'a str,
}

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

#[derive(Deserialize, Debug)]
pub struct RespInstanceSoftwareInfo<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespInstanceInfo<'a> {
    #[serde(default)]
    pub description: Cow<'a, str>,
    pub software: RespInstanceSoftwareInfo<'a>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum RespNotificationInfo<'a> {
    PostReply {
        reply: RespMinimalCommentInfo<'a>,
        post: RespMinimalPostInfo<'a>,
    },
    CommentReply {
        reply: RespMinimalCommentInfo<'a>,
        comment: i64,
        post: Option<RespMinimalPostInfo<'a>>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct RespNotification<'a> {
    #[serde(flatten)]
    pub info: RespNotificationInfo<'a>,

    pub unseen: bool,
}

#[derive(Deserialize, Debug)]
pub struct JustUser<'a> {
    pub user: RespMinimalAuthorInfo<'a>,
}

#[derive(Deserialize, Debug)]
pub struct JustContentText<'a> {
    pub content_text: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct JustContentHTML<'a> {
    pub content_html: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespList<'a, T: std::fmt::Debug + 'a> {
    pub items: Vec<T>,
    pub next_page: Option<Cow<'a, str>>,
}
