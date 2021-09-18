pub mod timeago;

use std::borrow::{Borrow, Cow};
use std::collections::HashMap;

use crate::resp_types::{
    RespCommentInfo, RespCommunityInfoMaybeYour, RespMinimalAuthorInfo, RespMinimalCommentInfo,
    RespMinimalCommunityInfo, RespNotification, RespNotificationInfo, RespPostCommentInfo,
    RespPostInfo, RespPostListPost, RespThingComment, RespThingInfo, RespUserInfo,
};
use crate::util::{abbreviate_link, author_is_me};
use crate::PageBaseData;

pub use timeago::TimeAgo;

#[render::component]
pub fn Comment<'a>(
    comment: &'a RespPostCommentInfo<'a>,
    sort: crate::SortType,
    base_data: &'a PageBaseData,
    lang: &'a crate::Translator,
) {
    render::rsx! {
        <li class={"comment"}>
            {
                if base_data.login.is_some() {
                    Some(render::rsx! {
                        <div class={"votebox"}>
                            {
                                if comment.your_vote.is_some() {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/comments/{}/unlike", comment.as_ref().id)}>
                                            <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTED.img()}</button>
                                        </form>
                                    }
                                } else {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/comments/{}/like", comment.as_ref().id)}>
                                            <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTE.img()}</button>
                                        </form>
                                    }
                                }
                            }
                        </div>
                    })
                } else {
                    None
                }
            }
            <div class={"content"}>
                <small>
                    <cite><UserLink lang user={comment.author.as_ref()} /></cite>
                    {" "}
                    <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&comment.created).unwrap()} lang />
                </small>
                <div class={"commentContent"}>
                    <Content src={comment} />
                </div>
                {
                    comment.attachments.iter().map(|attachment| {
                        let href = &attachment.url;
                        render::rsx! {
                            <div>
                                <strong>{lang.tr("comment_attachment_prefix", None)}</strong>
                                {" "}
                                <em><a href={href.as_ref()}>{abbreviate_link(&href)}{" ↗"}</a></em>
                            </div>
                        }
                    })
                    .collect::<Vec<_>>()
                }
                <div class={"actionList small"}>
                    {
                        if base_data.login.is_some() {
                            Some(render::rsx! {
                                <a href={format!("/comments/{}?sort={}", comment.as_ref().id, sort.as_str())}>{lang.tr("reply", None)}</a>
                            })
                        } else {
                            None
                        }
                    }
                    {
                        if author_is_me(&comment.author, &base_data.login) || (comment.local && base_data.is_site_admin()) {
                            Some(render::rsx! {
                                <a href={format!("/comments/{}/delete", comment.as_ref().id)}>{lang.tr("delete", None)}</a>
                            })
                        } else {
                            None
                        }
                    }
                </div>
            </div>

            {
                if let Some(replies) = &comment.replies {
                    if replies.items.is_empty() {
                        None
                    } else {
                        Some(render::rsx! {
                            <>
                                <ul class={"commentList"}>
                                    {
                                        replies.items.iter().map(|reply| {
                                            render::rsx! {
                                                <Comment sort={sort} comment={reply} base_data lang />
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                    }
                                </ul>
                                {
                                    replies.next_page.as_ref().map(|next_page| {
                                        render::rsx! {
                                            <a href={format!("/comments/{}?sort={}&page={}", comment.base.id, sort.as_str(), next_page)}>{"-> "}{lang.tr("view_more_comments", None)}</a>
                                        }
                                    })
                                }
                            </>
                        })
                    }
                } else {
                    None
                }
            }
            {
                if comment.replies.is_none() {
                    Some(render::rsx! {
                        <ul><li><a href={format!("/comments/{}", comment.as_ref().id)}>{"-> "}{lang.tr("view_more_comments", None)}</a></li></ul>
                    })
                } else {
                    None
                }
            }
        </li>
    }
}

pub struct CommunityLink<'community> {
    pub community: &'community RespMinimalCommunityInfo<'community>,
}
impl<'community> render::Render for CommunityLink<'community> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        let community = &self.community;

        let href = format!("/communities/{}", community.id);
        (render::rsx! {
            <a href={&href}>
            {
                (if community.local {
                    community.name.as_ref().into()
                } else {
                    Cow::Owned(format!("{}@{}", community.name, community.host))
                }).as_ref()
            }
            </a>
        })
        .render_into(writer)
    }
}

pub trait HavingContent {
    fn content_text(&self) -> Option<&str>;
    fn content_html(&self) -> Option<&str>;
}

impl<'a> HavingContent for RespMinimalCommentInfo<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text.as_deref()
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html.as_deref()
    }
}

impl<'a> HavingContent for RespThingComment<'a> {
    fn content_text(&self) -> Option<&str> {
        self.base.content_text()
    }
    fn content_html(&self) -> Option<&str> {
        self.base.content_html()
    }
}

impl<'a> HavingContent for RespPostCommentInfo<'a> {
    fn content_text(&self) -> Option<&str> {
        self.base.content_text()
    }
    fn content_html(&self) -> Option<&str> {
        self.base.content_html()
    }
}

impl<'a> HavingContent for RespCommentInfo<'a> {
    fn content_text(&self) -> Option<&str> {
        self.base.content_text()
    }
    fn content_html(&self) -> Option<&str> {
        self.base.content_html()
    }
}

impl<'a> HavingContent for RespPostInfo<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text.as_deref()
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html.as_deref()
    }
}

pub struct HavingContentRef<'a> {
    content_html: Option<&'a str>,
    content_text: Option<&'a str>,
}

impl<'a> HavingContent for HavingContentRef<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html
    }
}

impl<'a> RespUserInfo<'a> {
    pub fn description(&'a self) -> HavingContentRef<'a> {
        HavingContentRef {
            content_html: self.description_html.as_deref(),
            content_text: self.description_text.as_deref(),
        }
    }
}

impl<'a> RespCommunityInfoMaybeYour<'a> {
    pub fn description(&'a self) -> HavingContentRef<'a> {
        HavingContentRef {
            content_html: self.description_html.as_deref(),
            content_text: self.description_text.as_deref(),
        }
    }
}

pub struct Content<'a, T: HavingContent + 'a> {
    pub src: &'a T,
}

impl<'a, T: HavingContent + 'a> render::Render for Content<'a, T> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        match self.src.content_html() {
            Some(html) => {
                writer.write_str("<div>")?;
                render::raw!(html).render_into(writer)?;
                writer.write_str("</div>")?;
            }
            None => {
                if let Some(text) = self.src.content_text() {
                    writer.write_str("<div>")?;
                    text.render_into(writer)?;
                    writer.write_str("</div>")?;
                }
            }
        }

        Ok(())
    }
}

#[render::component]
pub fn HTPage<'a, Children: render::Render>(
    base_data: &'a PageBaseData,
    lang: &'a crate::Translator,
    title: &'a str,
    children: Children,
) {
    render::rsx! {
        <HTPageAdvanced base_data={base_data} lang={lang} title={title} head_items={()}>{children}</HTPageAdvanced>
    }
}

#[render::component]
pub fn HTPageAdvanced<'a, HeadItems: render::Render, Children: render::Render>(
    base_data: &'a PageBaseData,
    lang: &'a crate::Translator,
    title: &'a str,
    head_items: HeadItems,
    children: Children,
) {
    render::rsx! {
        <>
            <render::html::HTML5Doctype />
            <html>
                <head>
                    <meta charset={"utf-8"} />
                    <link rel={"stylesheet"} href={"/static/main.css"} />
                    <title>{title}</title>
                    {head_items}
                </head>
                <body>
                    <header class={"mainHeader"}>
                        <div class={"left actionList"}>
                            <a href={"/"} class={"siteName"}>{"lotide"}</a>
                            <a href={"/all"}>{lang.tr("all", None)}</a>
                            <a href={"/local"}>{lang.tr("local", None)}</a>
                            <a href={"/communities"}>{lang.tr("communities", None)}</a>
                            <a href={"/about"}>{lang.tr("about", None)}</a>
                        </div>
                        <div class={"right actionList"}>
                            {
                                if let Some(login) =  &base_data.login {
                                    Some(render::rsx! {
                                        <>
                                            <a
                                                href={"/notifications"}
                                            >
                                                {
                                                    if login.user.has_unread_notifications {
                                                        hitide_icons::NOTIFICATIONS_SOME.img()
                                                    } else {
                                                        hitide_icons::NOTIFICATIONS.img()
                                                    }
                                                }
                                            </a>
                                            <a href={format!("/users/{}", login.user.id)}>
                                                {hitide_icons::PERSON.img()}
                                            </a>
                                            <form method={"POST"} action={"/logout"} class={"inline"}>
                                                <button type={"submit"} class={"iconbutton"}>
                                                    {hitide_icons::LOGOUT.img()}
                                                </button>
                                            </form>
                                        </>
                                    })
                                } else {
                                    None
                                }
                            }
                            {
                                if base_data.login.is_none() {
                                    Some(render::rsx! {
                                        <a href={"/login"}>{lang.tr("login", None)}</a>
                                    })
                                } else {
                                    None
                                }
                            }
                        </div>
                    </header>
                    {children}
                </body>
            </html>
        </>
    }
}

#[render::component]
pub fn PostItem<'a>(
    post: &'a RespPostListPost<'a>,
    in_community: bool,
    no_user: bool,
    lang: &'a crate::Translator,
) {
    let post_href = format!("/posts/{}", post.as_ref().as_ref().id);

    render::rsx! {
        <li class={if post.as_ref().sticky { "sticky" } else { "" }}>
            <div class={"titleLine"}>
                <a href={post_href.clone()}>
                    {post.as_ref().as_ref().title.as_ref()}
                </a>
                {
                    if let Some(href) = &post.as_ref().href {
                        Some(render::rsx! {
                            <em><a href={href.as_ref()}>{abbreviate_link(&href)}{" ↗"}</a></em>
                        })
                    } else {
                        None
                    }
                }
            </div>
            <small>
                {lang.tr("submitted", None)}
                {" "}
                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&post.as_ref().created).unwrap()} lang />
                {
                    if no_user {
                        None
                    } else {
                        Some(render::rsx! {
                            <>
                                {" "}{lang.tr("by", None)}{" "}<UserLink lang user={post.as_ref().author.as_ref()} />
                            </>
                        })
                    }
                }
                {
                    if !in_community {
                        Some(render::rsx! {
                            <>{" "}{lang.tr("to", None)}{" "}<CommunityLink community={&post.as_ref().community} /></>
                        })
                    } else {
                        None
                    }
                }
                {" | "}
                <a href={post_href}>{lang.tr("post_comments_count", Some(&fluent::fluent_args!["count" => post.replies_count_total])).into_owned()}</a>
            </small>
        </li>
    }
}

pub struct ThingItem<'a> {
    pub lang: &'a crate::Translator,
    pub thing: &'a RespThingInfo<'a>,
}

impl<'a> render::Render for ThingItem<'a> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        let lang = self.lang;

        match self.thing {
            RespThingInfo::Post(post) => {
                (PostItem { post, in_community: false, no_user: true, lang: self.lang }).render_into(writer)
            },
            RespThingInfo::Comment(comment) => {
                (render::rsx! {
                    <li>
                        <small>
                            <a href={format!("/comments/{}", comment.as_ref().id)}>{lang.tr("comment", None)}</a>
                            {" "}{lang.tr("on", None)}{" "}<a href={format!("/posts/{}", comment.post.id)}>{comment.post.title.as_ref()}</a>{":"}
                        </small>
                        <Content src={comment} />
                    </li>
                }).render_into(writer)
            }
        }
    }
}

pub struct UserLink<'a> {
    pub lang: &'a crate::Translator,
    pub user: Option<&'a RespMinimalAuthorInfo<'a>>,
}

impl<'user> render::Render for UserLink<'user> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        match self.user {
            None => "[unknown]".render_into(writer),
            Some(user) => {
                let href = format!("/users/{}", user.id);
                (render::rsx! {
                    <a href={&href}>
                        {
                            (if user.local {
                                user.username.as_ref().into()
                            } else {
                                Cow::Owned(format!("{}@{}", user.username, user.host))
                            }).as_ref()
                        }
                        {
                            if user.is_bot {
                                Some(format!(" [{}]", self.lang.tr("user_bot_tag", None)))
                            } else {
                                None
                            }
                        }
                    </a>
                })
                .render_into(writer)
            }
        }
    }
}

pub trait GetIndex<K, V> {
    fn get(&self, key: K) -> Option<&V>;
}

impl<K: Borrow<Q> + Eq + std::hash::Hash, V, Q: ?Sized + Eq + std::hash::Hash> GetIndex<&Q, V>
    for HashMap<K, V>
{
    fn get<'a>(&'a self, key: &Q) -> Option<&'a V> {
        HashMap::get(self, key)
    }
}

impl<I: serde_json::value::Index> GetIndex<I, serde_json::Value> for serde_json::Value {
    fn get(&self, key: I) -> Option<&serde_json::Value> {
        self.get(key)
    }
}

fn maybe_fill_value<'a, 'b, M: GetIndex<&'b str, serde_json::Value>>(
    values: &'a Option<&'a M>,
    name: &'b str,
    default_value: Option<&'a str>,
) -> &'a str {
    values
        .and_then(|values| values.get(name))
        .and_then(serde_json::Value::as_str)
        .or(default_value)
        .unwrap_or("")
}

#[render::component]
pub fn MaybeFillInput<'a, M: GetIndex<&'a str, serde_json::Value>>(
    values: &'a Option<&'a M>,
    r#type: &'a str,
    name: &'a str,
    required: bool,
    id: &'a str,
) {
    let value = maybe_fill_value(values, name, None);
    if required {
        render::rsx! {
            <input
                r#type
                name
                value
                id
                required={""}
            />
        }
    } else {
        render::rsx! {
            <input
                r#type
                name
                value
                id
            />
        }
    }
}

#[render::component]
pub fn MaybeFillTextArea<'a, M: GetIndex<&'a str, serde_json::Value>>(
    values: &'a Option<&'a M>,
    name: &'a str,
    default_value: Option<&'a str>,
) {
    render::rsx! {
        <textarea name>
            {maybe_fill_value(values, name, default_value)}
        </textarea>
    }
}

#[render::component]
pub fn BoolSubmitButton<'a>(value: bool, do_text: &'a str, done_text: &'a str) {
    if value {
        render::rsx! {
            <button disabled={""}>{done_text}</button>
        }
    } else {
        render::rsx! {
            <button type={"submit"}>{do_text}</button>
        }
    }
}

#[render::component]
pub fn BoolCheckbox<'a>(name: &'a str, value: bool) {
    if value {
        render::rsx! {
            <input name type={"checkbox"} checked={""} />
        }
    } else {
        render::rsx! {
            <input type={"checkbox"} name />
        }
    }
}

pub struct NotificationItem<'a> {
    pub notification: &'a RespNotification<'a>,
    pub lang: &'a crate::Translator,
}

impl<'a> render::Render for NotificationItem<'a> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        let lang = self.lang;

        write!(writer, "<li class=\"notification-item")?;
        if self.notification.unseen {
            write!(writer, " unread")?;
        }
        write!(writer, "\">")?;
        match &self.notification.info {
            RespNotificationInfo::Unknown => {
                "[unknown notification type]".render_into(writer)?;
            }
            RespNotificationInfo::PostReply { reply, post } => {
                (render::rsx! {
                    <>
                        <a href={format!("/comments/{}", reply.id)}>{lang.tr("comment", None)}</a>
                        {" "}{lang.tr("on_your_post", None)}{" "}<a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a>{":"}
                        <Content src={reply} />
                    </>
                }).render_into(writer)?;
            }
            RespNotificationInfo::CommentReply {
                reply,
                comment,
                post,
            } => {
                (render::rsx! {
                    <>
                        {lang.tr("reply_to", None)}
                        {" "}
                        <a href={format!("/comments/{}", comment)}>{lang.tr("your_comment", None)}</a>
                        {
                            if let Some(post) = post {
                                Some(render::rsx! { <>{" "}{lang.tr("on", None)}{" "}<a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a></> })
                            } else {
                                None
                            }
                        }
                        {":"}
                        <Content src={reply} />
                    </>
                }).render_into(writer)?;
            }
        }

        write!(writer, "</li>")
    }
}

pub trait IconExt {
    fn img(&self) -> render::SimpleElement<()>;
}

impl IconExt for hitide_icons::Icon {
    fn img(&self) -> render::SimpleElement<()> {
        render::rsx! {
            <img src={format!("/static/{}", self.path)} class={if self.dark_invert { "icon darkInvert" } else { "icon" }} />
        }
    }
}
