use super::{
    fetch_base_data, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, with_auth,
};
use crate::components::{Comment, CommunityLink, Content, HTPage, UserLink};
use crate::resp_types::RespPostInfo;
use crate::util::author_is_me;
use std::sync::Arc;

async fn page_post(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
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
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

    let post: RespPostInfo = serde_json::from_slice(&api_res)?;

    let title = post.as_ref().as_ref().title.as_ref();

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={title}>
            <h1>{title}</h1>
            <p>
                <em>{post.score.to_string()}{" points"}</em>
                {" "}
                {
                    if base_data.login.is_some() {
                        Some(if post.your_vote.is_some() {
                            render::rsx! {
                                <form method={"POST"} action={format!("/posts/{}/unlike", post_id)}>
                                    <button type={"submit"}>{"Unlike"}</button>
                                </form>
                            }
                        } else {
                            render::rsx! {
                                <form method={"POST"} action={format!("/posts/{}/like", post_id)}>
                                    <button type={"submit"}>{"Like"}</button>
                                </form>
                            }
                        })
                    } else {
                        None
                    }
                }
            </p>
            <p>
                {"Submitted by "}<UserLink user={post.as_ref().author.as_ref()} />
                {" to "}<CommunityLink community={&post.as_ref().community} />
            </p>
            {
                match &post.as_ref().href {
                    None => None,
                    Some(href) => {
                        Some(render::rsx! {
                            <p><a href={href.as_ref()}>{href.as_ref()}</a></p>
                        })
                    }
                }
            }
            <Content src={&post} />
            {
                if author_is_me(&post.as_ref().author, &base_data.login) {
                    Some(render::rsx! {
                        <p>
                            <a href={format!("/posts/{}/delete", post_id)}>{"delete"}</a>
                        </p>
                    })
                } else {
                    None
                }
            }
            <div>
                <h2>{"Comments"}</h2>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <form method={"POST"} action={format!("/posts/{}/submit_reply", post.as_ref().as_ref().id)}>
                                <div>
                                    <textarea name={"content_markdown"}>{()}</textarea>
                                </div>
                                <button r#type={"submit"}>{"Post Comment"}</button>
                            </form>
                        })
                    } else {
                        None
                    }
                }
                <ul>
                    {
                        post.comments.iter().map(|comment| {
                            render::rsx! {
                                <Comment comment={comment} base_data={&base_data} />
                            }
                        }).collect::<Vec<_>>()
                    }
                </ul>
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

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

    let post: RespPostInfo = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Delete Post"}>
            <h1>{post.as_ref().as_ref().title.as_ref()}</h1>
            <h2>{"Delete this post?"}</h2>
            <form method={"POST"} action={format!("/posts/{}/delete/confirm", post.as_ref().as_ref().id)}>
                <a href={format!("/posts/{}/", post.as_ref().as_ref().id)}>{"No, cancel"}</a>
                {" "}
                <button r#type={"submit"}>{"Yes, delete"}</button>
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
            .request(with_auth(
                hyper::Request::delete(format!(
                    "{}/api/unstable/posts/{}",
                    ctx.backend_host, post_id,
                ))
                .body("".into())?,
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
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/like",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
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

async fn handler_post_unlike(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/unlike",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
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
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/replies",
                    ctx.backend_host, post_id
                ))
                .body(body.into())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully posted.".into())?)
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
                "unlike",
                crate::RouteNode::new().with_handler_async("POST", handler_post_unlike),
            )
            .with_child(
                "submit_reply",
                crate::RouteNode::new().with_handler_async("POST", handler_post_submit_reply),
            ),
    )
}
