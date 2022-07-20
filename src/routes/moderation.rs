use super::{fetch_base_data, for_client, get_cookie_map_for_req, html_response, res_to_error};
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
                        "{}/api/unstable/flags?to_community={}",
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
            <ul>
                {communities.items.iter().map(|community| {
                    render::rsx! {
                        <li>
                            <a class={if query.community == Some(community.base.id) { "selected" } else { "" }} href={format!("/moderation?community={}", community.base.id)}>
                                {community.base.name.deref()}{" ("}{community.pending_moderation_actions.unwrap()}{")"}
                            </a>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
            {
                if query.community.is_some() {
                    Some(flags.as_ref().unwrap().items.iter().map(|flag| {
                        FlagItem { flag, in_community: true, lang: &lang }
                    }).collect::<Vec<_>>())
                         }
                         else {
                             None
                         }
            }
        </HTPage>
    )))
}

pub fn route_moderation() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_moderation)
}
