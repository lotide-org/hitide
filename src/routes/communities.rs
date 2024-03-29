use crate::components::{
    maybe_fill_value, CommunityLink, ContentView, HTPage, HTPageAdvanced, MaybeFillCheckbox,
    MaybeFillInput, MaybeFillOption, MaybeFillTextArea, PostItem, TimeAgo,
};
use crate::lang;
use crate::query_types::PostListQuery;
use crate::resp_types::{
    JustContentHTML, JustStringID, RespCommunityInfoMaybeYour, RespCommunityModlogEvent,
    RespCommunityModlogEventDetails, RespList, RespMinimalAuthorInfo, RespMinimalCommunityInfo,
    RespPostListPost, RespYourFollow,
};
use crate::routes::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap, RespUserInfo,
};
use serde_derive::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::ops::Deref;
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

    #[derive(Deserialize, Serialize)]
    struct Query<'a> {
        local: Option<bool>,
        #[serde(rename = "your_follow.accepted")]
        your_follow_accepted: Option<bool>,
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let api_res = hyper::body::to_bytes(
        res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::get(format!(
                        "{}/api/unstable/communities?{}",
                        ctx.backend_host,
                        // for now this works but will need to change if we add other page parameters
                        serde_urlencoded::to_string(&query)?,
                    ))
                    .body(Default::default())?,
                    req.headers(),
                    &cookies,
                )?)
                .await?,
        )
        .await?
        .into_body(),
    )
    .await?;

    let communities: RespList<RespMinimalCommunityInfo> = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::COMMUNITIES);

    let filter_options: &[(lang::LangKey, bool, Option<bool>, Option<bool>)] = &[
        (lang::COMMUNITIES_FILTER_ALL, true, None, None),
        (lang::COMMUNITIES_FILTER_LOCAL, true, Some(true), None),
        (lang::COMMUNITIES_FILTER_REMOTE, true, Some(false), None),
        (
            lang::COMMUNITIES_FILTER_MINE,
            base_data.login.is_some(),
            None,
            Some(true),
        ),
    ];

    Ok(html_response(render::html! {
        <HTPage
            base_data={&base_data}
            lang={&lang}
            title={&title}
        >
            <h1>{title.as_ref()}</h1>
            {
                if let Some(login) = &base_data.login {
                    if login.permissions.create_community.allowed {
                        Some(render::rsx! { <a href={"/new_community"}>{lang.tr(&lang::COMMUNITY_CREATE)}</a> })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            <form method={"GET"} action={"/lookup"}>
                <label>
                    {lang.tr(&lang::ADD_BY_REMOTE_ID)}{" "}
                    <input r#type={"text"} name={"query"} placeholder={"group@example.com"} />
                </label>
                {" "}
                <button r#type={"submit"}>{lang.tr(&lang::FETCH)}</button>
            </form>
            <div class={"sortOptions"}>
                {
                    filter_options.iter()
                        .map(|(key, show, local, followed)| {
                            if *show {
                                let name = lang.tr(key);
                                Ok(Some(if &query.local == local && &query.your_follow_accepted == followed {
                                    render::rsx! { <span>{name}</span> }
                                } else {
                                    let mut href = "/communities".to_owned();
                                    if let Some(local) = local {
                                        if followed.is_some() {
                                            return Err(crate::Error::InternalStrStatic("Unimplemented"));
                                        }
                                        write!(href, "?local={}", local).unwrap();
                                    } else if let Some(followed) = followed {
                                        write!(href, "?your_follow.accepted={}", followed).unwrap();
                                    }

                                    render::rsx! { <a href={href}>{name}</a> }
                                }))
                            } else {
                                Ok(None)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?
                }
            </div>
            <ul>
                {
                    communities.items.iter()
                        .map(|community| {
                            render::rsx! {
                                <li><CommunityLink community={community} /></li>
                            }
                        })
                        .collect::<Vec<_>>()
                }
            </ul>
            {
                if let Some(next_page) = communities.next_page {
                    Some(render::rsx! {
                        <a href={format!("/communities?{}", serde_urlencoded::to_string(&Query {
                            page: Some(Cow::Borrowed(&next_page)),
                            ..query
                        })?)}>
                            {lang.tr(&lang::COMMUNITIES_PAGE_NEXT)}
                        </a>
                    })
                } else {
                    None
                }
            }
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
    struct Query<'a> {
        #[serde(default = "default_sort")]
        sort: crate::SortType,

        created_within: Option<Cow<'a, str>>,

        page: Option<Cow<'a, str>>,
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
                    "{}/api/unstable/posts?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&PostListQuery {
                        community: Some(community_id),
                        created_within: query.created_within.as_deref(),
                        sort_sticky: Some(query.sort == crate::SortType::Hot),
                        sort: Some(query.sort.as_str()),
                        page: query.page.as_deref(),
                        ..Default::default()
                    })?,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let posts_api_res = hyper::body::to_bytes(posts_api_res.into_body()).await?;

    let posts: RespList<RespPostListPost<'_>> = serde_json::from_slice(&posts_api_res)?;

    let new_post_url = format!("/communities/{}/new_post", community_id);

    let title = community_info.as_ref().name.as_ref();

    let feed_url = &community_info.feeds.atom.new;

    let basic_info_area = render::rsx! {
        <div class={"communityBaseInfo"}>
            <h2><a href={format!("/communities/{}", community_id)}>{title}</a></h2>
            <div><em>{format!("@{}@{}", community_info.as_ref().name, community_info.as_ref().host)}</em></div>
            {
                if community_info.as_ref().local {
                    None
                } else if let Some(remote_url) = &community_info.as_ref().remote_url {
                    Some(render::rsx! {
                        <div class={"infoBox"}>
                            {lang.tr(&lang::COMMUNITY_REMOTE_NOTE)}
                            {" "}
                            <a href={remote_url.as_ref()}>{lang.tr(&lang::VIEW_AT_SOURCE)}{" ↗"}</a>
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
                                        <button type={"submit"}>{lang.tr(&lang::FOLLOW_UNDO)}</button>
                                    </form>
                                }
                            },
                            Some(RespYourFollow { accepted: false }) => {
                                render::rsx! {
                                    <form method={"POST"} action={format!("/communities/{}/unfollow", community_id)}>
                                        <button type={"submit"}>{lang.tr(&lang::FOLLOW_REQUEST_CANCEL)}</button>
                                    </form>
                                }
                            },
                            None => {
                                render::rsx! {
                                    <form method={"POST"} action={format!("/communities/{}/follow", community_id)}>
                                        <button type={"submit"}>{lang.tr(&lang::FOLLOW)}</button>
                                    </form>
                                }
                            }
                        })
                    } else {
                        None
                    }
                }
            </p>
        </div>
    };

    let details_content = render::rsx! {
        <>
            <p>
                <a href={&new_post_url}>{lang.tr(&lang::POST_NEW)}</a>
            </p>
            {
                if community_info.you_are_moderator == Some(true) {
                    Some(render::rsx! {
                        <>
                            <p>
                                <a href={format!("/communities/{}/edit", community_id)}>{lang.tr(&lang::COMMUNITY_EDIT_LINK)}</a>
                            </p>
                            <p>
                                <a href={format!("/flags?to_community={}", community_id)}>{lang.tr(&lang::COMMUNITY_FLAGS_LINK)}</a>
                            </p>
                        </>
                    })
                } else {
                    None
                }
            }
            <ContentView src={&community_info.description} />
            {
                if community_info.as_ref().local {
                    Some(render::rsx! {
                        <>
                            <p>
                                <a href={format!("/communities/{}/moderators", community_id)}>
                                    {lang.tr(&lang::MODERATORS)}
                                </a>
                            </p>
                            <p>
                                <a href={format!("/communities/{}/modlog", community_id)}>
                                    {lang.tr(&lang::MODLOG)}
                                </a>
                            </p>
                        </>
                    })
                } else {
                    None
                }
            }
            {
                if community_info.you_are_moderator == Some(true) || base_data.is_site_admin() {
                    Some(render::rsx! {
                        <p>
                            <a href={format!("/communities/{}/delete", community_id)}>{lang.tr(&lang::COMMUNITY_DELETE_LINK)}</a>
                        </p>
                    })
                } else {
                    None
                }
            }
        </>
    };

    Ok(html_response(render::html! {
        <HTPageAdvanced
            base_data={&base_data}
            lang={&lang}
            title
            head_items={render::rsx! {
                <link rel={"alternate"} type={"application/atom+xml"} href={feed_url.as_ref()} />
            }}
        >
            <div class={"communityDetailsMobile"}>
                {basic_info_area.clone()}
                <details>
                    {details_content.clone()}
                </details>
                <hr />
            </div>
            <div class={"communitySidebar"}>
                {basic_info_area}
                {details_content}
            </div>
            <div class={"sortOptions"}>
                <span>{lang.tr(&lang::sort())}</span>
                {
                    crate::SortType::VALUES.iter()
                        .map(|value| {
                            let name = lang.tr(&value.lang_key()).into_owned();
                            if query.sort == *value {
                                render::rsx! { <span>{name}</span> }
                            } else {
                                render::rsx! { <a href={format!("/communities/{}?sort={}", community_id, value.as_str())}>{name}</a> }
                            }
                        })
                        .collect::<Vec<_>>()
                }
                {
                    (query.sort == crate::SortType::Top)
                        .then(|| {
                            render::rsx! {
                                <div class={"timeframeOptions"}>
                                    <span>{lang.tr(&lang::POST_TIMEFRAME)}</span>
                                    {
                                        [
                                            (lang::TIMEFRAME_ALL, None),
                                            (lang::TIMEFRAME_YEAR, Some("P1Y")),
                                            (lang::TIMEFRAME_MONTH, Some("P1M")),
                                            (lang::TIMEFRAME_WEEK, Some("P1W")),
                                            (lang::TIMEFRAME_DAY, Some("P1D")),
                                            (lang::TIMEFRAME_HOUR, Some("PT1H")),
                                        ]
                                            .iter()
                                            .map(|(key, interval)| {
                                                let name = lang.tr(key);
                                                if query.created_within.as_deref() == *interval {
                                                    render::rsx! { <span>{name}</span> }
                                                } else {
                                                    if let Some(interval) = interval {
                                                        render::rsx! { <a href={format!("/communities/{}?sort=top&created_within={}", community_id, interval)}>{name}</a> }
                                                    } else {
                                                        render::rsx! { <a href={format!("/communities/{}?sort=top", community_id)}>{name}</a> }
                                                    }
                                                }
                                            })
                                            .collect::<Vec<_>>()
                                    }
                                </div>
                            }
                        })
                }
            </div>
            {
                if posts.items.is_empty() {
                    Some(render::rsx! { <p>{lang.tr(&lang::NOTHING)}</p> })
                } else {
                    None
                }
            }
            <ul>
                {posts.items.iter().map(|post| {
                    PostItem { post, in_community: true, no_user: false, lang: &lang }
                }).collect::<Vec<_>>()}
            </ul>
            {
                if let Some(next_page) = &posts.next_page {
                    Some(render::rsx! {
                        <a href={format!("/communities/{}?sort={}&page={}", community_id, query.sort.as_str(), next_page)}>
                            {lang.tr(&lang::POSTS_PAGE_NEXT)}
                        </a>
                    })
                } else {
                    None
                }
            }
        </HTPageAdvanced>
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
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;
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
                cookies,
            )?)
            .await?,
    )
    .await?;
    let community_info_api_res = hyper::body::to_bytes(community_info_api_res.into_body()).await?;

    let community_info: RespCommunityInfoMaybeYour =
        { serde_json::from_slice(&community_info_api_res)? };

    let title = lang.tr(&lang::COMMUNITY_EDIT);

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
                    {lang.tr(&lang::description())}{":"}<br />
                    <MaybeFillTextArea values={&prev_values} name={"description_markdown"} default_value={Some(community_info.description.content_markdown.as_deref().or(community_info.description.content_html.as_deref()).or(community_info.description.content_text.as_deref()).unwrap())} />
                </label>
                <div>
                    <button r#type={"submit"}>{lang.tr(&lang::submit())}</button>
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

async fn page_community_delete(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}",
                    ctx.backend_host, community_id
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

    let community: RespCommunityInfoMaybeYour = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&lang.tr(&lang::community_delete_title())}>
            <h1>{community.as_ref().name.as_ref()}</h1>
            <h2>{lang.tr(&lang::community_delete_question())}</h2>
            <form method={"POST"} action={format!("/communities/{}/delete/confirm", community.as_ref().id)}>
                <a href={format!("/communities/{}/", community.as_ref().id)}>{lang.tr(&lang::no_cancel())}</a>
                {" "}
                <button r#type={"submit"}>{lang.tr(&lang::delete_yes())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_community_delete_confirm(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/communities/{}",
                    ctx.backend_host, community_id,
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

    let headers = req.headers();
    let cookies = get_cookie_map_for_req(&req)?;

    page_community_moderators_inner(community_id, headers, &cookies, ctx, None, None).await
}

async fn page_community_moderators_inner(
    community_id: i64,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error_main: Option<String>,
    display_error_add: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

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
                headers,
                cookies,
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
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespMinimalAuthorInfo> = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::MODERATORS);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            {
                display_error_main.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
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
                                                    <button type={"submit"}>{lang.tr(&lang::REMOVE)}</button>
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
                            <h2>{lang.tr(&lang::COMMUNITY_ADD_MODERATOR)}</h2>
                            {
                                display_error_add.map(|msg| {
                                    render::rsx! {
                                        <div class={"errorBox"}>{msg}</div>
                                    }
                                })
                            }
                            <form method={"POST"} action={format!("/communities/{}/moderators/add", community_id)}>
                                <label>
                                    {lang.tr(&lang::LOCAL_USER_NAME_PROMPT)}{" "}
                                    <input type={"text"} name={"username"} />
                                </label>
                                {" "}
                                <button type={"submit"}>{lang.tr(&lang::ADD)}</button>
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

    let lang = crate::get_lang_for_headers(&req_parts.headers);
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    #[derive(Deserialize)]
    struct ModeratorsAddParams<'a> {
        username: Cow<'a, str>,
    }

    let body = hyper::body::to_bytes(body).await?;
    let body: ModeratorsAddParams = serde_urlencoded::from_bytes(&body)?;

    #[derive(Serialize)]
    struct UsersListQuery<'a> {
        local: bool,
        username: &'a str,
    }

    let user_lookup_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/users?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&UsersListQuery {
                        local: true,
                        username: &body.username,
                    })?,
                ))
                .body(Default::default())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    let add_result = match user_lookup_api_res {
        Err(err) => Err(err),
        Ok(api_res) => {
            let value = hyper::body::to_bytes(api_res.into_body()).await?;
            let user_list: RespList<RespUserInfo> = serde_json::from_slice(&value)?;

            match user_list.items.first() {
                None => Err(crate::Error::InternalUserError(
                    lang.tr(&lang::no_such_local_user()).into_owned(),
                )),
                Some(target_user) => {
                    res_to_error(
                        ctx.http_client
                            .request(for_client(
                                hyper::Request::put(format!(
                                    "{}/api/unstable/communities/{}/moderators/{}",
                                    ctx.backend_host, community_id, target_user.base.id,
                                ))
                                .body(Default::default())?,
                                &req_parts.headers,
                                &cookies,
                            )?)
                            .await?,
                    )
                    .await
                }
            }
        }
    };

    match add_result {
        Err(crate::Error::RemoteError((_, message)))
        | Err(crate::Error::InternalUserError(message)) => {
            page_community_moderators_inner(
                community_id,
                &req_parts.headers,
                &cookies,
                ctx,
                None,
                Some(message),
            )
            .await
        }
        Err(other) => Err(other),
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(
                hyper::header::LOCATION,
                format!("/communities/{}/moderators", community_id),
            )
            .body("Successfully added.".into())?),
    }
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

    let api_res = res_to_error(
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
    .await;

    match api_res {
        Err(crate::Error::RemoteError((_, message))) => {
            page_community_moderators_inner(
                community_id,
                &req_parts.headers,
                &cookies,
                ctx,
                Some(message),
                None,
            )
            .await
        }
        Err(other) => Err(other),
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(
                hyper::header::LOCATION,
                format!("/communities/{}/moderators", community_id),
            )
            .body("Successfully removed.".into())?),
    }
}

async fn page_community_modlog(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}/modlog/events",
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
    let api_res: RespList<RespCommunityModlogEvent> = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::MODLOG);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ul>
                {
                    api_res.items.iter().map(|event| {
                        render::rsx! {
                            <li>
                                <TimeAgo since={chrono::DateTime::parse_from_rfc3339(&event.time).unwrap()} lang={&lang} />
                                {" - "}
                                {
                                    match &event.details {
                                        RespCommunityModlogEventDetails::ApprovePost { post } => {
                                            render::rsx! {
                                                <>
                                                    {lang.tr(&lang::MODLOG_EVENT_APPROVE_POST)}
                                                    {" "}
                                                    <a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a>
                                                </>
                                            }
                                        }
                                        RespCommunityModlogEventDetails::RejectPost { post } => {
                                            render::rsx! {
                                                <>
                                                    {lang.tr(&lang::MODLOG_EVENT_REJECT_POST)}
                                                    {" "}
                                                    <a href={format!("/posts/{}", post.id)}>{post.title.as_ref()}</a>
                                                </>
                                            }
                                        }
                                    }
                                }
                            </li>
                        }
                    })
                    .collect::<Vec<_>>()
                }
            </ul>
        </HTPage>
    }))
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

async fn handler_community_post_make_sticky(
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
                .body("{\"sticky\": true}".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully stickied.".into())?)
}

async fn handler_community_post_make_unsticky(
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
                .body("{\"sticky\": false}".into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully unstickied.".into())?)
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
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;
    let lang = crate::get_lang_for_headers(headers);

    let submit_url = format!("/communities/{}/new_post/submit", community_id);

    let title_key = lang::post_new();
    let title = lang.tr(&title_key);

    let poll_option_names: Vec<_> = (0..4).map(|idx| format!("poll_option_{}", idx)).collect();

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
                            <label for={"input_title"}>{lang.tr(&lang::title())}{":"}</label>
                        </td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"title"} required={true} id={"input_title"} />
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for={"input_url"}>{lang.tr(&lang::url())}{":"}</label>
                        </td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"href"} required={false} id={"input_url"} />
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for={"input_image"}>{lang.tr(&lang::post_new_image_prompt())}</label>
                        </td>
                        <td>
                            <input id={"input_image"} type={"file"} accept={"image/*"} name={"href_media"} />
                        </td>
                    </tr>
                </table>
                <label>
                    {lang.tr(&lang::text_with_markdown())}{":"}
                    <br />
                    <MaybeFillTextArea values={&prev_values} name={"content_markdown"} default_value={None} />
                </label>
                <br />
                <label>
                    <MaybeFillCheckbox values={&prev_values} id={"sensitiveCheckbox"} name={"sensitive"} default={false} />{" "}
                    {lang.tr(&lang::sensitive()).into_owned()}
                </label>
                <br />
                <MaybeFillCheckbox values={&prev_values} id={"pollEnableCheckbox"} name={"poll_enabled"} default={false} />
                <label for={"pollEnableCheckbox"}>
                    {" "}
                    {lang.tr(&lang::new_post_poll())}
                </label>
                <br />
                <div class={"pollArea"}>
                    <div>
                        <label>
                            <MaybeFillCheckbox values={&prev_values} name={"poll_multiple"} id={"poll_multiple"} default={false} />
                            {" "}
                            {lang.tr(&lang::poll_new_multiple())}
                        </label>
                    </div>
                    {lang.tr(&lang::poll_new_options_prompt())}
                    <ul>
                        {
                            poll_option_names.iter().map(|name| {
                                render::rsx! {
                                    <li><MaybeFillInput values={&prev_values} r#type={"text"} name={&name} id={&name} required={false} /></li>
                                }
                            })
                            .collect::<Vec<_>>()
                        }
                    </ul>
                    <div>
                        {lang.tr(&lang::poll_new_closes_prompt())}
                        {" "}
                        <input type={"number"} name={"poll_duration_value"} required={""} value={maybe_fill_value(&prev_values, "poll_duration_value", Some("10"))} />
                        <select name={"poll_duration_unit"}>
                            <MaybeFillOption default_value={None} values={&prev_values} name={"poll_duration_unit"} value={"m"}>{lang.tr(&lang::time_input_minutes())}</MaybeFillOption>
                            <MaybeFillOption default_value={None} values={&prev_values} name={"poll_duration_unit"} value={"h"}>{lang.tr(&lang::time_input_hours())}</MaybeFillOption>
                            <MaybeFillOption default_value={None} values={&prev_values} name={"poll_duration_unit"} value={"d"}>{lang.tr(&lang::time_input_days())}</MaybeFillOption>
                        </select>
                    </div>
                </div>
                <div>
                    <button r#type={"submit"}>{lang.tr(&lang::submit())}</button>
                    <button r#type={"submit"} name={"preview"}>{lang.tr(&lang::preview())}</button>
                </div>
            </form>
            {
                display_preview.map(|html| {
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

    let mut body_values_src: HashMap<Cow<'_, str>, serde_json::Value> = HashMap::new();
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

                if body_values_src.contains_key("href") && body_values_src["href"] != "" {
                    error = Some(lang.tr(&lang::post_new_href_conflict()).into_owned());
                } else {
                    match stream.get_ref().content_type() {
                        None => {
                            error =
                                Some(lang.tr(&lang::post_new_missing_content_type()).into_owned());
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

                                    body_values_src.insert(
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
                if name == "href"
                    && body_values_src.contains_key("href")
                    && body_values_src["href"] != ""
                {
                    error = Some(lang.tr(&lang::post_new_href_conflict()).into_owned());
                } else {
                    let name = name.to_owned();
                    let value = field.text().await?;
                    body_values_src.insert(name.into(), value.into());
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
                Some(&body_values_src),
                None,
            )
            .await;
        }
    }

    let body_values_src = body_values_src;
    let mut body_values: HashMap<_, _> = body_values_src
        .iter()
        .map(|(key, value)| (Cow::Borrowed(key.as_ref()), Cow::Borrowed(value)))
        .collect();

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
                    Some(&body_values_src),
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
                    Some(&body_values_src),
                    None,
                )
                .await
            }
            Err(other) => Err(other),
        };
    }

    body_values.insert("community".into(), Cow::Owned(community_id.into()));
    if body_values.get("content_markdown").and_then(|x| x.as_str()) == Some("") {
        body_values.remove("content_markdown");
    }
    if body_values.get("href").and_then(|x| x.as_str()) == Some("") {
        body_values.remove("href");
    }

    if body_values.remove("sensitive").is_some() {
        body_values.insert("sensitive".into(), Cow::Owned(true.into()));
    }

    if body_values.remove("poll_enabled").is_some() {
        let options: Vec<_> = (0..4)
            .filter_map(|idx| {
                let value = body_values.remove(format!("poll_option_{}", idx).deref());
                if value.as_ref().map(|x| x.as_ref()) == Some(&serde_json::json!("")) {
                    None
                } else {
                    value
                }
            })
            .collect();
        let multiple: bool = body_values.remove("poll_multiple").is_some();

        let duration_value = body_values.remove("poll_duration_value");
        let duration_value = duration_value.as_ref().and_then(|x| x.as_str()).ok_or(
            crate::Error::InternalStrStatic("Missing poll_duration_value"),
        )?;

        let duration_unit = body_values.remove("poll_duration_unit");
        let closed_in = match duration_unit.as_ref().and_then(|x| x.as_str()).ok_or(
            crate::Error::InternalStrStatic("Missing poll_duration_unit"),
        )? {
            "m" => format!("PT{}M", duration_value),
            "h" => format!("PT{}H", duration_value),
            "d" => format!("P{}D", duration_value),
            _ => return Err(crate::Error::InternalStrStatic("Unknown duration unit")),
        };

        body_values.insert(
            "poll".into(),
            Cow::Owned(serde_json::json!({
                "options": options,
                "multiple": multiple,
                "closed_in": closed_in,
            })),
        );
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
                Some(&body_values_src),
                None,
            )
            .await
        }
        Err(other) => Err(other),
    }
}

pub fn route_communities() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async(hyper::Method::GET, page_communities)
        .with_child_parse::<i64, _>(
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_community)
                .with_child(
                    "edit",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::GET, page_community_edit)
                        .with_child(
                            "submit",
                            crate::RouteNode::new().with_handler_async(
                                hyper::Method::POST,
                                handler_communities_edit_submit,
                            ),
                        ),
                )
                .with_child(
                    "delete",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::GET, page_community_delete)
                        .with_child(
                            "confirm",
                            crate::RouteNode::new().with_handler_async(
                                hyper::Method::POST,
                                handler_community_delete_confirm,
                            ),
                        ),
                )
                .with_child(
                    "follow",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_community_follow),
                )
                .with_child(
                    "moderators",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::GET, page_community_moderators)
                        .with_child(
                            "add",
                            crate::RouteNode::new().with_handler_async(
                                hyper::Method::POST,
                                handler_community_moderators_add,
                            ),
                        )
                        .with_child(
                            "remove",
                            crate::RouteNode::new().with_handler_async(
                                hyper::Method::POST,
                                handler_community_moderators_remove,
                            ),
                        ),
                )
                .with_child(
                    "modlog",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::GET, page_community_modlog),
                )
                .with_child(
                    "posts",
                    crate::RouteNode::new().with_child_parse::<i64, _>(
                        crate::RouteNode::new()
                            .with_child(
                                "approve",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_community_post_approve,
                                ),
                            )
                            .with_child(
                                "make_sticky",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_community_post_make_sticky,
                                ),
                            )
                            .with_child(
                                "make_unsticky",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_community_post_make_unsticky,
                                ),
                            )
                            .with_child(
                                "unapprove",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_community_post_unapprove,
                                ),
                            ),
                    ),
                )
                .with_child(
                    "unfollow",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_community_unfollow),
                )
                .with_child(
                    "new_post",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::GET, page_community_new_post)
                        .with_child(
                            "submit",
                            crate::RouteNode::new().with_handler_async(
                                hyper::Method::POST,
                                handler_communities_new_post_submit,
                            ),
                        ),
                ),
        )
}
