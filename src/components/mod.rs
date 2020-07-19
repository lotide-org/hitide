use std::borrow::{Borrow, Cow};
use std::collections::HashMap;

use crate::resp_types::{
    RespMinimalAuthorInfo, RespMinimalCommunityInfo, RespPostCommentInfo, RespPostInfo,
    RespPostListPost, RespThingComment, RespThingInfo,
};
use crate::util::{abbreviate_link, author_is_me};
use crate::PageBaseData;

#[render::component]
pub fn Comment<'comment, 'base_data>(
    comment: &'comment RespPostCommentInfo<'comment>,
    base_data: &'base_data PageBaseData,
) {
    render::rsx! {
        <li>
            <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
            <Content src={comment} />
            <div class={"actionList"}>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <>
                                {
                                    if comment.your_vote.is_some() {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/unlike", comment.id)}>
                                                <button type={"submit"}>{"Unlike"}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/like", comment.id)}>
                                                <button type={"submit"}>{"Like"}</button>
                                            </form>
                                        }
                                    }
                                }
                                <a href={format!("/comments/{}", comment.id)}>{"reply"}</a>
                            </>
                        })
                    } else {
                        None
                    }
                }
                {
                    if author_is_me(&comment.author, &base_data.login) {
                        Some(render::rsx! {
                            <a href={format!("/comments/{}/delete", comment.id)}>{"delete"}</a>
                        })
                    } else {
                        None
                    }
                }
            </div>

            {
                if let Some(replies) = &comment.replies {
                        Some(render::rsx! {
                            <ul>
                                {
                                    replies.iter().map(|reply| {
                                        render::rsx! {
                                            <Comment comment={reply} base_data />
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                }
                            </ul>
                        })
                } else {
                    None
                }
            }
            {
                if comment.replies.is_none() && comment.has_replies {
                    Some(render::rsx! {
                        <ul><li><a href={format!("/comments/{}", comment.id)}>{"-> View More Comments"}</a></li></ul>
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

impl<'a> HavingContent for RespPostCommentInfo<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text.as_deref()
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html.as_deref()
    }
}

impl<'a> HavingContent for RespThingComment<'a> {
    fn content_text(&self) -> Option<&str> {
        self.content_text.as_deref()
    }
    fn content_html(&self) -> Option<&str> {
        self.content_html.as_deref()
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

pub struct Content<'a, T: HavingContent + 'a> {
    pub src: &'a T,
}

impl<'a, T: HavingContent + 'a> render::Render for Content<'a, T> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        match self.src.content_html() {
            Some(html) => {
                let cleaned = ammonia::clean(&html);
                writer.write_str("<p>")?;
                render::raw!(cleaned.as_ref()).render_into(writer)?;
                writer.write_str("</p>")?;
            }
            None => match self.src.content_text() {
                Some(text) => {
                    writer.write_str("<p>")?;
                    text.render_into(writer)?;
                    writer.write_str("</p>")?;
                }
                None => {}
            },
        }

        Ok(())
    }
}

#[render::component]
pub fn HTPage<'a, Children: render::Render>(
    base_data: &'a PageBaseData,
    title: &'a str,
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
                </head>
                <body>
                    <header class={"mainHeader"}>
                        <div class={"left actionList"}>
                            <a href={"/"} class={"siteName"}>{"lotide"}</a>
                            <a href={"/all"}>{"All"}</a>
                            <a href={"/communities"}>{"Communities"}</a>
                            <a href={"/about"}>{"About"}</a>
                        </div>
                        <div class={"right actionList"}>
                            {
                                match &base_data.login {
                                    Some(login) => Some(render::rsx! {
                                        <a href={format!("/users/{}", login.user.id)}>{"👤︎"}</a>
                                    }),
                                    None => {
                                        Some(render::rsx! {
                                            <a href={"/login"}>{"Login"}</a>
                                        })
                                    }
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
pub fn PostItem<'post>(post: &'post RespPostListPost<'post>, in_community: bool, no_user: bool) {
    render::rsx! {
        <li>
            <a href={format!("/posts/{}", post.as_ref().id)}>
                {post.as_ref().title.as_ref()}
            </a>
            {
                if let Some(href) = &post.href {
                    Some(render::rsx! {
                        <>
                            {" "}
                            <em><a href={href.as_ref()}>{abbreviate_link(&href)}{" ↗"}</a></em>
                        </>
                    })
                } else {
                    None
                }
            }
            <br />
            {"Submitted"}
            {
                if no_user {
                    None
                } else {
                    Some(render::rsx! {
                        <>
                            {" by "}<UserLink user={post.author.as_ref()} />
                        </>
                    })
                }
            }
            {
                if !in_community {
                    Some(render::rsx! {
                        <>{" to "}<CommunityLink community={&post.community} /></>
                    })
                } else {
                    None
                }
            }
        </li>
    }
}

pub struct ThingItem<'a> {
    pub thing: &'a RespThingInfo<'a>,
}

impl<'a> render::Render for ThingItem<'a> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        match self.thing {
            RespThingInfo::Post(post) => {
                (PostItem { post, in_community: false, no_user: true }).render_into(writer)
            },
            RespThingInfo::Comment(comment) => {
                (render::rsx! {
                    <li>
                        <small>
                            <a href={format!("/comments/{}", comment.id)}>{"Comment"}</a>
                            {" on "}<a href={format!("/posts/{}", comment.post.id)}>{comment.post.title.as_ref()}</a>{":"}
                        </small>
                        <Content src={comment} />
                    </li>
                }).render_into(writer)
            }
        }
    }
}

pub struct UserLink<'user> {
    pub user: Option<&'user RespMinimalAuthorInfo<'user>>,
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
                    </a>
                })
                .render_into(writer)
            }
        }
    }
}

pub trait GetIndex<K, V> {
    fn get<'a>(&'a self, key: K) -> Option<&'a V>;
}

impl<K: Borrow<Q> + Eq + std::hash::Hash, V, Q: ?Sized + Eq + std::hash::Hash> GetIndex<&Q, V>
    for HashMap<K, V>
{
    fn get<'a>(&'a self, key: &Q) -> Option<&'a V> {
        HashMap::get(self, key)
    }
}

impl<I: serde_json::value::Index> GetIndex<I, serde_json::Value> for serde_json::Value {
    fn get<'a>(&'a self, key: I) -> Option<&'a serde_json::Value> {
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
) {
    let value = maybe_fill_value(values, name, None);
    if required {
        render::rsx! {
            <input
                r#type
                name
                value
                required={""}
            />
        }
    } else {
        render::rsx! {
            <input
                r#type
                name
                value
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
