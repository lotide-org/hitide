use serde_derive::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum RespFlagDetails<'a> {
    Post {
        #[serde(borrow)]
        post: RespPostListPost<'a>,
    },
}

#[derive(Deserialize, Debug)]
pub struct RespFlagInfo<'a> {
    pub id: i64,
    pub flagger: RespMinimalAuthorInfo<'a>,
    pub created_local: Cow<'a, str>,
    pub content: Option<JustContentText<'a>>,
    #[serde(borrow)]
    #[serde(flatten)]
    pub details: RespFlagDetails<'a>,
}

#[derive(Deserialize, Debug)]
pub struct RespMinimalAuthorInfo<'a> {
    pub id: i64,
    pub username: Cow<'a, str>,
    pub local: bool,
    pub host: Cow<'a, str>,
    pub remote_url: Option<Cow<'a, str>>,
    pub is_bot: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespMinimalPostInfo<'a> {
    pub id: i64,
    pub title: Cow<'a, str>,
    pub remote_url: Option<Cow<'a, str>>,
    pub sensitive: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespSomePostInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalPostInfo<'a>,
    pub href: Option<Cow<'a, str>>,
    #[serde(borrow)]
    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    #[serde(borrow)]
    pub community: RespMinimalCommunityInfo<'a>,
    pub sticky: bool,
}

impl<'a> AsRef<RespMinimalPostInfo<'a>> for RespSomePostInfo<'a> {
    fn as_ref(&self) -> &RespMinimalPostInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespPostListPost<'a> {
    #[serde(flatten, borrow)]
    pub base: RespSomePostInfo<'a>,
    pub replies_count_total: i64,
}

impl<'a> AsRef<RespSomePostInfo<'a>> for RespPostListPost<'a> {
    fn as_ref(&self) -> &RespSomePostInfo<'a> {
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
    pub sensitive: bool,
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
pub struct JustURL<'a> {
    pub url: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespPostCommentInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalCommentInfo<'a>,

    pub attachments: Vec<JustURL<'a>>,

    pub author: Option<RespMinimalAuthorInfo<'a>>,
    pub created: Cow<'a, str>,
    pub local: bool,
    pub your_vote: Option<Empty>,
    pub replies: Option<RespList<'a, RespPostCommentInfo<'a>>>,
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
    pub base: RespSomePostInfo<'a>,

    pub content_text: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
    pub approved: bool,
    pub rejected: bool,
    pub score: i64,
    pub local: bool,
    pub your_vote: Option<Empty>,
    pub poll: Option<RespPollInfo<'a>>,
}

impl<'a> AsRef<RespSomePostInfo<'a>> for RespPostInfo<'a> {
    fn as_ref(&self) -> &RespSomePostInfo<'a> {
        &self.base
    }
}

#[derive(Deserialize, Debug)]
pub struct RespPollInfo<'a> {
    pub multiple: bool,
    pub options: Vec<RespPollOption<'a>>,
    pub is_closed: bool,
    pub your_vote: Option<RespPollYourVote>,
}

#[derive(Deserialize, Debug)]
pub struct RespPollOption<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub votes: u32,
}

#[derive(Deserialize, Debug)]
pub struct RespPollYourVote {
    pub options: Vec<JustID>,
}

#[derive(Deserialize, Debug)]
pub struct RespMinimalCommunityInfo<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub local: bool,
    pub host: Cow<'a, str>,
    pub remote_url: Option<Cow<'a, str>>,
    pub deleted: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespUserInfo<'a> {
    #[serde(flatten)]
    pub base: RespMinimalAuthorInfo<'a>,
    pub description: Content<'a>,
    pub suspended: Option<bool>,
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
    pub is_site_admin: bool,
    pub has_unread_notifications: bool,
    pub has_pending_moderation_actions: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginInfo {
    pub user: RespLoginInfoUser,
    pub permissions: RespLoginPermissions,
}

#[derive(Deserialize, Debug)]
pub struct RespLoginPermissions {
    pub create_community: RespPermissionInfo,
    pub create_invitation: RespPermissionInfo,
}

#[derive(Deserialize, Debug)]
pub struct RespPermissionInfo {
    pub allowed: bool,
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

#[derive(Deserialize, Debug)]
pub struct RespCommunityFeedsType<'a> {
    pub new: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
pub struct RespCommunityFeeds<'a> {
    pub atom: RespCommunityFeedsType<'a>,
}

#[derive(Deserialize, Debug)]
pub struct RespCommunityInfoMaybeYour<'a> {
    #[serde(flatten)]
    pub base: RespMinimalCommunityInfo<'a>,

    pub description: Content<'a>,
    pub feeds: RespCommunityFeeds<'a>,

    pub you_are_moderator: Option<bool>,
    pub your_follow: Option<RespYourFollow>,
    pub pending_moderation_actions: Option<u32>,
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
    pub description: Content<'a>,
    pub software: RespInstanceSoftwareInfo<'a>,
    pub signup_allowed: bool,
}

#[derive(Deserialize, Debug)]
pub struct RespInvitationInfo<'a> {
    pub id: i32,
    pub key: Cow<'a, str>,
    pub created_by: RespMinimalAuthorInfo<'a>,
    pub created_at: Cow<'a, str>,
    pub used: bool,
}

#[derive(Deserialize, Debug)]
pub struct InvitationsCreateResponse<'a> {
    pub id: i32,
    pub key: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum RespCommunityModlogEventDetails<'a> {
    RejectPost { post: RespMinimalPostInfo<'a> },
    ApprovePost { post: RespMinimalPostInfo<'a> },
}

#[derive(Deserialize, Debug)]
pub struct RespCommunityModlogEvent<'a> {
    pub time: Cow<'a, str>,
    #[serde(flatten)]
    pub details: RespCommunityModlogEventDetails<'a>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum RespSiteModlogEventDetails<'a> {
    DeletePost {
        author: RespMinimalAuthorInfo<'a>,
        community: RespMinimalCommunityInfo<'a>,
    },
    DeleteComment {
        author: RespMinimalAuthorInfo<'a>,
        post: RespMinimalPostInfo<'a>,
    },
    SuspendUser {
        user: RespMinimalAuthorInfo<'a>,
    },
    UnsuspendUser {
        user: RespMinimalAuthorInfo<'a>,
    },
}

#[derive(Deserialize, Debug)]
pub struct RespSiteModlogEvent<'a> {
    pub time: Cow<'a, str>,
    #[serde(flatten)]
    pub details: RespSiteModlogEventDetails<'a>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum RespNotificationInfo<'a> {
    PostReply {
        reply: RespPostCommentInfo<'a>,
        #[serde(borrow)]
        post: RespPostListPost<'a>,
    },
    PostMention {
        #[serde(borrow)]
        post: RespPostListPost<'a>,
    },
    CommentReply {
        reply: RespPostCommentInfo<'a>,
        comment: RespPostCommentInfo<'a>,
        #[serde(borrow)]
        post: RespPostListPost<'a>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug)]
pub struct RespNotification<'a> {
    #[serde(flatten)]
    #[serde(borrow)]
    pub info: RespNotificationInfo<'a>,

    pub unseen: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Content<'a> {
    pub content_text: Option<Cow<'a, str>>,
    pub content_markdown: Option<Cow<'a, str>>,
    pub content_html: Option<Cow<'a, str>>,
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
