use crate::lang;
use crate::routes::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap, HTPage,
};
use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

async fn page_forgot_password(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    page_forgot_password_inner(ctx, req.headers(), &cookies, None).await
}

async fn page_forgot_password_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let title = lang.tr(&lang::FORGOT_PASSWORD);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={"/forgot_password/submit"}>
                <p>{lang.tr(&lang::forgot_password_info())}</p>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                <div>
                    <label>
                        {lang.tr(&lang::forgot_password_email_prompt())}
                        {" "}
                        <input type={"email"} name={"email_address"} required={"required"} />
                    </label>
                </div>
                <button type={"submit"}>{lang.tr(&lang::submit())}</button>
            </form>
        </HTPage>
    }))
}

async fn page_forgot_password_code(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    page_forgot_password_code_inner(ctx, req.headers(), &cookies, None).await
}

async fn page_forgot_password_code_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let title = lang.tr(&lang::FORGOT_PASSWORD);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={"/forgot_password/code/submit"}>
                <p>{lang.tr(&lang::forgot_password_code_info())}</p>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                <div>
                    <label>
                        {lang.tr(&lang::forgot_password_code_prompt())}
                        {" "}
                        <input type={"text"} name={"key"} required={"required"} />
                    </label>
                </div>
                <button type={"submit"}>{lang.tr(&lang::submit())}</button>
            </form>
        </HTPage>
    }))
}

async fn page_forgot_password_code_reset_inner(
    key: &str,
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let title = lang.tr(&lang::FORGOT_PASSWORD);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={"/forgot_password/code/submit"}>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                <input type={"hidden"} name={"key"} value={key} />
                <div>
                    <label>
                        {lang.tr(&lang::forgot_password_new_password_prompt())}
                        {" "}
                        <input type={"password"} name={"new_password"} required={"required"} />
                    </label>
                </div>
                <button type={"submit"}>{lang.tr(&lang::submit())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_forgot_password_code_submit(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct CodeSubmitBody<'a> {
        key: Cow<'a, str>,
        new_password: Option<Cow<'a, str>>,
    }

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: CodeSubmitBody = serde_urlencoded::from_bytes(&body)?;

    if let Some(new_password) = body.new_password {
        let api_res = res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::post(format!(
                        "{}/api/unstable/forgot_password/keys/{}/reset",
                        ctx.backend_host,
                        urlencoding::encode(&body.key),
                    ))
                    .body(
                        serde_json::to_vec(&serde_json::json!({ "new_password": new_password }))?
                            .into(),
                    )?,
                    &req_parts.headers,
                    &cookies,
                )?)
                .await?,
        )
        .await;

        match api_res {
            Ok(_) => {
                let base_data = fetch_base_data(
                    &ctx.backend_host,
                    &ctx.http_client,
                    &req_parts.headers,
                    &cookies,
                )
                .await?;

                let lang = crate::get_lang_for_headers(&req_parts.headers);

                let title = lang.tr(&lang::forgot_password()).into_owned();

                Ok(html_response(render::html! {
                    <HTPage base_data={&base_data} lang={&lang} title={&title}>
                        <h1>{title.as_ref()}</h1>
                        <p>
                            {lang.tr(&lang::forgot_password_complete())}{" "}
                            <a href={"/login"}>{lang.tr(&lang::login())}</a>
                        </p>
                    </HTPage>
                }))
            }
            Err(crate::Error::RemoteError((_, message))) => {
                page_forgot_password_code_reset_inner(
                    &body.key,
                    ctx,
                    &req_parts.headers,
                    &cookies,
                    Some(message),
                )
                .await
            }
            Err(other) => Err(other),
        }
    } else {
        let api_res = res_to_error(
            ctx.http_client
                .request(for_client(
                    hyper::Request::get(format!(
                        "{}/api/unstable/forgot_password/keys/{}",
                        ctx.backend_host,
                        urlencoding::encode(&body.key),
                    ))
                    .body(Default::default())?,
                    &req_parts.headers,
                    &cookies,
                )?)
                .await?,
        )
        .await;

        match api_res {
            Ok(_) => {
                page_forgot_password_code_reset_inner(
                    &body.key,
                    ctx,
                    &req_parts.headers,
                    &cookies,
                    None,
                )
                .await
            }
            Err(crate::Error::RemoteError((_, message))) => {
                page_forgot_password_code_inner(ctx, &req_parts.headers, &cookies, Some(message))
                    .await
            }
            Err(other) => Err(other),
        }
    }
}

async fn handler_forgot_password_submit(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!(
                    "{}/api/unstable/forgot_password/keys",
                    ctx.backend_host,
                ))
                .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(hyper::header::LOCATION, "/forgot_password/code")
            .body("Request submitted.".into())?),
        Err(crate::Error::RemoteError((_, message))) => {
            page_forgot_password_inner(ctx, &req_parts.headers, &cookies, Some(message)).await
        }
        Err(other) => Err(other),
    }
}

pub fn route_forgot_password() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_forgot_password)
        .with_child(
            "code",
            crate::RouteNode::new()
                .with_handler_async("GET", page_forgot_password_code)
                .with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async("POST", handler_forgot_password_code_submit),
                ),
        )
        .with_child(
            "submit",
            crate::RouteNode::new().with_handler_async("POST", handler_forgot_password_submit),
        )
}
