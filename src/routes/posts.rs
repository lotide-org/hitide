use super::JustStringID;
use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap,
};
use crate::components::{
    Comment, CommunityLink, ContentView, HTPage, IconExt, MaybeFillCheckbox, MaybeFillTextArea,
    PollView, TimeAgo, UserLink,
};
use crate::lang;
use crate::query_types::PollVoteBody;
use crate::resp_types::{
    JustContentHTML, JustID, JustUser, RespCommunityInfoMaybeYour, RespList, RespPostCommentInfo,
    RespPostInfo,
};
use crate::util::author_is_me;
use render::Render;
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
    display_error_comments: Option<String>,
    display_error_poll: Option<String>,
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

    let is_community_moderator = !post.as_ref().community.deleted
        && if base_data.login.is_some() {
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

    let created = chrono::DateTime::parse_from_rfc3339(&post.as_ref().created)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={title}>
            {
                if post.approved {
                    None
                } else {
                    Some(if post.rejected {
                        render::rsx! { <div class={"errorBox"}>{lang.tr(&lang::post_rejected()).into_owned()}</div> }
                    } else {
                        render::rsx! { <div class={"infoBox"}>{lang.tr(&lang::post_not_approved()).into_owned()}</div> }
                    })
                }
            }
            <h1 class={"bigPostTitle"}>
                {post.as_ref().as_ref().sensitive.then(|| hitide_icons::SENSITIVE.img(lang.tr(&lang::SENSITIVE)))}
                {title}
            </h1>
            <div>
                {
                    if base_data.login.is_some() {
                        Some(if post.your_vote.is_some() {
                            render::rsx! {
                                <>
                                    <form method={"POST"} action={format!("/posts/{}/unlike", post_id)} class={"inline"}>
                                        <button type={"submit"} class={"iconbutton"}>{hitide_icons::UPVOTED.img(lang.tr(&lang::remove_upvote()).into_owned())}</button>
                                    </form>
                                    {" "}
                                </>
                            }
                        } else {
                            render::rsx! {
                                <>
                                    <form method={"POST"} action={format!("/posts/{}/like", post_id)} class={"inline"}>
                                        <button type={"submit"} class={"iconbutton"}>{hitide_icons::UPVOTE.img(lang.tr(&lang::upvote()).into_owned())}</button>
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
                    <em>{lang.tr(&lang::score(post.score))}</em>
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
                                                <button type={"submit"}>{lang.tr(&lang::post_approve_undo()).into_owned()}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/approve", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr(&lang::post_approve()).into_owned()}</button>
                                            </form>
                                        }
                                    }
                                }
                                {
                                    if post.as_ref().sticky {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/make_unsticky", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr(&lang::post_make_not_sticky()).into_owned()}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/communities/{}/posts/{}/make_sticky", post.as_ref().community.id, post_id)}>
                                                <button type={"submit"}>{lang.tr(&lang::post_make_sticky()).into_owned()}</button>
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
                {
                    lang::TrElements::new(
                        lang.tr(&lang::post_submitted_by_to(lang::LangPlaceholder(0), lang::LangPlaceholder(1), lang::LangPlaceholder(2))),
                        |id, w| {
                            match id {
                                0 => render::rsx! {
                                    <TimeAgo since={created} lang={&lang} />
                                }.render_into(w),
                                1 => render::rsx! {
                                    <UserLink lang={&lang} user={post.as_ref().author.as_ref()} />
                                }.render_into(w),
                                2 => render::rsx! {
                                    <CommunityLink community={&post.as_ref().community} />
                                }.render_into(w),
                                _ => unreachable!(),
                            }
                        }
                    )
                }
            </p>
            {
                post.as_ref().href.as_ref().map(|href| {
                    render::rsx! {
                        <p><a rel={"ugc noopener"} href={href.as_ref()}>{href.as_ref()}</a></p>
                    }
                })
            }
            <div class={"postContent"}>
                <ContentView src={&post} />
            </div>
            {
                display_error_poll.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            {
                post.poll.as_ref().map(|poll| {
                    render::rsx! {
                        <PollView poll={poll} action={format!("/posts/{}/poll/submit", post.as_ref().as_ref().id)} lang={&lang} />
                    }
                })
            }
            <div class={"actionList"}>
                {
                    if author_is_me(&post.as_ref().author, &base_data.login) || (post.local && base_data.is_site_admin()) {
                        Some(render::rsx! {
                            <a href={format!("/posts/{}/delete", post_id)}>{lang.tr(&lang::delete()).into_owned()}</a>
                        })
                    } else {
                        None
                    }
                }
                {
                    if !post.local && base_data.is_site_admin() {
                        Some(render::rsx! {
                            <a href={format!("/posts/{}/site_block", post_id)}>{lang.tr(&lang::SITE_BLOCK)}</a>
                        })
                    } else {
                        None
                    }
                }
                {
                    if post.local {
                        None
                    } else {
                        if let Some(remote_url) = &post.as_ref().as_ref().remote_url {
                            Some(render::rsx! {
                                <a href={remote_url.as_ref()}>{lang.tr(&lang::remote_url()).into_owned()}</a>
                            })
                        } else {
                            None
                        }
                    }
                }
                {
                    if base_data.login.is_some() && !author_is_me(&post.as_ref().author, &base_data.login) {
                        Some(render::rsx! {
                            <a href={format!("/posts/{}/flag", post_id)}>{lang.tr(&lang::action_flag()).into_owned()}</a>
                        })
                    } else {
                        None
                    }
                }
            </div>
            <div>
                <h2>{lang.tr(&lang::comments())}</h2>
                {
                    display_error_comments.map(|msg| {
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
                                        {lang.tr(&lang::comment_reply_image_prompt()).into_owned()}
                                        {" "}
                                        <input type={"file"} accept={"image/*"} name={"attachment_media"} />
                                    </label>
                                </div>
                                <div>
                                    <label>
                                        <MaybeFillCheckbox values={&prev_values} name={"sensitive"} id={"sensitive"} default={post.as_ref().as_ref().sensitive} />
                                        {" "}
                                        {lang.tr(&lang::SENSITIVE)}
                                    </label>
                                </div>
                                <button r#type={"submit"}>{lang.tr(&lang::comment_submit()).into_owned()}</button>
                                <button r#type={"submit"} name={"preview"}>{lang.tr(&lang::preview()).into_owned()}</button>
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
                                <Comment comment={comment} sort={query.sort} root_sensitive={post.as_ref().as_ref().sensitive} base_data={&base_data} lang={&lang} />
                            }
                        }).collect::<Vec<_>>()
                    }
                </ul>
                {
                    replies.next_page.map(|next_page| {
                        render::rsx! {
                            <a href={format!("/posts/{}?page={}", post_id, next_page)}>{"-> "}{lang.tr(&lang::view_more_comments()).into_owned()}</a>
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
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr(&lang::post_delete_title())}>
            <h1>{post.as_ref().as_ref().title.as_ref()}</h1>
            <h2>{lang.tr(&lang::post_delete_question())}</h2>
            <form method={"POST"} action={format!("/posts/{}/delete/confirm", post.as_ref().as_ref().id)}>
                <a href={format!("/posts/{}/", post.as_ref().as_ref().id)}>{lang.tr(&lang::no_cancel())}</a>
                {" "}
                <button r#type={"submit"}>{lang.tr(&lang::delete_yes())}</button>
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

async fn page_post_site_block(
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
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr(&lang::post_site_block_title())}>
            <h1>{post.as_ref().as_ref().title.as_ref()}</h1>
            <h2>{lang.tr(&lang::post_site_block_question())}</h2>
            <p>{lang.tr(&lang::post_site_block_question_description())}</p>
            <form method={"POST"} action={format!("/posts/{}/site_block/confirm", post.as_ref().as_ref().id)}>
                <a href={format!("/posts/{}/", post.as_ref().as_ref().id)}>{lang.tr(&lang::no_cancel())}</a>
                {" "}
                <button r#type={"submit"}>{lang.tr(&lang::site_block_yes())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_post_site_block_confirm(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let api_res_get = res_to_error(
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
    let api_res_get = hyper::body::to_bytes(api_res_get.into_body()).await?;
    let post: RespPostInfo = serde_json::from_slice(&api_res_get)?;

    if let Some(remote_url) = &post.as_ref().as_ref().remote_url {
        res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::put(format!(
                        "{}/api/unstable/objects:blocks/{}",
                        ctx.backend_host,
                        percent_encoding::utf8_percent_encode(
                            &remote_url,
                            percent_encoding::NON_ALPHANUMERIC
                        ),
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
            .body("Successfully blocked from site.".into())?)
    } else {
        Err(crate::Error::UserError({
            let mut res = hyper::Response::new("Not a remote post".into());
            *res.status_mut() = hyper::StatusCode::BAD_REQUEST;
            res
        }))
    }
}

async fn page_post_flag(
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
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr(&lang::post_flag_title())}>
            <h1>{post.as_ref().as_ref().title.as_ref()}</h1>
            <h2>{lang.tr(&lang::post_flag_question())}</h2>
            <form method={"POST"} action={format!("/posts/{}/flag/submit", post.as_ref().as_ref().id)}>
                <div>
                    <strong>{lang.tr(&lang::post_flag_target_prompt())}</strong>
                </div>
                <div><label><input type={"checkbox"} name={"to_site_admin"} />{" "}{lang.tr(&lang::post_flag_target_choice_site_admin())}</label></div>
                <div><label><input type={"checkbox"} name={"to_community"} />{" "}{lang.tr(&lang::post_flag_target_choice_community())}</label></div>
                {
                    (post.as_ref().author.as_ref().map(|x| x.local) == Some(false)).then(|| render::rsx! {
                        <div><label><input type={"checkbox"} name={"to_remote_site_admin"} />{" "}{lang.tr(&lang::post_flag_target_choice_remote_site_admin()).into_owned()}</label></div>
                    })
                }
                <div>
                    <label>
                        {lang.tr(&lang::flag_comment_prompt())}<br />
                        <textarea name={"content_text"}>{""}</textarea>
                    </label>
                </div>
                <a href={format!("/posts/{}", post.as_ref().as_ref().id)}>{lang.tr(&lang::no_cancel())}</a>
                {" "}
                <button r#type={"submit"}>{lang.tr(&lang::submit())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_post_flag_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let mut body: serde_json::map::Map<String, serde_json::Value> =
        serde_urlencoded::from_bytes(&body)?;

    for key in &["to_community", "to_site_admin", "to_remote_site_admin"] {
        body.insert((*key).to_owned(), body.contains_key(*key).into());
    }

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/flags",
                    ctx.backend_host, post_id
                ))
                .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully flagged.".into())?)
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
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr(&lang::likes())}>
        {
            if api_res.items.is_empty() {
                Some(render::rsx! { <p>{lang.tr(&lang::post_likes_nothing()).into_owned()}</p> })
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
                        <p>{lang.tr(&lang::liked_by()).into_owned()}</p>
                        <ul>
                            {
                                api_res.items.iter().map(|like| {
                                    render::rsx! { <li><UserLink lang={&lang} user={Some(&like.user)} /></li> }
                                })
                                .collect::<Vec<_>>()
                            }
                            {
                                if api_res.next_page.is_some() {
                                    Some(render::rsx! { <li>{lang.tr(&lang::and_more()).into_owned()}</li> })
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

async fn handler_post_poll_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let (req_parts, body) = req.into_parts();
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::map::Map<String, serde_json::Value> =
        serde_urlencoded::from_bytes(&body)?;

    let body = if let Some(choice) = body.get("choice") {
        let choice = choice
            .as_str()
            .ok_or(crate::Error::InternalStrStatic("wrong type for choice"))?
            .parse()
            .map_err(|_| crate::Error::InternalStrStatic("Invalid choice"))?;

        PollVoteBody::Single { option: choice }
    } else {
        let choices: Vec<_> = body
            .keys()
            .filter_map(|key| {
                if let Ok(key) = key.parse::<i64>() {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        PollVoteBody::Multiple { options: choices }
    };

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::put(format!(
                    "{}/api/unstable/posts/{}/poll/your_vote",
                    ctx.backend_host, post_id
                ))
                .body(serde_json::to_vec(&body)?.into())?,
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
                None,
                Some(message),
                None,
                None,
            )
            .await
        }
        Err(other) => Err(other),
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
            .body("Successfully voted.".into())?),
    }
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
                None,
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
                    None,
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
                None,
                Some(&body_values),
                None,
            )
            .await
        }
        Err(other) => Err(other),
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: JustID = serde_json::from_slice(&api_res)?;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(
                    hyper::header::LOCATION,
                    format!("/posts/{}#comment{}", post_id, api_res.id),
                )
                .body("Successfully posted.".into())?)
        }
    }
}

pub fn route_posts() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_child_parse::<i64, _>(
        crate::RouteNode::new()
            .with_handler_async(hyper::Method::GET, page_post)
            .with_child(
                "delete",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::GET, page_post_delete)
                    .with_child(
                        "confirm",
                        crate::RouteNode::new()
                            .with_handler_async(hyper::Method::POST, handler_post_delete_confirm),
                    ),
            )
            .with_child(
                "site_block",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::GET, page_post_site_block)
                    .with_child(
                        "confirm",
                        crate::RouteNode::new().with_handler_async(
                            hyper::Method::POST,
                            handler_post_site_block_confirm,
                        ),
                    ),
            )
            .with_child(
                "flag",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::GET, page_post_flag)
                    .with_child(
                        "submit",
                        crate::RouteNode::new()
                            .with_handler_async(hyper::Method::POST, handler_post_flag_submit),
                    ),
            )
            .with_child(
                "like",
                crate::RouteNode::new().with_handler_async(hyper::Method::POST, handler_post_like),
            )
            .with_child(
                "likes",
                crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_post_likes),
            )
            .with_child(
                "poll",
                crate::RouteNode::new().with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_post_poll_submit),
                ),
            )
            .with_child(
                "unlike",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::POST, handler_post_unlike),
            )
            .with_child(
                "submit_reply",
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::POST, handler_post_submit_reply),
            ),
    )
}
