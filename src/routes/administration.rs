use super::{
    fetch_base_data, for_client, get_cookie_map_for_headers, get_cookie_map_for_req, html_response,
    res_to_error, CookieMap,
};
use crate::components::{HTPage, MaybeFillOption, MaybeFillTextArea};
use crate::lang;
use crate::resp_types::RespInstanceInfo;
use render::Render;
use std::borrow::Cow;
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
                    {
                        lang::TrElements::new(
                            lang.tr(&lang::administration_signup_allowed(lang::LangPlaceholder(0))),
                            |id, w| {
                                match id {
                                    0 => render::rsx! {
                                        <strong>{lang.tr(if api_res.signup_allowed {
                                            &lang::ALLOWED_TRUE
                                        } else {
                                            &lang::ALLOWED_FALSE
                                        })}</strong>
                                    }.render_into(w),
                                    _ => unreachable!(),
                                }
                            }
                        )
                    }
                </li>
                <li>
                    {
                        lang::TrElements::new(
                            lang.tr(&lang::administration_invitations_enabled(lang::LangPlaceholder(0))),
                            |id, w| {
                                match id {
                                    0 => render::rsx! {
                                        <strong>{lang.tr(if api_res.invitations_enabled {
                                            &lang::ENABLED_TRUE
                                        } else {
                                            &lang::ENABLED_FALSE
                                        })}</strong>
                                    }.render_into(w),
                                    _ => unreachable!(),
                                }
                            }
                        )
                    }
                    {
                        if api_res.invitations_enabled {
                            Some(render::rsx! {
                                <ul>
                                    <li>
                                        {lang.tr(&lang::ADMINISTRATION_INVITATION_CREATION_REQUIREMENT)}{" "}
                                        <strong>{lang.tr(match api_res.invitation_creation_requirement.as_deref() {
                                            None => &lang::REQUIREMENT_NONE,
                                            Some("site_admin") => &lang::REQUIREMENT_SITE_ADMIN,
                                            Some(_) => &lang::UNKNOWN,
                                        })}</strong>
                                    </li>
                                </ul>
                            })
                        } else {
                            None
                        }
                    }
                </li>
                <li>
                    {lang.tr(&lang::ADMINISTRATION_COMMUNITY_CREATION_REQUIREMENT)}{" "}
                    <strong>{lang.tr(match api_res.community_creation_requirement.as_deref() {
                        None => &lang::REQUIREMENT_NONE,
                        Some("site_admin") => &lang::REQUIREMENT_SITE_ADMIN,
                        Some(_) => &lang::UNKNOWN,
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
    prev_values: Option<&HashMap<Cow<'_, str>, serde_json::Value>>,
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
    let invitations_enabled_value = Some(crate::bool_as_str(api_res.invitations_enabled));
    let invitation_creation_requirement_value = Some(
        api_res
            .invitation_creation_requirement
            .as_deref()
            .unwrap_or(""),
    );
    let community_creation_requirement_value = Some(
        api_res
            .community_creation_requirement
            .as_deref()
            .unwrap_or(""),
    );

    let (description_content, description_format) = match api_res.description.content_markdown {
        Some(content) => (content, "markdown"),
        None => match api_res.description.content_html {
            Some(content) => (content, "html"),
            None => (api_res.description.content_text.unwrap(), "text"),
        },
    };

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
                <div>
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
                </div>
                <div>
                    <label>
                        {lang.tr(&lang::administration_edit_invitations_enabled())}<br />
                        <select name={"invitations_enabled"}>
                            <MaybeFillOption value={"true"} values={&prev_values} default_value={invitations_enabled_value} name={"invitations_enabled"}>
                                {lang.tr(&lang::enabled_true())}
                            </MaybeFillOption>
                            <MaybeFillOption value={"false"} values={&prev_values} default_value={invitations_enabled_value} name={"invitations_enabled"}>
                                {lang.tr(&lang::enabled_false())}
                            </MaybeFillOption>
                        </select>
                    </label>
                </div>
                <div>
                    <label>
                        {lang.tr(&lang::administration_invitation_creation_requirement())}{":"}<br />
                        <select name={"invitation_creation_requirement"}>
                            <MaybeFillOption value={""} values={&prev_values} default_value={invitation_creation_requirement_value} name={"invitation_creation_requirement"}>
                                {lang.tr(&lang::requirement_none())}
                            </MaybeFillOption>
                            <MaybeFillOption value={"site_admin"} values={&prev_values} default_value={invitation_creation_requirement_value} name={"invitation_creation_requirement"}>
                                {lang.tr(&lang::requirement_site_admin())}
                            </MaybeFillOption>
                        </select>
                    </label>
                </div>
                <div>
                    <label>
                        {lang.tr(&lang::administration_community_creation_requirement())}{":"}<br />
                        <select name={"community_creation_requirement"}>
                            <MaybeFillOption value={""} values={&prev_values} default_value={community_creation_requirement_value} name={"community_creation_requirement"}>
                                {lang.tr(&lang::requirement_none())}
                            </MaybeFillOption>
                            <MaybeFillOption value={"site_admin"} values={&prev_values} default_value={community_creation_requirement_value} name={"community_creation_requirement"}>
                                {lang.tr(&lang::requirement_site_admin())}
                            </MaybeFillOption>
                        </select>
                    </label>
                </div>
                <label>
                    {lang.tr(&lang::description())}
                    <br />
                    <MaybeFillTextArea values={&prev_values} name={"description"} default_value={Some(&description_content)} />
                    <br />
                    <select name={"description_format"}>
                        <MaybeFillOption value={"text"} values={&prev_values} default_value={Some(description_format)} name={"description_format"}>
                            {lang.tr(&lang::content_format_text())}
                        </MaybeFillOption>
                        <MaybeFillOption value={"markdown"} values={&prev_values} default_value={Some(description_format)} name={"description_format"}>
                            {lang.tr(&lang::content_format_markdown())}
                        </MaybeFillOption>
                        <MaybeFillOption value={"html"} values={&prev_values} default_value={Some(description_format)} name={"description_format"}>
                            {lang.tr(&lang::content_format_html())}
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
    let body_original: HashMap<Cow<'_, str>, serde_json::Value> =
        serde_urlencoded::from_bytes(&body)?;
    let mut body = body_original.clone();

    for key in ["signup_allowed", "invitations_enabled"] {
        body.insert(
            key.into(),
            body.get(key)
                .and_then(|x| x.as_str())
                .ok_or(crate::Error::InternalStrStatic(
                    "Failed to extract value in administration edit",
                ))?
                .parse()?,
        );
    }

    for key in [
        "invitation_creation_requirement",
        "community_creation_requirement",
    ] {
        if body.get(key).and_then(|x| x.as_str()) == Some("") {
            body.insert(key.into(), serde_json::Value::Null);
        }
    }

    if let Some(content) = body.remove("description") {
        let content = content.as_str().ok_or(crate::Error::InternalStrStatic(
            "Failed to extract description in administration edit",
        ))?;

        let format = body.remove("description_format");
        let format = match format.as_ref().and_then(|x| x.as_str()) {
            Some(format) => format,
            None => {
                return Err(crate::Error::InternalStrStatic(
                    "Invalid or missing description format",
                ))
            }
        };

        body.insert(format!("description_{}", format).into(), content.into());
    }

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
