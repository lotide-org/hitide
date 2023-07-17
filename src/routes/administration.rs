use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error,
};
use crate::components::HTPage;
use crate::lang;
use crate::resp_types::RespInstanceInfo;
use serde_derive::Deserialize;
use std::convert::TryInto;
use std::ops::Deref;
use std::sync::Arc;

async fn page_administration(
    _params: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;
    let lang = crate::get_lang_for_req(&req);

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .get(
                format!("{}/api/unstable/instance", ctx.backend_host)
                    .try_into()
                    .unwrap(),
            )
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespInstanceInfo = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::ADMINISTRATION);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ul>
                <li>
                    {lang.tr(&lang::ADMINISTRATION_SIGNUP_ALLOWED)}{" "}
                    <strong>{lang.tr(if api_res.signup_allowed {
                        &lang::ALLOWED_TRUE
                    } else {
                        &lang::ALLOWED_FALSE
                    })}</strong>
                </li>
            </ul>
        </HTPage>
    }))
}

pub fn route_administration() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_administration)
}
