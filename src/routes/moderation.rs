use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error,
};
use crate::components::{FlagItem, HTPage};
use crate::lang;
use crate::resp_types::{RespCommunityInfoMaybeYour, RespFlagInfo, RespList};
use serde_derive::Deserialize;
use std::ops::Deref;
use std::sync::Arc;

async fn page_moderation(
    _params: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;
    let lang = crate::get_lang_for_req(&req);

    #[derive(Deserialize)]
    struct Query {
        community: Option<i64>,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let communities_api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/communities?you_are_moderator=true&include_your=true",
                    ctx.backend_host,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    let communities_api_res = hyper::body::to_bytes(communities_api_res.into_body()).await?;
    let communities: RespList<RespCommunityInfoMaybeYour> =
        serde_json::from_slice(&communities_api_res)?;

    let flags_api_res = if let Some(community) = query.community {
        let api_res = res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::get(format!(
                        "{}/api/unstable/flags?to_community={}&dismissed=false",
                        ctx.backend_host, community,
                    ))
                    .body(Default::default())?,
                    req.headers(),
                    &cookies,
                )?)
                .await?,
        )
        .await?;
        Some(hyper::body::to_bytes(api_res.into_body()).await?)
    } else {
        None
    };
    let flags: Option<RespList<RespFlagInfo>> = flags_api_res
        .as_ref()
        .map(|x| serde_json::from_slice(x))
        .transpose()?;

    let title = lang.tr(&lang::MODERATION_DASHBOARD);

    Ok(html_response(render::html!(
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <nav class={"tabs"}>
                {communities.items.iter().map(|community| {
                    render::rsx! {
                        <a class={if query.community == Some(community.base.id) { "selected" } else { "" }} href={format!("/moderation?community={}", community.base.id)}>
                            {community.base.name.deref()}{" ("}{community.pending_moderation_actions.unwrap()}{")"}
                        </a>
                    }
                }).collect::<Vec<_>>()}
            </nav>
            {
                if query.community.is_some() {
                    Some(flags.as_ref().unwrap().items.iter().map(|flag| {
                        render::rsx! {
                            <div>
                                <FlagItem flag in_community={true} lang={&lang} />
                                <form method={"POST"} action={"/moderation/submit_dismiss"}>
                                    <input type={"hidden"} name={"community"} value={query.community.unwrap().to_string()} />
                                    <input type={"hidden"} name={"flag"} value={flag.id.to_string()} />
                                    <input type={"submit"} value={lang.tr(&lang::FLAG_DISMISS)} />
                                </form>
                            </div>
                        }
                    }).collect::<Vec<_>>())
                         }
                         else {
                             None
                         }
            }
        </HTPage>
    )))
}

async fn handler_moderation_submit_dismiss(
    _params: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct Body {
        community: i64,
        flag: i64,
    }

    let (req_parts, body) = req.into_parts();
    let body = hyper::body::to_bytes(body).await?;
    let body: Body = serde_urlencoded::from_bytes(&body)?;

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/flags/{}",
                    ctx.backend_host, body.flag
                ))
                .body(r#"{"community_dismissed":true}"#.into())?,
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
            format!("/moderation?community={}", body.community),
        )
        .body("Successfully dismissed.".into())?)
}

pub fn route_moderation() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async(hyper::Method::GET, page_moderation)
        .with_child(
            "submit_dismiss",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::POST, handler_moderation_submit_dismiss),
        )
}
