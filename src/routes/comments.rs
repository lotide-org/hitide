use super::{
    default_comments_sort, fetch_base_data, for_client, get_cookie_map_for_headers,
    get_cookie_map_for_req, html_response, res_to_error, CookieMap, JustStringID, ReturnToParams,
};
use crate::components::{
    Comment, ContentView, HTPage, IconExt, MaybeFillCheckbox, MaybeFillTextArea, UserLink,
};
use crate::lang;
use crate::resp_types::{JustContentHTML, JustID, RespCommentInfo, RespList, RespPostCommentInfo};
use crate::util::{abbreviate_link, author_is_me};
use serde_derive::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

async fn page_comment(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_comment_inner(
        comment_id,
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

async fn page_comment_inner(
    comment_id: i64,
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
        #[serde(default = "default_comments_sort")]
        sort: crate::SortType,
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(query.unwrap_or(""))?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let info_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/comments/{}{}",
                    ctx.backend_host,
                    comment_id,
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
    let info_api_res = hyper::body::to_bytes(info_api_res.into_body()).await?;
    let comment: RespCommentInfo<'_> = serde_json::from_slice(&info_api_res)?;

    #[derive(Serialize)]
    struct RepliesListQuery<'a> {
        include_your: Option<bool>,
        sort: Option<crate::SortType>,
        page: Option<&'a str>,
    }
    let replies_req_query = RepliesListQuery {
        include_your: if base_data.login.is_some() {
            Some(true)
        } else {
            None
        },
        sort: Some(query.sort),
        page: query.page.as_deref(),
    };
    let replies_req_query = serde_urlencoded::to_string(&replies_req_query)?;

    let replies_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/comments/{}/replies?{}",
                    ctx.backend_host, comment_id, replies_req_query,
                ))
                .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;
    let replies_api_res = hyper::body::to_bytes(replies_api_res.into_body()).await?;
    let replies: RespList<RespPostCommentInfo<'_>> = serde_json::from_slice(&replies_api_res)?;

    let title = lang.tr(&lang::COMMENT);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            {
                comment.post.as_ref().map(|post| {
                    render::rsx! {
                        <p>
                            {lang.tr(&lang::TO_POST)}{" "}<a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a>
                        </p>
                    }
                })
            }
            <p>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <>
                                {
                                    if comment.as_ref().your_vote.is_some() {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/unlike", comment.as_ref().as_ref().id)}>
                                                <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTED.img(lang.tr(&lang::remove_upvote()).into_owned())}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/like", comment.as_ref().as_ref().id)}>
                                                <button class={"iconbutton"} type={"submit"}>{hitide_icons::UPVOTE.img(lang.tr(&lang::upvote()).into_owned())}</button>
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
                {
                    comment.parent.as_ref().map(|parent| {
                        render::rsx! {
                            <div>
                                <small><a href={format!("/comments/{}", parent.id)}>{"<- "}{lang.tr(&lang::TO_PARENT)}</a></small>
                            </div>
                        }
                    })
                }
                <small><cite><UserLink lang={&lang} user={comment.as_ref().author.as_ref()} /></cite>{":"}</small>
                <ContentView src={&comment} />
                {
                    comment.as_ref().attachments.iter().map(|attachment| {
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
            </p>
            <div class={"actionList"}>
                {
                    if !comment.as_ref().local {
                        if let Some(remote_url) = &comment.as_ref().as_ref().remote_url {
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
                    if author_is_me(&comment.as_ref().author, &base_data.login) {
                        Some(render::rsx! {
                            <a href={format!("/comments/{}/delete", comment.as_ref().as_ref().id)}>{lang.tr(&lang::DELETE)}</a>
                        })
                    } else {
                        None
                    }
                }
            </div>
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
                        <form method={"POST"} action={format!("/comments/{}/submit_reply", comment.as_ref().as_ref().id)} enctype={"multipart/form-data"}>
                            <div>
                                <MaybeFillTextArea values={&prev_values} name={"content_markdown"} default_value={None} />
                            </div>
                            <div>
                                <label>
                                    {lang.tr(&lang::COMMENT_REPLY_IMAGE_PROMPT)}
                                    {" "}
                                    <input type={"file"} accept={"image/*"} name={"attachment_media"} />
                                </label>
                            </div>
                            <div>
                                <label>
                                    <MaybeFillCheckbox values={&prev_values} name={"sensitive"} id={"sensitive"} default={comment.as_ref().as_ref().sensitive} />
                                    {" "}
                                    {lang.tr(&lang::SENSITIVE)}
                                </label>
                            </div>
                            <button r#type={"submit"}>{lang.tr(&lang::REPLY_SUBMIT)}</button>
                            <button r#type={"submit"} name={"preview"}>{lang.tr(&lang::PREVIEW)}</button>
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
                <span>{lang.tr(&lang::sort())}</span>
                {
                    crate::SortType::VALUES.iter()
                        .map(|value| {
                            let name = lang.tr(&value.lang_key()).into_owned();
                            if query.sort == *value {
                                render::rsx! { <span>{name}</span> }
                            } else {
                                render::rsx! { <a href={format!("/comments/{}?sort={}", comment_id, value.as_str())}>{name}</a> }
                            }
                        })
                        .collect::<Vec<_>>()
                }
            </div>
            <ul class={"commentList topLevel"}>
                {
                    replies.items.iter().map(|reply| {
                        render::rsx! {
                            <Comment comment={reply} sort={query.sort} root_sensitive={comment.as_ref().as_ref().sensitive} base_data={&base_data} lang={&lang} />
                        }
                    }).collect::<Vec<_>>()
                }
            </ul>
            {
                replies.next_page.as_ref().map(|next_page| {
                    render::rsx! {
                        <a href={format!("/comments/{}?sort={}&page={}", comment.base.base.id, query.sort.as_str(), next_page)}>{"-> "}{lang.tr(&lang::VIEW_MORE_COMMENTS)}</a>
                    }
                })
            }
        </HTPage>
    }))
}

async fn page_comment_delete(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_comment_delete_inner(comment_id, ctx, req.headers(), &cookies, None).await
}

async fn page_comment_delete_inner(
    comment_id: i64,
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let referer = headers
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/comments/{}",
                    ctx.backend_host, comment_id
                ))
                .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let comment: RespPostCommentInfo<'_> = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::COMMENT_DELETE_TITLE);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <p>
                <small><cite><UserLink lang={&lang} user={comment.author.as_ref()} /></cite>{":"}</small>
                <br />
                <ContentView src={&comment} />
            </p>
            <div id={"delete"}>
                <h2>{lang.tr(&lang::comment_delete_question())}</h2>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                <form method={"POST"} action={format!("/comments/{}/delete/confirm", comment.as_ref().id)}>
                    {
                        referer.map(|referer| {
                            render::rsx! {
                                <input type={"hidden"} name={"return_to"} value={referer} />
                            }
                        })
                    }
                    <a href={format!("/comments/{}/", comment.as_ref().id)}>{lang.tr(&lang::no_cancel())}</a>
                    {" "}
                    <button r#type={"submit"}>{lang.tr(&lang::delete_yes())}</button>
                </form>
            </div>
        </HTPage>
    }))
}

async fn handler_comment_delete_confirm(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: ReturnToParams = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/comments/{}",
                    ctx.backend_host, comment_id,
                ))
                .body("".into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(
                hyper::header::LOCATION,
                if let Some(return_to) = &body.return_to {
                    &return_to
                } else {
                    "/"
                },
            )
            .body("Successfully deleted.".into())?),
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_comment_delete_inner(comment_id, ctx, &req_parts.headers, &cookies, Some(message))
                .await
        }
        Err(other) => Err(other),
    }
}

async fn handler_comment_like(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let referer = req
        .headers()
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::put(format!(
                    "{}/api/unstable/comments/{}/your_vote",
                    ctx.backend_host, comment_id
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
        .header(
            hyper::header::LOCATION,
            (if let Some(referer) = referer {
                Cow::Borrowed(referer)
            } else {
                format!("/comments/{}", comment_id).into()
            })
            .as_ref(),
        )
        .body("Successfully liked.".into())?)
}

async fn handler_comment_unlike(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let referer = req
        .headers()
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/comments/{}/your_vote",
                    ctx.backend_host, comment_id
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
        .header(
            hyper::header::LOCATION,
            (if let Some(referer) = referer {
                Cow::Borrowed(referer)
            } else {
                format!("/comments/{}", comment_id).into()
            })
            .as_ref(),
        )
        .body("Successfully unliked.".into())?)
}

async fn handler_comment_submit_reply(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

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
                            lang.tr(&lang::comment_reply_attachment_missing_content_type())
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
                let name = field.name().unwrap();
                if name == "href" && body_values.contains_key("href") && body_values["href"] != "" {
                    error = Some(lang.tr(&lang::post_new_href_conflict()).into_owned());
                } else {
                    let name = name.to_owned();
                    let value = field.text().await?;
                    body_values.insert(name.into(), value.into());
                }
            }
        }

        if let Some(error) = error {
            return page_comment_inner(
                comment_id,
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

                page_comment_inner(
                    comment_id,
                    &req_parts.headers,
                    req_parts.uri.query(),
                    &cookies,
                    ctx,
                    None,
                    Some(&body_values),
                    Some(&preview_res.content_html),
                )
                .await
            }
            Err(crate::Error::RemoteError((_, message))) => {
                page_comment_inner(
                    comment_id,
                    &req_parts.headers,
                    req_parts.uri.query(),
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

    body_values.insert(
        "sensitive".into(),
        body_values.contains_key("sensitive").into(),
    );

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/comments/{}/replies",
                    ctx.backend_host, comment_id
                ))
                .body(serde_json::to_vec(&body_values)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: JustID = serde_json::from_slice(&api_res)?;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(
                    hyper::header::LOCATION,
                    format!("/comments/{}#comment{}", comment_id, api_res.id),
                )
                .body("Successfully posted.".into())?)
        }
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_comment_inner(
                comment_id,
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
    }
}

pub fn route_comments() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_child_parse::<i64, _>(
        crate::RouteNode::new()
            .with_handler_async(hyper::Method::GET, page_comment)
            .with_child(
                "delete",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::GET, page_comment_delete)
                    .with_child(
                        "confirm",
                        crate::RouteNode::new().with_handler_async(
                            hyper::Method::POST,
                            handler_comment_delete_confirm,
                        ),
                    ),
            )
            .with_child(
                "like",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::POST, handler_comment_like),
            )
            .with_child(
                "unlike",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::POST, handler_comment_unlike),
            )
            .with_child(
                "submit_reply",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::POST, handler_comment_submit_reply),
            ),
    )
}
