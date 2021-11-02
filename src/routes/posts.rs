use super::JustStringID;
use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap,
};
use crate::components::{
    Comment, CommunityLink, ContentView, HTPage, IconExt, MaybeFillTextArea, TimeAgo, UserLink,
};
use crate::resp_types::{
    JustContentHTML, JustUser, RespCommunityInfoMaybeYour, RespList, RespPostCommentInfo,
    RespPostInfo,
};
use crate::util::author_is_me;
use serde_derive::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

async fn page_post(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_post_inner(
        post_id,
        req.headers(),
        req.uri().query(),
        &cookies,
        ctx,
        None,
        None,
        None,
    )
    .await
}

async fn page_post_inner(
    post_id: i64,
    headers: &hyper::header::HeaderMap,
    query: Option<&str>,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error: Option<String>,
    prev_values: Option<&HashMap<Cow<'_, str>, serde_json::Value>>,
    display_preview: Option<&str>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);

    #[derive(Deserialize)]
    struct Query<'a> {
        #[serde(default = "super::default_comments_sort")]
        sort: crate::SortType,
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(query.unwrap_or(""))?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}{}",
                    ctx.backend_host,
                    post_id,
                    if base_data.login.is_some() {
                        "?include_your=true"
                    } else {
                        ""
                    },
                ))
                .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let post: RespPostInfo = serde_json::from_slice(&api_res)?;

    #[derive(Serialize)]
    struct RepliesListQuery<'a> {
        include_your: Option<bool>,
        sort: Option<crate::SortType>,
        page: Option<&'a str>,
    }
    let api_req_query = RepliesListQuery {
        include_your: if base_data.login.is_some() {
            Some(true)
        } else {
            None
        },
        sort: Some(query.sort),
        page: query.page.as_deref(),
    };
    let api_req_query = serde_urlencoded::to_string(&api_req_query)?;

    let replies_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}/replies?{}",
                    ctx.backend_host, post_id, api_req_query,
                ))
                .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;
    let replies_api_res = hyper::body::to_bytes(replies_api_res.into_body()).await?;
    let replies: RespList<RespPostCommentInfo> = serde_json::from_slice(&replies_api_res)?;

    let is_community_moderator = if base_data.login.is_some() {
        let api_res = res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::get(format!(
                        "{}/api/unstable/communities/{}?include_your=true",
                        ctx.backend_host,
                        post.as_ref().community.id,
                    ))
                    .body(Default::default())?,
                    headers,
                    cookies,
                )?)
                .await?,
        )
        .await?;
        let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

        let info: RespCommunityInfoMaybeYour = serde_json::from_slice(&api_res)?;
        info.you_are_moderator.unwrap()
    } else {
        false
    };

    let title = post.as_ref().as_ref().title.as_ref();

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={title}>
            {
                if post.approved {
                    None
                } else {
                    Some(render::rsx! { <div class={"infoBox"}>{lang.tr("post_not_approved", None)}</div> })
                }
            }
            <h1>{title}</h1>
            <div>
                {
                    if base_data.login.is_some() {
                        Some(if post.your_vote.is_some() {
                            render::rsx! {
                                <>
                                    <form method={"POST"} action={format!("/posts/{}/unlike", post_id)} class={"inline"}>
                                        <button type={"submit"} class={"iconbutton"}>{hitide_icons::UPVOTED.img()}</button>
                                    </form>
                                    {" "}
                                </>
                            }
                        } else {
                            render::rsx! {
                                <>
                                    <form method={"POST"} action={format!("/posts/{}/like", post_id)} class={"inline"}>
                                        <button type={"submit"} class={"iconbutton"}>{hitide_icons::UPVOTE.img()}</button>
                                    </form>
                                    {" "}
                                </>
                            }
                        })
                    } else {
                        None
                    }
                }
                <a href={format!("/posts/{}/likes", post_id)}>
                    <em>{lang.tr("score", Some(&fluent::fluent_args!["score" => post.score]))}</em>
                </a>
                {" "}
                {
                    if is_community_moderator {
                        Some(render::rsx! {
                            <>
                                {
                                    if post.approved {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/unapprove", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr("post_approve_undo", None)}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/approve", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr("post_approve", None)}</button>
                                            </form>
                                        }
                                    }
                                }
                                {
                                    if post.as_ref().sticky {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/make_unsticky", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr("post_make_not_sticky", None)}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/make_sticky", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr("post_make_sticky", None)}</button>
                                            </form>
                                        }
                                    }
                                }
                            </>
                        })
                    } else {
                        None
                    }
                }
            </div>
            <br />
            <p>
                {lang.tr("submitted", None)}
                {" "}<TimeAgo since={chrono::DateTime::parse_from_rfc3339(&post.as_ref().created)?} lang={&lang} />
                {" "}{lang.tr("by", None)}{" "}<UserLink lang={&lang} user={post.as_ref().author.as_ref()} />
                {" "}{lang.tr("to", None)}{" "}<CommunityLink community={&post.as_ref().community} />
            </p>
            {
                post.as_ref().href.as_ref().map(|href| {
                    render::rsx! {
                        <p><a href={href.as_ref()}>{href.as_ref()}</a></p>
                    }
                })
            }
            <div class={"postContent"}>
                <ContentView src={&post} />
            </div>
            {
                if author_is_me(&post.as_ref().author, &base_data.login) || (post.local && base_data.is_site_admin()) {
                    Some(render::rsx! {
                        <p>
                            <a href={format!("/posts/{}/delete", post_id)}>{lang.tr("delete", None)}</a>
                        </p>
                    })
                } else {
                    None
                }
            }
            <div>
                <h2>{lang.tr("comments", None)}</h2>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <form method={"POST"} action={format!("/posts/{}/submit_reply", post.as_ref().as_ref().id)} enctype={"multipart/form-data"}>
                                <div>
                                    <MaybeFillTextArea name={"content_markdown"} values={&prev_values} default_value={None} />
                                </div>
                                <div>
                                    <label>
                                        {lang.tr("comment_reply_image_prompt", None)}
                                        {" "}
                                        <input type={"file"} accept={"image/*"} name={"attachment_media"} />
                                    </label>
                                </div>
                                <button r#type={"submit"}>{lang.tr("comment_submit", None)}</button>
                                <button r#type={"submit"} name={"preview"}>{lang.tr("preview", None)}</button>
                            </form>
                        })
                    } else {
                        None
                    }
                }
                {
                    display_preview.map(|html| {
                        render::rsx! {
                            <div class={"preview"}>{render::raw!(html)}</div>
                        }
                    })
                }
                <div class={"sortOptions"}>
                    <span>{lang.tr("sort", None)}</span>
                    {
                        crate::SortType::VALUES.iter()
                            .map(|value| {
                                let name = lang.tr(value.lang_key(), None);
                                if query.sort == *value {
                                    render::rsx! { <span>{name}</span> }
                                } else {
                                    render::rsx! { <a href={format!("/posts/{}?sort={}", post_id, value.as_str())}>{name}</a> }
                                }
                            })
                            .collect::<Vec<_>>()
                    }
                </div>
                <ul class={"commentList topLevel"}>
                    {
                        replies.items.iter().map(|comment| {
                            render::rsx! {
                                <Comment comment={comment} sort={query.sort} base_data={&base_data} lang={&lang} />
                            }
                        }).collect::<Vec<_>>()
                    }
                </ul>
                {
                    replies.next_page.map(|next_page| {
                        render::rsx! {
                            <a href={format!("/posts/{}?page={}", post_id, next_page)}>{"-> "}{lang.tr("view_more_comments", None)}</a>
                        }
                    })
                }
            </div>
        </HTPage>
    }))
}

async fn page_post_delete(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

    let post: RespPostInfo = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr("post_delete_title", None)}>
            <h1>{post.as_ref().as_ref().title.as_ref()}</h1>
            <h2>{lang.tr("post_delete_question", None)}</h2>
            <form method={"POST"} action={format!("/posts/{}/delete/confirm", post.as_ref().as_ref().id)}>
                <a href={format!("/posts/{}/", post.as_ref().as_ref().id)}>{lang.tr("no_cancel", None)}</a>
                {" "}
                <button r#type={"submit"}>{lang.tr("delete_yes", None)}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_post_delete_confirm(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/posts/{}",
                    ctx.backend_host, post_id,
                ))
                .body("".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, "/")
        .body("Successfully deleted.".into())?)
}

async fn handler_post_like(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::put(format!(
                    "{}/api/unstable/posts/{}/your_vote",
                    ctx.backend_host, post_id
                ))
                .body("{}".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully liked.".into())?)
}

async fn page_post_likes(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}/votes",
                    ctx.backend_host, post_id,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespList<JustUser> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr("likes", None)}>
        {
            if api_res.items.is_empty() {
                Some(render::rsx! { <p>{lang.tr("post_likes_nothing", None)}</p> })
            } else {
                None
            }
        }
        {
            if api_res.items.is_empty() {
                None
            } else {
                Some(render::rsx! {
                    <>
                        <p>{lang.tr("liked_by", None)}</p>
                        <ul>
                            {
                                api_res.items.iter().map(|like| {
                                    render::rsx! { <li><UserLink lang={&lang} user={Some(&like.user)} /></li> }
                                })
                                .collect::<Vec<_>>()
                            }
                            {
                                if api_res.next_page.is_some() {
                                    Some(render::rsx! { <li>{lang.tr("and_more", None)}</li> })
                                } else {
                                    None
                                }
                            }
                        </ul>
                    </>
                })
            }
        }
        </HTPage>
    }))
}

async fn handler_post_unlike(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/posts/{}/your_vote",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully unliked.".into())?)
}

async fn handler_post_submit_reply(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let (req_parts, body) = req.into_parts();
    let lang = crate::get_lang_for_headers(&req_parts.headers);
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let content_type = req_parts
        .headers
        .get(hyper::header::CONTENT_TYPE)
        .ok_or_else(|| {
            crate::Error::InternalStr("missing content-type header in form submission".to_owned())
        })?;
    let content_type = std::str::from_utf8(content_type.as_ref())?;
    let boundary = multer::parse_boundary(&content_type)?;

    let mut multipart = multer::Multipart::new(body, boundary);

    let mut body_values: HashMap<Cow<'_, str>, serde_json::Value> = HashMap::new();

    {
        let mut error = None;

        loop {
            let field = multipart.next_field().await?;
            let field = match field {
                None => break,
                Some(field) => field,
            };

            if field.name().is_none() {
                continue;
            }

            if field.name().unwrap() == "attachment_media" {
                use futures_util::StreamExt;
                let mut stream = field.peekable();

                let first_chunk = std::pin::Pin::new(&mut stream).peek().await;
                let is_empty = match first_chunk {
                    None => true,
                    Some(Ok(chunk)) => chunk.is_empty(),
                    Some(Err(err)) => {
                        return Err(crate::Error::InternalStr(format!(
                            "failed parsing form: {:?}",
                            err
                        )));
                    }
                };
                if is_empty {
                    continue;
                }

                match stream.get_ref().content_type() {
                    None => {
                        error = Some(
                            lang.tr("comment_reply_attachment_missing_content_type", None)
                                .into_owned(),
                        );
                    }
                    Some(mime) => {
                        let res = res_to_error(
                            ctx.http_client
                                .request(for_client(
                                    hyper::Request::post(format!(
                                        "{}/api/unstable/media",
                                        ctx.backend_host,
                                    ))
                                    .header(hyper::header::CONTENT_TYPE, mime.as_ref())
                                    .body(hyper::Body::wrap_stream(stream))?,
                                    &req_parts.headers,
                                    &cookies,
                                )?)
                                .await?,
                        )
                        .await;

                        match res {
                            Err(crate::Error::RemoteError((_, message))) => {
                                error = Some(message);
                            }
                            Err(other) => {
                                return Err(other);
                            }
                            Ok(res) => {
                                let res = hyper::body::to_bytes(res.into_body()).await?;
                                let res: JustStringID = serde_json::from_slice(&res)?;

                                body_values.insert(
                                    "attachment".into(),
                                    format!("local-media://{}", res.id).into(),
                                );
                            }
                        }

                        log::debug!("finished media upload");
                    }
                }
            } else {
                let name = field.name().unwrap().to_owned();
                let value = field.text().await?;
                body_values.insert(name.into(), value.into());
            }
        }

        if let Some(error) = error {
            return page_post_inner(
                post_id,
                &req_parts.headers,
                None,
                &cookies,
                ctx,
                Some(error),
                Some(&body_values),
                None,
            )
            .await;
        }
    }

    if body_values.contains_key("preview") {
        let md = body_values
            .get("content_markdown")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let preview_res = res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::post(format!(
                        "{}/api/unstable/misc/render_markdown",
                        ctx.backend_host
                    ))
                    .body(
                        serde_json::to_vec(&serde_json::json!({ "content_markdown": md }))?.into(),
                    )?,
                    &req_parts.headers,
                    &cookies,
                )?)
                .await?,
        )
        .await;
        return match preview_res {
            Ok(preview_res) => {
                let preview_res = hyper::body::to_bytes(preview_res.into_body()).await?;
                let preview_res: JustContentHTML = serde_json::from_slice(&preview_res)?;

                page_post_inner(
                    post_id,
                    &req_parts.headers,
                    None,
                    &cookies,
                    ctx,
                    None,
                    Some(&body_values),
                    Some(&preview_res.content_html),
                )
                .await
            }
            Err(crate::Error::RemoteError((_, message))) => {
                page_post_inner(
                    post_id,
                    &req_parts.headers,
                    None,
                    &cookies,
                    ctx,
                    Some(message),
                    Some(&body_values),
                    None,
                )
                .await
            }
            Err(other) => Err(other),
        };
    }

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/replies",
                    ctx.backend_host, post_id
                ))
                .body(serde_json::to_vec(&body_values)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Err(crate::Error::RemoteError((_, message))) => {
            page_post_inner(
                post_id,
                &req_parts.headers,
                None,
                &cookies,
                ctx,
                Some(message),
                Some(&body_values),
                None,
            )
            .await
        }
        Err(other) => Err(other),
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
            .body("Successfully posted.".into())?),
    }
}

pub fn route_posts() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_child_parse::<i64, _>(
        crate::RouteNode::new()
            .with_handler_async("GET", page_post)
            .with_child(
                "delete",
                crate::RouteNode::new()
                    .with_handler_async("GET", page_post_delete)
                    .with_child(
                        "confirm",
                        crate::RouteNode::new()
                            .with_handler_async("POST", handler_post_delete_confirm),
                    ),
            )
            .with_child(
                "like",
                crate::RouteNode::new().with_handler_async("POST", handler_post_like),
            )
            .with_child(
                "likes",
                crate::RouteNode::new().with_handler_async("GET", page_post_likes),
            )
            .with_child(
                "unlike",
                crate::RouteNode::new().with_handler_async("POST", handler_post_unlike),
            )
            .with_child(
                "submit_reply",
                crate::RouteNode::new().with_handler_async("POST", handler_post_submit_reply),
            ),
    )
}
