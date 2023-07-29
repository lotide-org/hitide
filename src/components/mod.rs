pub mod timeago;

use std::borrow::{Borrow, Cow};
use std::collections::HashMap;

use crate::lang;
use crate::resp_types::{
    Content, RespCommentInfo, RespFlagDetails, RespFlagInfo, RespMinimalAuthorInfo,
    RespMinimalCommentInfo, RespMinimalCommunityInfo, RespNotification, RespNotificationInfo,
    RespPollInfo, RespPostCommentInfo, RespPostInfo, RespPostListPost, RespSiteModlogEvent,
    RespSiteModlogEventDetails, RespThingComment, RespThingInfo,
};
use crate::util::{abbreviate_link, author_is_me};
use crate::PageBaseData;

pub use timeago::TimeAgo;

#[render::component]
pub fn Comment<'a>(
    comment: &'a RespPostCommentInfo<'a>,
    sort: crate::SortType,
    root_sensitive: bool,
    base_data: &'a PageBaseData,
    lang: &'a crate::Translator,
) {
    let sensitive_hide = !root_sensitive && comment.as_ref().sensitive;

    render::rsx! {
        <li class={"comment"} id={format!("comment{}", comment.as_ref().id)}>
            {
                if base_data.login.is_some() {
                    Some(render::rsx! {
                        <div class={"votebox"}>
                            {
                                if comment.your_vote.is_some() {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/comments/{}/unlike", comment.as_ref().id)}>
                                            <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTED.img(lang.tr(&lang::remove_upvote()).into_owned())}</button>
                                        </form>
                                    }
                                } else {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/comments/{}/like", comment.as_ref().id)}>
                                            <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTE.img(lang.tr(&lang::upvote()).into_owned())}</button>
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
            <details class={"commentCollapse"} open={"open"}>
                <summary>
                    <small>
                        <cite><UserLink lang user={comment.author.as_ref()} /></cite>
                        {" "}
                        <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&comment.created).unwrap()} lang />
                    </small>
                </summary>
                <div class={"content"}>
                    <div class={"commentContent"}>
                        {
                            sensitive_hide.then(|| {
                                render::rsx! {
                                    <details>
                                        <summary>
                                            {hitide_icons::SENSITIVE.img_aria_hidden()}
                                            {lang.tr(&lang::SENSITIVE)}
                                        </summary>
                                        <ContentView src={comment} />
                                    </details>
                                }
                            })
                        }
                        {
                            (!sensitive_hide).then(|| {
                                render::rsx! { <ContentView src={comment} /> }
                            })
                        }
                    </div>
                    {
                        comment.attachments.iter().map(|attachment| {
                            let href = &attachment.url;
                            render::rsx! {
                                <div>
                                    <strong>{lang.tr(&lang::COMMENT_ATTACHMENT_PREFIX)}</strong>
                                    {" "}
                                    <em><a href={href.as_ref()}>{abbreviate_link(href)}{" ↗"}</a></em>
                                </div>
                            }
                        })
                        .collect::<Vec<_>>()
                    }
                    <div class={"actionList small"}>
                        {
                            if base_data.login.is_some() {
                                Some(render::rsx! {
                                    <a href={format!("/comments/{}?sort={}", comment.as_ref().id, sort.as_str())}>{lang.tr(&lang::REPLY)}</a>
                                })
                            } else {
                                None
                            }
                        }
                        {
                            if !comment.local {
                                if let Some(remote_url) = &comment.as_ref().remote_url {
                                    Some(render::rsx! {
                                        <a href={remote_url.as_ref()}>{lang.tr(&lang::remote_url()).into_owned()}</a>
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        {
                            if author_is_me(&comment.author, &base_data.login) || (comment.local && base_data.is_site_admin()) {
                                Some(render::rsx! {
                                    <a href={format!("/comments/{}/delete", comment.as_ref().id)}>{lang.tr(&lang::DELETE)}</a>
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
                                                    <Comment sort={sort} comment={reply} root_sensitive base_data lang />
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                        }
                                    </ul>
                                    {
                                        replies.next_page.as_ref().map(|next_page| {
                                            render::rsx! {
                                                <a href={format!("/comments/{}?sort={}&page={}", comment.base.id, sort.as_str(), next_page)}>{"-> "}{lang.tr(&lang::VIEW_MORE_COMMENTS)}</a>
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
                            <ul><li><a href={format!("/comments/{}", comment.as_ref().id)}>{"-> "}{lang.tr(&lang::VIEW_MORE_COMMENTS)}</a></li></ul>
                        })
                    } else {
                        None
                    }
                }
            </details>
        </li>
    }
}

pub struct CommunityLink<'community> {
    pub community: &'community RespMinimalCommunityInfo<'community>,
}
impl<'community> render::Render for CommunityLink<'community> {
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
        let community = &self.community;

        if community.deleted {
            (render::rsx! {
                <strong>{"[deleted]"}</strong>
            })
            .render_into(writer)
        } else {
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

impl<'a> HavingContent for Content<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text.as_deref()
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html.as_deref()
    }
}

#[derive(Clone)]
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

#[derive(Clone)]
pub struct ContentView<'a, T: HavingContent + 'a> {
    pub src: &'a T,
}

impl<'a, T: HavingContent + 'a> render::Render for ContentView<'a, T> {
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
        match self.src.content_html() {
            Some(html) => {
                (render::rsx! { <div class={"contentView"}>{render::raw!(html)}</div> })
                    .render_into(writer)?;
            }
            None => {
                if let Some(text) = self.src.content_text() {
                    (render::rsx! { <div class={"contentView"}>{text}</div> })
                        .render_into(writer)?;
                }
            }
        }

        Ok(())
    }
}

#[render::component]
pub fn FlagItem<'a>(flag: &'a RespFlagInfo<'a>, in_community: bool, lang: &'a crate::Translator) {
    let RespFlagDetails::Post { post } = &flag.details;

    render::rsx! {
        <li class={"flagItem"}>
            <div class={"flaggedContent"}>
                <PostItemContent post={post} in_community no_user={false} lang />
            </div>
            {lang.tr(&lang::FLAGGED_BY)}{" "}<UserLink user={Some(&flag.flagger)} lang />
            {
                flag.content.as_ref().map(|content| {
                    render::rsx! {
                        <blockquote>
                            {content.content_text.as_ref()}
                        </blockquote>
                    }
                })
            }
        </li>
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
    let left_links = render::rsx! {
        <>
            <a href={"/all"}>{lang.tr(&lang::ALL)}</a>
            <a href={"/local"}>{lang.tr(&lang::LOCAL)}</a>
            <a href={"/communities"}>{lang.tr(&lang::COMMUNITIES)}</a>
            <a href={"/about"}>{lang.tr(&lang::ABOUT)}</a>
        </>
    };

    render::rsx! {
        <>
            <render::html::HTML5Doctype />
            <html lang={lang.primary_language().to_string()}>
                <head>
                    <meta charset={"utf-8"} />
                    <meta name={"viewport"} content={"width=device-width, initial-scale=1"} />
                    <link rel={"stylesheet"} href={"/static/main.css"} />
                    <title>{title}</title>
                    {head_items}
                </head>
                <body>
                    <header class={"mainHeader"}>
                        <nav aria-label={"Main Navigation"} class={"left"}>
                            <details class={"leftLinksMobile"}>
                                <summary>{hitide_icons::HAMBURGER_MENU.img(lang.tr(&lang::open_menu()).into_owned())}</summary>
                                <div>
                                    {left_links.clone()}
                                </div>
                            </details>
                            <a href={"/"} class={"siteName"}>{"lotide"}</a>
                            <div class={"actionList leftLinks"}>
                                {left_links}
                            </div>
                        </nav>
                        <nav class={"right actionList"}>
                            {
                                base_data.login.as_ref().map(|login| {
                                    render::rsx! {
                                        <>
                                            <a
                                                href={"/notifications"}
                                            >
                                                {
                                                    if login.user.has_unread_notifications {
                                                        hitide_icons::NOTIFICATIONS_SOME.img(lang.tr(&lang::new_notifications()).into_owned())
                                                    } else {
                                                        hitide_icons::NOTIFICATIONS.img(lang.tr(&lang::notifications()).into_owned())
                                                    }
                                                }
                                            </a>
                                            <a href={format!("/users/{}", login.user.id)}>
                                                {hitide_icons::PERSON.img(lang.tr(&lang::profile()).into_owned())}
                                            </a>
                                            <a href={"/moderation"}>
                                                {
                                                    if login.user.has_pending_moderation_actions {
                                                        hitide_icons::MODERATION_SOME.img(lang.tr(&lang::moderation_dashboard_some()).into_owned())
                                                    } else {
                                                        hitide_icons::MODERATION.img(lang.tr(&lang::moderation_dashboard()).into_owned())
                                                    }
                                                }
                                            </a>
                                            {
                                                base_data.is_site_admin().then(|| {
                                                    render::rsx! {
                                                        <>
                                                            <a href={"/administration"}>
                                                                {hitide_icons::ADMINISTRATION.img(lang.tr(&lang::administration()).into_owned())}
                                                            </a>
                                                            <a href={"/flags?to_this_site_admin=true"}>
                                                                {hitide_icons::FLAG.img(lang.tr(&lang::flags()).into_owned())}
                                                            </a>
                                                        </>
                                                    }
                                                })
                                            }
                                            <form method={"POST"} action={"/logout"} class={"inline"}>
                                                <button type={"submit"} class={"iconbutton"}>
                                                    {hitide_icons::LOGOUT.img(lang.tr(&lang::logout()).into_owned())}
                                                </button>
                                            </form>
                                        </>
                                    }
                                })
                            }
                            {
                                if base_data.login.is_none() {
                                    Some(render::rsx! {
                                        <a href={"/login"}>{lang.tr(&lang::LOGIN)}</a>
                                    })
                                } else {
                                    None
                                }
                            }
                        </nav>
                    </header>
                    <main>
                        {children}
                    </main>
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
    render::rsx! {
        <li class={if post.as_ref().sticky { "sticky" } else { "" }}>
            <PostItemContent post in_community no_user lang />
        </li>
    }
}

#[render::component]
pub fn PostItemContent<'a>(
    post: &'a RespPostListPost<'a>,
    in_community: bool,
    no_user: bool,
    lang: &'a crate::Translator,
) {
    let post_href = format!("/posts/{}", post.as_ref().as_ref().id);

    render::rsx! {
        <>
            <div class={"titleLine"}>
                <a href={post_href.clone()}>
                    {post.as_ref().as_ref().sensitive.then(|| hitide_icons::SENSITIVE.img(lang.tr(&lang::SENSITIVE)))}
                    {post.as_ref().as_ref().title.as_ref()}
                </a>
                {
                    post.as_ref().href.as_ref().map(|href| {
                        render::rsx! {
                            <em><a href={href.as_ref()}>{abbreviate_link(href)}{" ↗"}</a></em>
                        }
                    })
                }
            </div>
            <small>
                {lang.tr(&lang::SUBMITTED)}
                {" "}
                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&post.as_ref().created).unwrap()} lang />
                {
                    if no_user {
                        None
                    } else {
                        Some(render::rsx! {
                            <>
                                {" "}{lang.tr(&lang::BY)}{" "}<UserLink lang user={post.as_ref().author.as_ref()} />
                            </>
                        })
                    }
                }
                {
                    if !in_community {
                        Some(render::rsx! {
                            <>{" "}{lang.tr(&lang::TO)}{" "}<CommunityLink community={&post.as_ref().community} /></>
                        })
                    } else {
                        None
                    }
                }
                {" | "}
                <a href={post_href}>{lang.tr(&lang::post_comments_count(post.replies_count_total)).into_owned()}</a>
            </small>
        </>
    }
}

pub struct ThingItem<'a> {
    pub lang: &'a crate::Translator,
    pub thing: &'a RespThingInfo<'a>,
}

impl<'a> render::Render for ThingItem<'a> {
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
        let lang = self.lang;

        match self.thing {
            RespThingInfo::Post(post) => {
                (PostItem { post, in_community: false, no_user: true, lang: self.lang }).render_into(writer)
            },
            RespThingInfo::Comment(comment) => {
                (render::rsx! {
                    <li>
                        <small>
                            <a href={format!("/comments/{}", comment.as_ref().id)}>{lang.tr(&lang::comment())}</a>
                            {" "}{lang.tr(&lang::on())}{" "}<a href={format!("/posts/{}", comment.post.id)}>{comment.post.title.as_ref()}</a>{":"}
                        </small>
                        <ContentView src={comment} />
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
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
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
                                Some(format!(" [{}]", self.lang.tr(&lang::user_bot_tag())))
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

pub fn maybe_fill_value<'a, 'b, M: GetIndex<&'b str, serde_json::Value>>(
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
pub fn MaybeFillCheckbox<'a, M: GetIndex<&'a str, serde_json::Value>>(
    values: &'a Option<&'a M>,
    name: &'a str,
    id: &'a str,
    default: bool,
) {
    let checked = values.map(|x| x.get(name).is_some()).unwrap_or(default);
    log::debug!(
        "MaybeFillCheckbox {} checked={} (values? {})",
        name,
        checked,
        values.is_some()
    );
    if checked {
        render::rsx! {
            <input
                type={"checkbox"}
                name
                id
                checked={""}
            />
        }
    } else {
        render::rsx! {
            <input
                type={"checkbox"}
                name
                id
            />
        }
    }
}

#[render::component]
pub fn MaybeFillOption<'a, M: GetIndex<&'a str, serde_json::Value>, Children: render::Render>(
    values: &'a Option<&'a M>,
    default_value: Option<&'a str>,
    name: &'a str,
    value: &'a str,
    children: Children,
) {
    let selected_value = maybe_fill_value(values, name, default_value);

    SelectOption {
        value,
        selected: selected_value == value,
        children,
    }
}

#[render::component]
pub fn SelectOption<'a, Children: render::Render>(
    value: &'a str,
    selected: bool,
    children: Children,
) {
    if selected {
        render::rsx! {
            <option value={value} selected={""}>{children}</option>
        }
    } else {
        render::rsx! {
            <option value={value}>{children}</option>
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
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
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
                        <div>
                            <a href={format!("/comments/{}", reply.as_ref().id)}>{lang.tr(&lang::comment())}</a>
                            {" "}{lang.tr(&lang::on_your_post())}{" "}<a href={format!("/posts/{}", post.as_ref().as_ref().id)}>{post.as_ref().as_ref().title.as_ref()}</a>{":"}
                        </div>
                        <div class={"body"}>
                            <small>
                                <cite><UserLink lang user={reply.author.as_ref()} /></cite>
                                {" "}
                                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&reply.created).unwrap()} lang />
                            </small>
                            <ContentView src={reply} />
                        </div>
                    </>
                }).render_into(writer)?;
            }
            RespNotificationInfo::PostMention { post } => {
                (render::rsx! {
                    <>
                        <div>
                            {lang.tr(&lang::notification_post_mention())}
                            <div class={"body"}>
                                <PostItemContent post={post} in_community={false} no_user={false} lang={lang} />
                            </div>
                        </div>
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
                        <div>
                            {lang.tr(&lang::reply_to())}
                            {" "}
                            <a href={format!("/comments/{}", comment.as_ref().id)}>{lang.tr(&lang::your_comment())}</a>
                            {" "}{lang.tr(&lang::on())}{" "}<a href={format!("/posts/{}", post.as_ref().as_ref().id)}>{post.as_ref().as_ref().title.as_ref()}</a>
                            {":"}
                        </div>
                        <div class={"body"}>
                            <small>
                                <cite><UserLink lang user={reply.author.as_ref()} /></cite>
                                {" "}
                                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&reply.created).unwrap()} lang />
                            </small>
                            <ContentView src={reply} />
                        </div>
                    </>
                }).render_into(writer)?;
            }
            RespNotificationInfo::CommentMention { comment, post } => {
                (render::rsx! {
                    <>
                        <div>
                            {lang.tr(&lang::notification_comment_mention_1())}{" "}
                            <a href={format!("/comments/{}", comment.as_ref().id)}>{lang.tr(&lang::a_comment())}</a>
                            {" "}{lang.tr(&lang::on())}{" "}
                            <a href={format!("/posts/{}", post.as_ref().as_ref().id)}>
                                {post.as_ref().as_ref().title.as_ref()}
                            </a>
                            {":"}
                            <div class={"body"}>
                                <small>
                                    <cite><UserLink lang user={comment.author.as_ref()} /></cite>
                                    {" "}
                                    <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&comment.created).unwrap()} lang />
                                </small>
                                <ContentView src={comment} />
                            </div>
                        </div>
                    </>
                }).render_into(writer)?;
            }
        }

        write!(writer, "</li>")
    }
}

pub struct SiteModlogEventItem<'a> {
    pub event: &'a RespSiteModlogEvent<'a>,
    pub lang: &'a crate::Translator,
}

impl<'a> render::Render for SiteModlogEventItem<'a> {
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
        let lang = self.lang;
        let event = &self.event;

        write!(writer, "<li>")?;

        (render::rsx! {
            <>
                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&event.time).unwrap()} lang={&lang} />
                {" - "}
            </>
        }).render_into(writer)?;

        match &event.details {
            RespSiteModlogEventDetails::DeletePost { author, community } => {
                (render::rsx! {
                    <>
                        {lang.tr(&lang::MODLOG_EVENT_DELETE_POST_1)}
                        {" "}
                        <UserLink user={Some(author)} lang={&lang} />
                        {" "}
                        {lang.tr(&lang::MODLOG_EVENT_DELETE_POST_2)}
                        {" "}
                        <CommunityLink community />
                    </>
                })
                .render_into(writer)?;
            }
            RespSiteModlogEventDetails::DeleteComment { author, post } => {
                (render::rsx! {
                    <>
                        {lang.tr(&lang::MODLOG_EVENT_DELETE_COMMENT_1)}
                        {" "}
                        <UserLink user={Some(author)} lang={&lang} />
                        {" "}
                        {lang.tr(&lang::MODLOG_EVENT_DELETE_COMMENT_2)}
                        {" "}
                        <a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a>
                    </>
                })
                .render_into(writer)?;
            }
            RespSiteModlogEventDetails::SuspendUser { user } => {
                (render::rsx! {
                    <>
                        {lang.tr(&lang::MODLOG_EVENT_SUSPEND_USER)}
                        {" "}
                        <UserLink user={Some(user)} lang={&lang} />
                    </>
                })
                .render_into(writer)?;
            }
            RespSiteModlogEventDetails::UnsuspendUser { user } => {
                (render::rsx! {
                    <>
                        {lang.tr(&lang::MODLOG_EVENT_UNSUSPEND_USER)}
                        {" "}
                        <UserLink user={Some(user)} lang={&lang} />
                    </>
                })
                .render_into(writer)?;
            }
        }

        write!(writer, "</li>")?;

        Ok(())
    }
}

pub struct PollView<'a> {
    pub poll: &'a RespPollInfo<'a>,
    pub action: String,
    pub lang: &'a crate::Translator,
}
impl<'a> render::Render for PollView<'a> {
    fn render_into<W: std::fmt::Write + ?Sized>(self, writer: &mut W) -> std::fmt::Result {
        let PollView { poll, action, lang } = &self;

        if poll.your_vote.is_some() || poll.is_closed {
            let full_width_votes = f64::from(if poll.multiple {
                poll.options.iter().map(|x| x.votes).max().unwrap_or(0)
            } else {
                poll.options.iter().map(|x| x.votes).sum()
            });

            (render::rsx! {
                <div>
                    <table class={"pollResults"}>
                        {
                            poll.options.iter().map(|option| {
                                let selected = poll.your_vote.as_ref().map(|your_vote| your_vote.options.iter().any(|x| x.id == option.id)).unwrap_or(false);
                                render::rsx! {
                                    <tr class={if selected { "selected" } else { "" }}>
                                        <td class={"count"}>
                                            <div class={"background"} style={format!("width: {}%", f64::from(option.votes) * 100.0 / full_width_votes)}>{""}</div>
                                            {option.votes}
                                        </td>
                                        <td>{option.name.as_ref()}</td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()
                        }
                    </table>
                </div>
            }).render_into(writer)
        } else {
            (render::rsx! {
                <div>
                    <form method={"post"} action={action}>
                        {
                            if poll.multiple {
                                poll.options.iter().map(|option| {
                                    render::rsx! {
                                        <div>
                                            <label>
                                                <input type={"checkbox"} name={option.id.to_string()} />{" "}
                                                {option.name.as_ref()}
                                            </label>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            } else {
                                poll.options.iter().map(|option| {
                                    render::rsx! {
                                        <div>
                                            <label>
                                                <input type={"radio"} name={"choice"} value={option.id.to_string()} />{" "}
                                                {option.name.as_ref()}
                                            </label>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            }
                        }
                        <input type={"submit"} value={lang.tr(&lang::POLL_SUBMIT)} />
                    </form>
                </div>
            }).render_into(writer)
        }
    }
}

pub trait IconExt {
    fn img<'a>(&self, alt: impl Into<Cow<'a, str>>) -> render::SimpleElement<'a, ()>;
    fn img_aria_hidden(&self) -> render::SimpleElement<'static, ()>;
}

impl IconExt for hitide_icons::Icon {
    fn img<'a>(&self, alt: impl Into<Cow<'a, str>>) -> render::SimpleElement<'a, ()> {
        render::rsx! {
            <img src={format!("/static/{}", self.path)} class={if self.dark_invert { "icon darkInvert" } else { "icon" }} alt={alt.into()} />
        }
    }

    fn img_aria_hidden(&self) -> render::SimpleElement<'static, ()> {
        render::rsx! {
            <img src={format!("/static/{}", self.path)} class={if self.dark_invert { "icon darkInvert" } else { "icon" }} aria-hidden={"true"} />
        }
    }
}
