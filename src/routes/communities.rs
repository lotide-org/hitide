use crate::routes::{
    fetch_base_data, get_cookie_map_for_req, html_response, res_to_error, with_auth, HTPage,
    PostItem, RespMinimalCommunityInfo, RespPostListPost,
};
use std::sync::Arc;

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
                    "{}/api/unstable/communities/{}",
                    ctx.backend_host, community_id
                ))
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let community_info_api_res = hyper::body::to_bytes(community_info_api_res.into_body()).await?;

    let community_info: RespMinimalCommunityInfo =
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

    let follow_url = format!("/communities/{}/follow", community_id);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <h1>{community_info.name}</h1>
            <p>
                <form method={"POST"} action={&follow_url}>
                    <button r#type={"submit"}>{"Follow"}</button>
                </form>
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
        .body("Successfully followed".into())?)
}

pub fn route_communities() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_child_parse::<i64, _>(
        crate::RouteNode::new()
            .with_handler_async("GET", page_community)
            .with_child(
                "follow",
                crate::RouteNode::new().with_handler_async("POST", handler_community_follow),
            ),
    )
}
