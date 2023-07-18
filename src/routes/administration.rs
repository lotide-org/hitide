use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap,
};
use crate::components::{HTPage, MaybeFillOption};
use crate::lang;
use crate::resp_types::RespInstanceInfo;
use std::collections::HashMap;
use std::convert::TryInto;
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

    let title = lang.tr(&lang::ADMINISTRATION);

    if !base_data.is_site_admin() {
        return Ok(html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <div class={"errorBox"}>
                    {lang.tr(&lang::not_site_admin())}
                </div>
            </HTPage>
        }));
    }

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

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <a href={"/administration/edit"}>{lang.tr(&lang::administration_edit())}</a>
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

async fn page_administration_edit(
    _params: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    page_administration_edit_inner(req.headers(), &cookies, ctx, None, None).await
}

async fn page_administration_edit_inner(
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error: Option<String>,
    prev_values: Option<&HashMap<&str, serde_json::Value>>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let title = lang.tr(&lang::ADMINISTRATION_EDIT);

    if !base_data.is_site_admin() {
        return Ok(html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <div class={"errorBox"}>
                    {lang.tr(&lang::not_site_admin())}
                </div>
            </HTPage>
        }));
    }

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

    let signup_allowed_value = Some(crate::bool_as_str(api_res.signup_allowed));

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
            <form method={"POST"} action={"/administration/edit/submit"}>
                <label>
                    {lang.tr(&lang::administration_edit_signup_allowed())}<br />
                    <select name={"signup_allowed"}>
                        <MaybeFillOption value={"true"} values={&prev_values} default_value={signup_allowed_value} name={"signup_allowed"}>
                            {lang.tr(&lang::allowed_true())}
                        </MaybeFillOption>
                        <MaybeFillOption value={"false"} values={&prev_values} default_value={signup_allowed_value} name={"signup_allowed"}>
                            {lang.tr(&lang::allowed_false())}
                        </MaybeFillOption>
                    </select>
                </label>
                <br />
                <br />
                <button type={"submit"}>{"Save"}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_administration_edit_submit(
    _params: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let mut body: HashMap<&str, serde_json::Value> = serde_urlencoded::from_bytes(&body)?;

    body.insert(
        "signup_allowed",
        body.get("signup_allowed")
            .and_then(|x| x.as_str())
            .ok_or(crate::Error::InternalStrStatic(
                "Failed to extract signup_allowed in administration edit",
            ))?
            .parse()?,
    );

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!("{}/api/unstable/instance", ctx.backend_host,))
                    .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Err(crate::Error::RemoteError((_, message))) => {
            page_administration_edit_inner(
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
            .header(hyper::header::LOCATION, "/administration")
            .body("Successfully edited.".into())?),
    }
}

pub fn route_administration() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async(hyper::Method::GET, page_administration)
        .with_child(
            "edit",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_administration_edit)
                .with_child(
                    "submit",
                    crate::RouteNode::new().with_handler_async(
                        hyper::Method::POST,
                        handler_administration_edit_submit,
                    ),
                ),
        )
}
