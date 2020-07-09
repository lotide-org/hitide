use crate::components::{CommunityLink, HTPage, PostItem};
use crate::resp_types::{
    RespCommunityInfoMaybeYour, RespMinimalCommunityInfo, RespPostListPost, RespYourFollow,
};
use crate::routes::{
    fetch_base_data, get_cookie_map, get_cookie_map_for_req, html_response, res_to_error, with_auth,
};
use std::collections::HashMap;
use std::sync::Arc;

async fn page_communities(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

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

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Communities"}>
            <h1>{"Communities"}</h1>
            <div>
                <h2>{"Local"}</h2>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! { <a href={"/new_community"}>{"Create Community"}</a> })
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
                <h2>{"Remote"}</h2>
                <form method={"GET"} action={"/lookup"}>
                    <label>
                        {"Add by ID: "}
                        <input r#type={"text"} name={"query"} placeholder={"group@example.com"} />
                    </label>
                    {" "}
                    <button r#type={"submit"}>{"Fetch"}</button>
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

    let cookies = get_cookie_map_for_req(&req)?;

    // TODO parallelize requests

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let community_info_api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
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
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities/{}/posts",
                    ctx.backend_host, community_id
                ))
                .body(Default::default())?,
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
        <HTPage base_data={&base_data} title>
            <h1>{title}</h1>
            <p>
                {
                    if base_data.login.is_some() {
                        Some(match community_info.your_follow {
                            Some(RespYourFollow { accepted: true }) => {
                                render::rsx! {
                                    <form method={"POST"} action={format!("/communities/{}/unfollow", community_id)}>
                                        <button type={"submit"}>{"Unfollow"}</button>
                                    </form>
                                }
                            },
                            Some(RespYourFollow { accepted: false }) => {
                                render::rsx! {
                                    <form>
                                        <button disabled={""}>{"Follow request sent!"}</button>
                                    </form>
                                }
                            },
                            None => {
                                render::rsx! {
                                    <form method={"POST"} action={format!("/communities/{}/follow", community_id)}>
                                        <button type={"submit"}>{"Follow"}</button>
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
                <a href={&new_post_url}>{"New Post"}</a>
            </p>
            <ul>
                {posts.iter().map(|post| {
                    PostItem { post, in_community: true }
                }).collect::<Vec<_>>()}
            </ul>
        </HTPage>
    }))
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
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/communities/{}/follow",
                    ctx.backend_host, community_id
                ))
                .header(hyper::header::CONTENT_TYPE, "application/json")
                .body("{\"try_wait_for_accept\":true}".into())?,
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

async fn handler_community_unfollow(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/communities/{}/unfollow",
                    ctx.backend_host, community_id
                ))
                .body(Default::default())?,
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

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let submit_url = format!("/communities/{}/new_post/submit", community_id);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"New Post"}>
            <h1>{"New Post"}</h1>
            <form method={"POST"} action={&submit_url}>
                <div>
                    <label>
                        {"Title: "}<input r#type={"text"} name={"title"} required={"true"} />
                    </label>
                </div>
                <div>
                    <label>
                        {"URL: "}<input r#type={"text"} name={"href"} />
                    </label>
                </div>
                <div>
                    <label>
                        {"Text:"}
                        <br />
                        <textarea name={"content_text"}>{""}</textarea>
                    </label>
                </div>
                <div>
                    <button r#type={"submit"}>{"Submit"}</button>
                </div>
            </form>
        </HTPage>
    }))
}

async fn handler_communities_new_post_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (community_id,) = params;

    let cookies_string = req
        .headers()
        .get(hyper::header::COOKIE)
        .map(|x| x.to_str())
        .transpose()?
        .map(|x| x.to_owned());
    let cookies_string = cookies_string.as_deref();

    let cookies = get_cookie_map(cookies_string)?;

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let mut body: HashMap<&str, serde_json::Value> = serde_urlencoded::from_bytes(&body)?;
    body.insert("community", community_id.into());
    if body.get("content_text").and_then(|x| x.as_str()) == Some("") {
        body.remove("content_text");
    }
    if body.get("href").and_then(|x| x.as_str()) == Some("") {
        body.remove("href");
    }
    let body = serde_json::to_vec(&body)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!("{}/api/unstable/posts", ctx.backend_host))
                    .body(body.into())?,
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
        .body("Successfully posted.".into())?)
}

pub fn route_communities() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_communities)
        .with_child_parse::<i64, _>(
            crate::RouteNode::new()
                .with_handler_async("GET", page_community)
                .with_child(
                    "follow",
                    crate::RouteNode::new().with_handler_async("POST", handler_community_follow),
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
