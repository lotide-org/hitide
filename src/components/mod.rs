use std::borrow::Cow;

use crate::resp_types::{
    RespMinimalAuthorInfo, RespMinimalCommunityInfo, RespPostCommentInfo, RespPostListPost,
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
                                <form method={"POST"} action={format!("/comments/{}/like", comment.id)} style={"display: inline"}>
                                    <button r#type={"submit"}>{"Like"}</button>
                                </form>
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
                match &comment.replies {
                    Some(replies) => {
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
                    },
                    None => None,
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

impl<'a> HavingContent for RespPostListPost<'a> {
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
pub fn HTPage<'base_data, Children: render::Render>(
    base_data: &'base_data PageBaseData,
    children: Children,
) {
    render::rsx! {
        <>
            <render::html::HTML5Doctype />
            <html>
                <head>
                    <meta charset={"utf-8"} />
                    <link rel={"stylesheet"} href={"/static/main.css"} />
                </head>
                <body>
                    <header class={"mainHeader"}>
                        <div class={"left actionList"}>
                            <a href={"/"} class={"siteName"}>{"lotide"}</a>
                            <a href={"/communities"}>{"Communities"}</a>
                            <a href={"/about"}>{"About"}</a>
                        </div>
                        <div class={"right actionList"}>
                            {
                                match base_data.login {
                                    Some(_) => None,
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
pub fn PostItem<'post>(post: &'post RespPostListPost<'post>, in_community: bool) {
    render::rsx! {
        <li>
            <a href={format!("/posts/{}", post.id)}>
                {post.title.as_ref()}
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
            {"Submitted by "}<UserLink user={post.author.as_ref()} />
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

fn maybe_fill_value<'a>(values: &'a Option<&'a serde_json::Value>, name: &str) -> &'a str {
    values
        .and_then(|values| values.get(name))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
}

#[render::component]
pub fn MaybeFillInput<'a>(
    values: &'a Option<&'a serde_json::Value>,
    r#type: &'a str,
    name: &'a str,
    required: bool,
) {
    render::rsx! {
        <input
            r#type
            name
            value={maybe_fill_value(values, name)}
            required={if required { "true" } else { "false" }}
        />
    }
}

#[render::component]
pub fn MaybeFillTextArea<'a>(values: &'a Option<&'a serde_json::Value>, name: &'a str) {
    render::rsx! {
        <textarea name>
            {maybe_fill_value(values, name)}
        </textarea>
    }
}
