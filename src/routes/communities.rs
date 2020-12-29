use crate::components::{CommunityLink, HTPage, MaybeFillInput, MaybeFillTextArea, PostItem};
use crate::resp_types::{
    JustContentHTML, JustStringID, RespCommunityInfoMaybeYour, RespMinimalAuthorInfo,
    RespMinimalCommunityInfo, RespPostListPost, RespYourFollow,
};
use crate::routes::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap,
};
use serde_derive::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

async fn page_communities(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;
    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::get(format!("{}/api/unstable/communities", ctx.backend_host,))
                    .body(Default::default())?,
            )
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let communities: Vec<RespMinimalCommunityInfo> = serde_json::from_slice(&api_res)?;

    let title = lang.tr("communities", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <div>
                <h2>{lang.tr("local", None)}</h2>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! { <a href={"/new_community"}>{lang.tr("community_create", None)}</a> })
                    } else {
                        None
                    }
                }
                <ul>
                    {
                        communities.iter()
                            .filter(|x| x.local)
                            .map(|community| {
                                render::rsx! {
                                    <li><CommunityLink community={&community} /></li>
                                }
                            })
                            .collect::<Vec<_>>()
                    }
                </ul>
            </div>
            <div>
                <h2>{lang.tr("remote", None)}</h2>
                <form method={"GET"} action={"/lookup"}>
                    <label>
                        {lang.tr("add_by_remote_id", None)}{" "}
                        <input r#type={"text"} name={"query"} placeholder={"group@example.com"} />
                    </label>
                    {" "}
                    <button r#type={"submit"}>{lang.tr("fetch", None)}</button>
                </form>
                <ul>
                    {
                        communities.iter()
                            .filter(|x| !x.local)
                            .map(|community| {
                                render::rsx! {
                                    <li><CommunityLink community={&community} /></li>
                                }
                            })
                            .collect::<Vec<_>>()
                    }
                </ul>
            </div>
        </HTPage>
    }))
}

async fn page_community(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    fn default_sort() -> crate::SortType {
        crate::SortType::Hot
    }

    #[derive(Deserialize)]
    struct Query {
        #[serde(default = "default_sort")]
        sort: crate::SortType,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    // TODO parallelize requests

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let community_info_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}{}",
                    ctx.backend_host,
                    community_id,
                    if base_data.login.is_some() {
                        "?include_your=true"
                    } else {
                        ""
                    },
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let community_info_api_res = hyper::body::to_bytes(community_info_api_res.into_body()).await?;

    let community_info: RespCommunityInfoMaybeYour =
        { serde_json::from_slice(&community_info_api_res)? };

    let posts_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}/posts?sort={}",
                    ctx.backend_host,
                    community_id,
                    query.sort.as_str(),
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let posts_api_res = hyper::body::to_bytes(posts_api_res.into_body()).await?;

    let posts: Vec<RespPostListPost<'_>> = serde_json::from_slice(&posts_api_res)?;

    let new_post_url = format!("/communities/{}/new_post", community_id);

    let title = community_info.as_ref().name.as_ref();

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title>
            <div class={"communitySidebar"}>
                <h2>{title}</h2>
                <div><em>{format!("@{}@{}", community_info.as_ref().name, community_info.as_ref().host)}</em></div>
                {
                    if community_info.as_ref().local {
                        None
                    } else if let Some(remote_url) = &community_info.as_ref().remote_url {
                        Some(render::rsx! {
                            <div class={"infoBox"}>
                                {lang.tr("community_remote_note", None)}
                                {" "}
                                <a href={remote_url.as_ref()}>{lang.tr("view_at_source", None)}{" ↗"}</a>
                            </div>
                        })
                    } else {
                        None // shouldn't ever happen
                    }
                }
                <p>
                    {
                        if base_data.login.is_some() {
                            Some(match community_info.your_follow {
                                Some(RespYourFollow { accepted: true }) => {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/communities/{}/unfollow", community_id)}>
                                            <button type={"submit"}>{lang.tr("follow_undo", None)}</button>
                                        </form>
                                    }
                                },
                                Some(RespYourFollow { accepted: false }) => {
                                    render::rsx! {
                                        <form>
                                            <button disabled={""}>{lang.tr("follow_request_sent", None)}</button>
                                        </form>
                                    }
                                },
                                None => {
                                    render::rsx! {
                                        <form method={"POST"} action={format!("/communities/{}/follow", community_id)}>
                                            <button type={"submit"}>{lang.tr("follow", None)}</button>
                                        </form>
                                    }
                                }
                            })
                        } else {
                            None
                        }
                    }
                </p>
                <p>
                    <a href={&new_post_url}>{lang.tr("post_new", None)}</a>
                </p>
                {
                    if community_info.you_are_moderator == Some(true) {
                        Some(render::rsx! {
                            <p>
                                <a href={format!("/communities/{}/edit", community_id)}>{lang.tr("community_edit_link", None)}</a>
                            </p>
                        })
                    } else {
                        None
                    }
                }
                <p>{community_info.description.as_ref()}</p>
                {
                    if community_info.as_ref().local {
                        Some(render::rsx! {
                            <p>
                                <a href={format!("/communities/{}/moderators", community_id)}>
                                    {lang.tr("moderators", None)}
                                </a>
                            </p>
                        })
                    } else {
                        None
                    }
                }
            </div>
            <div class={"sortOptions"}>
                <span>{lang.tr("sort", None)}</span>
                {
                    crate::SortType::VALUES.iter()
                        .map(|value| {
                            let name = lang.tr(value.lang_key(), None);
                            if query.sort == *value {
                                render::rsx! { <span>{name}</span> }
                            } else {
                                render::rsx! { <a href={format!("/communities/{}?sort={}", community_id, value.as_str())}>{name}</a> }
                            }
                        })
                        .collect::<Vec<_>>()
                }
            </div>
            {
                if posts.is_empty() {
                    Some(render::rsx! { <p>{lang.tr("nothing", None)}</p> })
                } else {
                    None
                }
            }
            <ul>
                {posts.iter().map(|post| {
                    PostItem { post, in_community: true, no_user: false, lang: &lang }
                }).collect::<Vec<_>>()}
            </ul>
        </HTPage>
    }))
}

async fn page_community_edit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_community_edit_inner(community_id, req.headers(), &cookies, ctx, None, None).await
}

async fn page_community_edit_inner(
    community_id: i64,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error: Option<String>,
    prev_values: Option<&HashMap<&str, serde_json::Value>>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, &cookies).await?;
    let lang = crate::get_lang_for_headers(headers);

    let community_info_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}",
                    ctx.backend_host, community_id,
                ))
                .body(Default::default())?,
                headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let community_info_api_res = hyper::body::to_bytes(community_info_api_res.into_body()).await?;

    let community_info: RespCommunityInfoMaybeYour =
        { serde_json::from_slice(&community_info_api_res)? };

    let title = lang.tr("community_edit", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <h2>{community_info.as_ref().name.as_ref()}</h2>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            <form method={"POST"} action={format!("/communities/{}/edit/submit", community_id)}>
                <label>
                    {lang.tr("description", None)}{":"}<br />
                    <MaybeFillTextArea values={&prev_values} name={"description"} default_value={Some(community_info.description.as_ref())} />
                </label>
                <div>
                    <button r#type={"submit"}>{lang.tr("submit", None)}</button>
                </div>
            </form>
        </HTPage>
    }))
}

async fn handler_communities_edit_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: HashMap<&str, serde_json::Value> = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/communities/{}",
                    ctx.backend_host, community_id
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
            page_community_edit_inner(
                community_id,
                &req_parts.headers,
                &cookies,
                ctx,
                Some(message),
                Some(&body),
            )
            .await
        }
        Err(other) => Err(other),
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(
                hyper::header::LOCATION,
                format!("/communities/{}", community_id),
            )
            .body("Successfully edited.".into())?),
    }
}

async fn handler_community_follow(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/communities/{}/follow",
                    ctx.backend_host, community_id
                ))
                .header(hyper::header::CONTENT_TYPE, "application/json")
                .body("{\"try_wait_for_accept\":true}".into())?,
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
            format!("/communities/{}", community_id),
        )
        .body("Successfully followed".into())?)
}

async fn page_community_moderators(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;
    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let community_info_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}{}",
                    ctx.backend_host,
                    community_id,
                    if base_data.login.is_some() {
                        "?include_your=true"
                    } else {
                        ""
                    },
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let community_info_api_res = hyper::body::to_bytes(community_info_api_res.into_body()).await?;
    let community_info: RespCommunityInfoMaybeYour =
        { serde_json::from_slice(&community_info_api_res)? };

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}/moderators",
                    ctx.backend_host, community_id,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespMinimalAuthorInfo> = serde_json::from_slice(&api_res)?;

    let title = lang.tr("moderators", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ul>
                {
                    api_res.iter().map(|user| {
                        render::rsx! {
                            <li>
                                <a href={format!("/users/{}", user.id)}>{user.username.as_ref()}</a>
                                {
                                    if community_info.you_are_moderator == Some(true) {
                                        Some(render::rsx! {
                                            <>
                                                {" "}
                                                <form class={"inline"} method={"POST"} action={format!("/communities/{}/moderators/remove", community_id)}>
                                                    <input type={"hidden"} name={"user"} value={user.id.to_string()} />
                                                    <button type={"submit"}>{lang.tr("remove", None)}</button>
                                                </form>
                                            </>
                                        })
                                    } else {
                                        None
                                    }
                                }
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()
                }
            </ul>
            {
                if community_info.you_are_moderator == Some(true) {
                    Some(render::rsx! {
                        <div>
                            <h2>{lang.tr("community_add_moderator", None)}</h2>
                            <form method={"POST"} action={format!("/communities/{}/moderators/add", community_id)}>
                                <label>
                                    {lang.tr("user_id_prompt", None)}{" "}
                                    <input type={"number"} name={"user"} />
                                </label>
                                {" "}
                                <button type={"submit"}>{lang.tr("add", None)}</button>
                            </form>
                        </div>
                    })
                } else {
                    None
                }
            }
        </HTPage>
    }))
}

async fn handler_community_moderators_add(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    #[derive(Deserialize)]
    struct ModeratorsAddParams {
        user: i64,
    }

    let body = hyper::body::to_bytes(body).await?;
    let body: ModeratorsAddParams = serde_urlencoded::from_bytes(&body)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::put(format!(
                    "{}/api/unstable/communities/{}/moderators/{}",
                    ctx.backend_host, community_id, body.user,
                ))
                .body(Default::default())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(
            hyper::header::LOCATION,
            format!("/communities/{}/moderators", community_id),
        )
        .body("Successfully added.".into())?)
}

async fn handler_community_moderators_remove(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    #[derive(Deserialize)]
    struct ModeratorsRemoveParams {
        user: i64,
    }

    let body = hyper::body::to_bytes(body).await?;
    let body: ModeratorsRemoveParams = serde_urlencoded::from_bytes(&body)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/communities/{}/moderators/{}",
                    ctx.backend_host, community_id, body.user,
                ))
                .body(Default::default())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(
            hyper::header::LOCATION,
            format!("/communities/{}/moderators", community_id),
        )
        .body("Successfully removed.".into())?)
}

async fn handler_community_post_approve(
    params: (i64, i64),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id, post_id) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/communities/{}/posts/{}",
                    ctx.backend_host, community_id, post_id
                ))
                .body("{\"approved\": true}".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully approved.".into())?)
}

async fn handler_community_post_unapprove(
    params: (i64, i64),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id, post_id) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/communities/{}/posts/{}",
                    ctx.backend_host, community_id, post_id
                ))
                .body("{\"approved\": false}".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully unapproved.".into())?)
}

async fn handler_community_unfollow(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/communities/{}/unfollow",
                    ctx.backend_host, community_id
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
            format!("/communities/{}", community_id),
        )
        .body("Successfully unfollowed".into())?)
}

async fn page_community_new_post(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_community_new_post_inner(community_id, req.headers(), &cookies, ctx, None, None, None)
        .await
}

async fn page_community_new_post_inner(
    community_id: i64,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error: Option<String>,
    prev_values: Option<&HashMap<Cow<'_, str>, serde_json::Value>>,
    display_preview: Option<&str>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, &cookies).await?;
    let lang = crate::get_lang_for_headers(headers);

    let submit_url = format!("/communities/{}/new_post/submit", community_id);

    let title = lang.tr("post_new", None);

    let display_preview = display_preview.map(|x| ammonia::clean(&x));

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            <form method={"POST"} action={&submit_url} enctype={"multipart/form-data"}>
                <table>
                    <tr>
                        <td>
                            <label for={"input_title"}>{lang.tr("title", None)}{":"}</label>
                        </td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"title"} required={true} id={"input_title"} />
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for={"input_url"}>{lang.tr("url", None)}{":"}</label>
                        </td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"href"} required={false} id={"input_url"} />
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for={"input_image"}>{lang.tr("post_new_image_prompt", None)}</label>
                        </td>
                        <td>
                            <input id={"input_image"} type={"file"} accept={"image/*"} name={"href_media"} />
                        </td>
                    </tr>
                </table>
                <label>
                    {lang.tr("text_with_markdown", None)}{":"}
                    <br />
                    <MaybeFillTextArea values={&prev_values} name={"content_markdown"} default_value={None} />
                </label>
                <div>
                    <button r#type={"submit"}>{lang.tr("submit", None)}</button>
                    <button r#type={"submit"} name={"preview"}>{lang.tr("preview", None)}</button>
                </div>
            </form>
            {
                display_preview.as_deref().map(|html| {
                    render::rsx! {
                        <div class={"preview"}>{render::raw!(html)}</div>
                    }
                })
            }
        </HTPage>
    }))
}

async fn handler_communities_new_post_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

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

            if field.name().unwrap() == "href_media" {
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

                if body_values.contains_key("href") && body_values["href"] != "" {
                    error = Some(lang.tr("post_new_href_conflict", None).into_owned());
                } else {
                    match stream.get_ref().content_type() {
                        None => {
                            error =
                                Some(lang.tr("post_new_missing_content_type", None).into_owned());
                        }
                        Some(mime) => {
                            log::debug!("will upload media");
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
                                        "href".into(),
                                        format!("local-media://{}", res.id).into(),
                                    );
                                }
                            }

                            log::debug!("finished media upload");
                        }
                    }
                }
            } else {
                let name = field.name().unwrap();
                if name == "href" && body_values.contains_key("href") && body_values["href"] != "" {
                    error = Some(lang.tr("post_new_href_conflict", None).into_owned());
                } else {
                    let name = name.to_owned();
                    let value = field.text().await?;
                    body_values.insert(name.into(), value.into());
                }
            }
        }

        if let Some(error) = error {
            return page_community_new_post_inner(
                community_id,
                &req_parts.headers,
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

                page_community_new_post_inner(
                    community_id,
                    &req_parts.headers,
                    &cookies,
                    ctx,
                    None,
                    Some(&body_values),
                    Some(&preview_res.content_html),
                )
                .await
            }
            Err(crate::Error::RemoteError((_, message))) => {
                page_community_new_post_inner(
                    community_id,
                    &req_parts.headers,
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

    body_values.insert("community".into(), community_id.into());
    if body_values.get("content_markdown").and_then(|x| x.as_str()) == Some("") {
        body_values.remove("content_markdown");
    }
    if body_values.get("href").and_then(|x| x.as_str()) == Some("") {
        body_values.remove("href");
    }

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!("{}/api/unstable/posts", ctx.backend_host))
                    .body(serde_json::to_vec(&body_values)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            #[derive(Deserialize)]
            struct PostsCreateResponse {
                id: i64,
            }

            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: PostsCreateResponse = serde_json::from_slice(&api_res)?;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(hyper::header::LOCATION, format!("/posts/{}", api_res.id))
                .body("Successfully posted.".into())?)
        }
        Err(crate::Error::RemoteError((_, message))) => {
            page_community_new_post_inner(
                community_id,
                &req_parts.headers,
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

pub fn route_communities() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_communities)
        .with_child_parse::<i64, _>(
            crate::RouteNode::new()
                .with_handler_async("GET", page_community)
                .with_child(
                    "edit",
                    crate::RouteNode::new()
                        .with_handler_async("GET", page_community_edit)
                        .with_child(
                            "submit",
                            crate::RouteNode::new()
                                .with_handler_async("POST", handler_communities_edit_submit),
                        ),
                )
                .with_child(
                    "follow",
                    crate::RouteNode::new().with_handler_async("POST", handler_community_follow),
                )
                .with_child(
                    "moderators",
                    crate::RouteNode::new()
                        .with_handler_async("GET", page_community_moderators)
                        .with_child(
                            "add",
                            crate::RouteNode::new()
                                .with_handler_async("POST", handler_community_moderators_add),
                        )
                        .with_child(
                            "remove",
                            crate::RouteNode::new()
                                .with_handler_async("POST", handler_community_moderators_remove),
                        ),
                )
                .with_child(
                    "posts",
                    crate::RouteNode::new().with_child_parse::<i64, _>(
                        crate::RouteNode::new()
                            .with_child(
                                "approve",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_community_post_approve),
                            )
                            .with_child(
                                "unapprove",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_community_post_unapprove),
                            ),
                    ),
                )
                .with_child(
                    "unfollow",
                    crate::RouteNode::new().with_handler_async("POST", handler_community_unfollow),
                )
                .with_child(
                    "new_post",
                    crate::RouteNode::new()
                        .with_handler_async("GET", page_community_new_post)
                        .with_child(
                            "submit",
                            crate::RouteNode::new()
                                .with_handler_async("POST", handler_communities_new_post_submit),
                        ),
                ),
        )
}
