use serde_derive::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;

use crate::components::{
    BoolCheckbox, ContentView, FlagItem, HTPage, MaybeFillInput, NotificationItem, PostItem,
    SiteModlogEventItem, ThingItem,
};
use crate::lang;
use crate::query_types::{FlagListQuery, PostListQuery};
use crate::resp_types::{
    InvitationsCreateResponse, JustStringID, RespFlagInfo, RespInstanceInfo, RespInvitationInfo,
    RespList, RespNotification, RespPostListPost, RespSiteModlogEvent, RespThingInfo, RespUserInfo,
};
use crate::PageBaseData;

mod comments;
mod communities;
mod forgot_password;
mod moderation;
mod posts;
mod r#static;

const COOKIE_AGE: u32 = 60 * 60 * 24 * 365;

#[derive(Deserialize)]
struct ReturnToParams<'a> {
    return_to: Option<Cow<'a, str>>,
}

#[derive(Deserialize, Serialize)]
struct SignupQuery<'a> {
    invitation_key: Option<Cow<'a, str>>,
}

type CookieMap<'a> = std::collections::HashMap<&'a str, ginger::Cookie<'a>>;

fn get_cookie_map(src: Option<&str>) -> Result<CookieMap, ginger::ParseError> {
    use fallible_iterator::FallibleIterator;

    src.map(|s| {
        fallible_iterator::convert(ginger::parse_cookies(s))
            .map(|cookie| Ok((cookie.name, cookie)))
            .collect()
    })
    .unwrap_or_else(|| Ok(Default::default()))
}

fn get_cookie_map_for_req(req: &hyper::Request<hyper::Body>) -> Result<CookieMap, crate::Error> {
    get_cookie_map_for_headers(req.headers())
}

fn get_cookie_map_for_headers(headers: &hyper::HeaderMap) -> Result<CookieMap, crate::Error> {
    get_cookie_map(get_cookies_string(headers)?).map_err(Into::into)
}

fn get_cookies_string(headers: &hyper::HeaderMap) -> Result<Option<&str>, crate::Error> {
    Ok(headers
        .get(hyper::header::COOKIE)
        .map(|x| x.to_str())
        .transpose()?)
}

fn for_client(
    mut new_req: hyper::Request<hyper::Body>,
    src_headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
) -> Result<hyper::Request<hyper::Body>, hyper::header::InvalidHeaderValue> {
    let token = cookies.get("hitideToken").map(|c| c.value);
    if let Some(token) = token {
        new_req.headers_mut().insert(
            hyper::header::AUTHORIZATION,
            hyper::header::HeaderValue::from_str(&format!("Bearer {}", token))?,
        );
    }
    if let Some(value) = src_headers.get(hyper::header::ACCEPT_LANGUAGE) {
        new_req
            .headers_mut()
            .insert(hyper::header::ACCEPT_LANGUAGE, value.clone());
    }

    Ok(new_req)
}

async fn fetch_base_data(
    backend_host: &str,
    http_client: &crate::HttpClient,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
) -> Result<PageBaseData, crate::Error> {
    let login = {
        let api_res = http_client
            .request(for_client(
                hyper::Request::get(format!("{}/api/unstable/logins/~current", backend_host))
                    .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?;

        if api_res.status() == hyper::StatusCode::UNAUTHORIZED {
            Ok(None)
        } else {
            let api_res = res_to_error(api_res).await?;
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            serde_json::from_slice(&api_res)
        }
    }?;

    Ok(PageBaseData { login })
}

fn html_response(html: String) -> hyper::Response<hyper::Body> {
    let mut res = hyper::Response::new(html.into());
    res.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html"),
    );
    res
}

pub fn default_comments_sort() -> crate::SortType {
    crate::SortType::Hot
}

async fn page_about(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

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

    let title = lang.tr(&lang::ABOUT_TITLE);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ContentView src={&api_res.description} />
            <p>
                {
                    lang.tr(
                        &lang::about_versions(
                            env!("CARGO_PKG_VERSION"),
                            api_res.software.name,
                            api_res.software.version
                        )
                    )
                }
            </p>
            <p>
                <a href={"/modlog"}>{lang.tr(&lang::modlog_site())}</a>
            </p>
            <h2>{lang.tr(&lang::about_what_is())}</h2>
            <p>
                {lang.tr(&lang::about_text1())}
                {" "}<a href={"https://activitypub.rocks"}>{"ActivityPub"}</a>{"."}
            </p>
            <p>
                {lang.tr(&lang::about_text2())}
                {" "}
                <a href={"https://sr.ht/~vpzom/lotide/"}>{lang.tr(&lang::about_sourcehut())}</a>{"."}
            </p>
        </HTPage>
    }))
}

async fn page_login(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    page_login_inner(ctx, req.into_parts().0, None, None).await
}

async fn page_login_inner(
    ctx: Arc<crate::RouteContext>,
    req_parts: http::request::Parts,
    display_error: Option<String>,
    prev_values: Option<&serde_json::Value>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(&req_parts.headers);
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let base_data = fetch_base_data(
        &ctx.backend_host,
        &ctx.http_client,
        &req_parts.headers,
        &cookies,
    )
    .await?;

    let title = lang.tr(&lang::LOGIN);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            <form method={"POST"} action={"/login/submit"}>
                <table>
                    <tr>
                        <td><label for={"input_username"}>{lang.tr(&lang::username_prompt())}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_password"}>{lang.tr(&lang::password_prompt())}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                        </td>
                    </tr>
                </table>
                <button r#type={"submit"}>{lang.tr(&lang::login())}</button>
            </form>
            <br />
            <p>
                {lang.tr(&lang::or_start())}{" "}<a href={"/signup"}>{lang.tr(&lang::login_signup_link())}</a>
            </p>
            <p>
                <a href={"/forgot_password"}>{lang.tr(&lang::forgot_password())}</a>
            </p>
        </HTPage>
    }))
}

pub async fn res_to_error(
    res: hyper::Response<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let status = res.status();
    if status.is_success() {
        Ok(res)
    } else {
        let bytes = hyper::body::to_bytes(res.into_body()).await?;
        Err(crate::Error::RemoteError((
            status,
            String::from_utf8_lossy(&bytes).into_owned(),
        )))
    }
}

async fn handler_login_submit(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct LoginsCreateResponse<'a> {
        token: &'a str,
    }

    let (req_parts, body) = req.into_parts();

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::post(format!("{}/api/unstable/logins", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
            )
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: LoginsCreateResponse = serde_json::from_slice(&api_res)?;

            let token = api_res.token;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(
                    hyper::header::SET_COOKIE,
                    format!("hitideToken={}; Path=/; Max-Age={}", token, COOKIE_AGE),
                )
                .header(hyper::header::LOCATION, "/")
                .body("Successfully logged in.".into())?)
        }
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_login_inner(ctx, req_parts, Some(message), Some(&body)).await
        }
        Err(other) => Err(other),
    }
}

async fn handler_logout(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::delete(format!(
                    "{}/api/unstable/logins/~current",
                    ctx.backend_host,
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
        .header(hyper::header::LOCATION, "/")
        .header(
            hyper::header::SET_COOKIE,
            "hitideToken=\"\"; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT".to_owned(),
        )
        .body("Successfully logged out.".into())?)
}

async fn page_lookup(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;
    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    #[derive(Deserialize)]
    struct LookupQuery<'a> {
        query: Option<Cow<'a, str>>,
    }

    let query: LookupQuery<'_> = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;
    let query = query.query;

    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum ActorType {
        Community,
        User,
        #[serde(other)]
        Unknown,
    }

    #[derive(Deserialize)]
    struct LookupResult {
        id: i64,
        #[serde(rename = "type")]
        kind: ActorType,
    }

    let api_res: Option<Result<Vec<LookupResult>, String>> = if let Some(query) = &query {
        let api_res = res_to_error(
            ctx.http_client
                .request(
                    hyper::Request::get(format!(
                        "{}/api/unstable/actors:lookup/{}",
                        ctx.backend_host,
                        urlencoding::encode(query)
                    ))
                    .body(Default::default())?,
                )
                .await?,
        )
        .await;

        Some(match api_res {
            Ok(api_res) => {
                let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
                Ok(serde_json::from_slice(&api_res)?)
            }
            Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
                Err(message)
            }
            Err(other) => return Err(other),
        })
    } else {
        None
    };

    match api_res {
        Some(Ok(items)) if !items.is_empty() => {
            let item = &items[0];
            let dest = match item.kind {
                ActorType::Community => format!("/communities/{}", item.id),
                ActorType::User => format!("/users/{}", item.id),
                ActorType::Unknown => {
                    return Err(crate::Error::InternalStr(
                        "Unknown actor type received from lookup".to_owned(),
                    ));
                }
            };
            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::FOUND)
                .header(hyper::header::LOCATION, dest)
                .body("Redirecting…".into())?)
        }
        api_res => {
            let title = lang.tr(&lang::LOOKUP_TITLE);
            Ok(html_response(render::html! {
                <HTPage base_data={&base_data} lang={&lang} title={&title}>
                    <h1>{title.as_ref()}</h1>
                    <form method={"GET"} action={"/lookup"}>
                        <input r#type={"text"} name={"query"} value={query.as_deref().unwrap_or("")} />
                    </form>
                    {
                        match api_res {
                            None => None,
                            Some(Ok(_)) => {
                                // non-empty case is handled above
                                Some(render::rsx! { <p>{lang.tr(&lang::LOOKUP_NOTHING)}</p> })
                            },
                            Some(Err(display_error)) => {
                                Some(render::rsx! {
                                    <div class={"errorBox"}>{display_error.into()}</div>
                                })
                            }
                        }
                    }
                </HTPage>
            }))
        }
    }
}

async fn page_modlog(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/instance/modlog/events",
                    ctx.backend_host,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespList<RespSiteModlogEvent> = serde_json::from_slice(&api_res)?;

    let title = lang.tr(&lang::MODLOG_SITE);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ul>
                {
                    api_res.items.iter().map(|event| {
                        render::rsx! {
                            <SiteModlogEventItem event={event} lang={&lang} />
                        }
                    })
                    .collect::<Vec<_>>()
                }
            </ul>
        </HTPage>
    }))
}

async fn page_my_invitations(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    page_my_invitations_inner(ctx, req.headers(), &cookies, None).await
}

async fn page_my_invitations_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    res: Option<Result<InvitationsCreateResponse<'_>, &str>>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, &cookies).await?;

    let can_create_result = match &base_data.login {
        None => Err(lang.tr(&lang::MUST_LOGIN)),
        Some(login) => {
            if login.permissions.create_invitation.allowed {
                Ok(())
            } else {
                Err(lang.tr(&lang::MISSING_PERMISSION_CREATE_INVITATION))
            }
        }
    };

    let title = lang.tr(&lang::MY_INVITATIONS);

    match can_create_result {
        Ok(()) => Ok(html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <form method={"POST"} action={"/my_invitations/create"}>
                    <button type={"submit"}>{lang.tr(&lang::CREATE_INVITATION)}</button>
                </form>
                <br />
                {
                    if let Some(Ok(res)) = &res {
                        let url = {
                            let mut url = ctx.frontend_url.clone();
                            url.path_segments_mut().unwrap().push("signup");
                            url.set_query(Some(&format!("invitation_key={}", res.key)));
                            url
                        };

                        Some(render::rsx! {
                            <div>
                                <p>{lang.tr(&lang::CREATE_INVITATION_RESULT)}</p>
                                <input type={"text"} readonly={""} value={String::from(url)} />
                            </div>
                        })
                    } else {
                        None
                    }
                }
                {
                    if let Some(Err(message)) = res {
                        Some(render::rsx! {
                            <div class={"errorBox"}>
                                {message}
                            </div>
                        })
                    } else {
                        None
                    }
                }
            </HTPage>
        })),
        Err(err) => Ok(html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <div class={"errorBox"}>{err}</div>
            </HTPage>
        })),
    }
}

async fn handler_my_invitations_create(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!("{}/api/unstable/invitations", ctx.backend_host))
                    .body(Default::default())?,
                &req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: InvitationsCreateResponse = serde_json::from_slice(&api_res)?;

            page_my_invitations_inner(ctx, req.headers(), &cookies, Some(Ok(api_res))).await
        }
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_my_invitations_inner(ctx, req.headers(), &cookies, Some(Err(&message))).await
        }
        Err(other) => Err(other),
    }
}

async fn page_new_community(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    page_new_community_inner(ctx, req.headers(), &cookies, None, None).await
}

async fn page_new_community_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
    prev_values: Option<&serde_json::Value>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, cookies).await?;

    let title = lang.tr(&lang::COMMUNITY_CREATE);

    let not_allowed = base_data.login.is_some()
        && !base_data
            .login
            .as_ref()
            .unwrap()
            .permissions
            .create_community
            .allowed;

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
            {
                not_allowed.then(|| {
                    render::rsx! {
                        <div class={"errorBox"}>{lang.tr(&lang::COMMUNITY_CREATE_NOT_ALLOWED)}</div>
                    }
                })
            }
            {
                (!not_allowed).then(|| {
                    render::rsx! {
                        <form method={"POST"} action={"/new_community/submit"}>
                            <div>
                                <label>
                                    {lang.tr(&lang::NAME_PROMPT)}{" "}<MaybeFillInput values={&prev_values} r#type={"text"} name={"name"} required={true} id={"input_name"} />
                                </label>
                            </div>
                            <div>
                                <button r#type={"submit"}>{lang.tr(&lang::COMMUNITY_CREATE_SUBMIT)}</button>
                            </div>
                        </form>
                    }
                })
            }
        </HTPage>
    }))
}

async fn handler_new_community_submit(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    #[derive(Deserialize)]
    struct CommunitiesCreateResponseCommunity {
        id: i64,
    }

    #[derive(Deserialize)]
    struct CommunitiesCreateResponse {
        community: CommunitiesCreateResponseCommunity,
    }

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::post(format!("{}/api/unstable/communities", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: CommunitiesCreateResponse = serde_json::from_slice(&api_res)?;

            let community_id = api_res.community.id;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(
                    hyper::header::LOCATION,
                    format!("/communities/{}", community_id),
                )
                .body("Successfully created.".into())?)
        }
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_new_community_inner(
                ctx,
                &req_parts.headers,
                &cookies,
                Some(message),
                Some(&body),
            )
            .await
        }
        Err(other) => Err(other),
    }
}

async fn page_notifications(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    use futures_util::future::TryFutureExt;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let api_res: Result<Result<RespList<RespNotification>, _>, _> = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/~me/notifications",
                    ctx.backend_host
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .map_err(crate::Error::from)
    .and_then(|body| hyper::body::to_bytes(body).map_err(crate::Error::from))
    .await
    .map(|body| serde_json::from_slice(&body));

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let title = lang.tr(&lang::NOTIFICATIONS);

    match api_res {
        Err(crate::Error::RemoteError((_, message))) => {
            let mut res = html_response(render::html! {
                <HTPage base_data={&base_data} lang={&lang} title={&title}>
                    <h1>{title.as_ref()}</h1>
                    <div class={"errorBox"}>{message}</div>
                </HTPage>
            });

            *res.status_mut() = hyper::StatusCode::FORBIDDEN;

            Ok(res)
        }
        Err(other) => Err(other),
        Ok(api_res) => {
            let notifications = api_res?.items;

            Ok(html_response(render::html! {
                <HTPage base_data={&base_data} lang={&lang} title={&title}>
                    <h1>{title.as_ref()}</h1>
                    {
                        if notifications.is_empty() {
                            Some(render::rsx! { <p>{lang.tr(&lang::NOTHING)}</p> })
                        } else {
                            None
                        }
                    }
                    <ul>
                        {
                            notifications.iter()
                                .map(|item| render::rsx! { <NotificationItem notification={item} lang={&lang} /> })
                                .collect::<Vec<_>>()
                        }
                    </ul>
                </HTPage>
            }))
        }
    }
}

async fn page_signup(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let query: SignupQuery = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    page_signup_inner(ctx, req.headers(), query, None, None).await
}

async fn page_signup_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::HeaderMap,
    query: SignupQuery<'_>,
    display_error: Option<String>,
    prev_values: Option<&HashMap<Cow<'_, str>, serde_json::Value>>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let cookies = get_cookie_map_for_headers(headers)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, &cookies).await?;

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
    let instance_info: RespInstanceInfo = serde_json::from_slice(&api_res)?;

    let can_signup_res = {
        if let Some(invitation_key) = &query.invitation_key {
            let api_res = res_to_error(
                ctx.http_client
                    .get(
                        format!(
                            "{}/api/unstable/invitations?{}",
                            ctx.backend_host,
                            serde_urlencoded::to_string(&serde_json::json!({
                                "key": invitation_key
                            }))
                            .unwrap()
                        )
                        .try_into()
                        .unwrap(),
                    )
                    .await?,
            )
            .await?;
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: RespList<RespInvitationInfo> = serde_json::from_slice(&api_res)?;

            if let Some(info) = api_res.items.first() {
                if info.used {
                    Err(lang.tr(&lang::INVITATION_ALREADY_USED))
                } else {
                    Ok(())
                }
            } else {
                if instance_info.signup_allowed {
                    Ok(())
                } else {
                    Err(lang.tr(&lang::NO_SUCH_INVITATION))
                }
            }
        } else {
            if instance_info.signup_allowed {
                Ok(())
            } else {
                Err(lang.tr(&lang::SIGNUP_NOT_ALLOWED))
            }
        }
    };

    let title = lang.tr(&lang::REGISTER);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            {
                can_signup_res.as_ref().err().map(|err| render::rsx! {
                    <div class={"errorBox"}>{err.as_ref()}</div>
                })
            }
            {
                can_signup_res.is_ok().then(|| render::rsx! {
                    <form method={"POST"} action={"/signup/submit"}>
                        {
                            query.invitation_key.map(|invitation_key| {
                                render::rsx! {
                                    <input type={"hidden"} name={"invitation_key"} value={invitation_key} />
                                }
                            })
                        }
                        <table>
                            <tr>
                                <td><label for={"input_username"}>{lang.tr(&lang::USERNAME_PROMPT)}</label></td>
                                <td>
                                    <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                                </td>
                            </tr>
                            <tr>
                                <td><label for={"input_password"}>{lang.tr(&lang::PASSWORD_PROMPT)}</label></td>
                                <td>
                                    <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                                </td>
                            </tr>
                            <tr>
                                <td><label for={"input_email_address"}>{lang.tr(&lang::SIGNUP_EMAIL_ADDRESS_PROMPT)}</label></td>
                                <td>
                                    <MaybeFillInput values={&prev_values} r#type={"email"} name={"email_address"} required={false} id={"input_email_address"} />
                                </td>
                            </tr>
                        </table>
                        <button r#type={"submit"}>{lang.tr(&lang::REGISTER)}</button>
                    </form>
                })
            }
        </HTPage>
    }))
}

async fn handler_signup_submit(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct UsersCreateResponse<'a> {
        token: &'a str,
    }

    let (req_parts, body) = req.into_parts();

    let body = hyper::body::to_bytes(body).await?;
    let mut body: HashMap<Cow<'_, str>, serde_json::Value> = serde_urlencoded::from_bytes(&body)?;
    body.insert("login".into(), true.into());
    if body.get("email_address").and_then(|x| x.as_str()) == Some("") {
        body.remove("email_address");
    }

    let invitation_key = if let Some(key) = body.get("invitation_key") {
        match key.as_str() {
            Some("") => {
                body.remove("invitation_key");

                None
            }
            Some(key) => Some(key),
            None => {
                return Err(crate::Error::UserError({
                    let mut res =
                        hyper::Response::new("Invalid value type for invitation_key".into());
                    *res.status_mut() = hyper::StatusCode::BAD_REQUEST;
                    res
                }));
            }
        }
    } else {
        None
    };

    let query = SignupQuery {
        invitation_key: invitation_key.map(Cow::Borrowed),
    };

    let api_res = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::post(format!("{}/api/unstable/users", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
            )
            .await?,
    )
    .await;

    match api_res {
        Ok(api_res) => {
            let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
            let api_res: UsersCreateResponse = serde_json::from_slice(&api_res)?;

            let token = api_res.token;

            Ok(hyper::Response::builder()
                .status(hyper::StatusCode::SEE_OTHER)
                .header(
                    hyper::header::SET_COOKIE,
                    format!("hitideToken={}; Path=/; Max-Age={}", token, COOKIE_AGE),
                )
                .header(hyper::header::LOCATION, "/")
                .body("Successfully registered new account.".into())?)
        }
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_signup_inner(ctx, &req_parts.headers, query, Some(message), Some(&body)).await
        }
        Err(other) => Err(other),
    }
}

async fn page_user(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let user = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/{}{}",
                    ctx.backend_host,
                    user_id,
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
    let user = hyper::body::to_bytes(user.into_body()).await?;
    let user: RespUserInfo<'_> = serde_json::from_slice(&user)?;

    let things = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/{}/things",
                    ctx.backend_host, user_id,
                ))
                .body(Default::default())?,
            )
            .await?,
    )
    .await?;
    let things = hyper::body::to_bytes(things.into_body()).await?;
    let things: RespList<RespThingInfo> = serde_json::from_slice(&things)?;

    let title = user.as_ref().username.as_ref();

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title>
            <h1>{title}</h1>
            <p><em>{format!("@{}@{}", user.as_ref().username, user.as_ref().host)}</em></p>
            {
                if user.as_ref().local {
                    None
                } else if let Some(remote_url) = &user.as_ref().remote_url {
                    Some(render::rsx! {
                        <div class={"infoBox"}>
                            {lang.tr(&lang::USER_REMOTE_NOTE)}
                            {" "}
                            <a href={remote_url.as_ref()}>{lang.tr(&lang::VIEW_AT_SOURCE)}{" ↗"}</a>
                        </div>
                    })
                } else {
                    None // shouldn't ever happen
                }
            }
            {
                if user.as_ref().local {
                    Some(render::rsx! {
                        <>
                            {
                                if user.suspended == Some(true) {
                                    Some(render::rsx! {
                                        <div class={"infoBox"}>
                                            {lang.tr(&lang::USER_SUSPENDED_NOTE)}
                                            {" "}
                                            {
                                                base_data.is_site_admin().then(|| {
                                                    render::rsx! {
                                                        <form method={"POST"} action={format!("/users/{}/suspend/undo", user_id)} class={"inline"}>
                                                            <button type={"submit"}>{lang.tr(&lang::USER_SUSPEND_UNDO)}</button>
                                                        </form>
                                                    }
                                                })
                                            }
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }
                            {
                                if user.suspended == Some(false) {
                                    if base_data.is_site_admin() {
                                        Some(render::rsx! {
                                            <div>
                                                <a href={format!("/users/{}/suspend", user_id)}>{lang.tr(&lang::USER_SUSPEND)}</a>
                                            </div>
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            }
                        </>
                    })
                } else {
                    None
                }
            }
            {
                user.your_note.as_ref().map(|your_note| {
                    render::rsx! {
                        <div>
                            {lang.tr(&lang::YOUR_NOTE)}{" ("}<a href={format!("/users/{}/your_note/edit", user_id)}>{lang.tr(&lang::EDIT)}</a>{"):"}
                            <pre>{your_note.content_text.as_ref()}</pre>
                        </div>
                    }
                })
            }
            {
                if user.your_note.is_none() && base_data.login.is_some() {
                    Some(render::rsx! {
                        <div>
                            <a href={format!("/users/{}/your_note/edit", user_id)}>{lang.tr(&lang::YOUR_NOTE_ADD)}</a>
                        </div>
                    })
                } else {
                    None
                }
            }
            {
                if let Some(login) = &base_data.login {
                    if login.user.id == user_id {
                        Some(render::rsx! {
                            <>
                                <div>
                                    <a href={format!("/users/{}/edit", user_id)}>{lang.tr(&lang::EDIT)}</a>
                                </div>
                                {
                                    login.permissions.create_invitation.allowed.then(|| {
                                        render::rsx! {
                                            <div>
                                                <a href={"/my_invitations"}>{lang.tr(&lang::INVITE_USERS)}</a>
                                            </div>
                                        }
                                    })
                                }
                            </>
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            <ContentView src={&user.description} />
            {
                if things.items.is_empty() {
                    Some(render::rsx! { <p>{lang.tr(&lang::NOTHING)}</p> })
                } else {
                    None
                }
            }
            <ul>
                {
                    things.items.iter().map(|thing| {
                        ThingItem { thing, lang: &lang }
                    })
                    .collect::<Vec<_>>()
                }
            </ul>
        </HTPage>
    }))
}

async fn page_user_edit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let title = lang.tr(&lang::USER_EDIT_TITLE);

    let is_me = match &base_data.login {
        None => false,
        Some(login) => login.user.id == user_id,
    };

    if !is_me {
        let mut res = html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <div class={"errorBox"}>{lang.tr(&lang::user_edit_not_you())}</div>
            </HTPage>
        });

        *res.status_mut() = hyper::StatusCode::FORBIDDEN;

        return Ok(res);
    }

    let user = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/{}",
                    ctx.backend_host, user_id
                ))
                .body(Default::default())?,
            )
            .await?,
    )
    .await?;
    let user = hyper::body::to_bytes(user.into_body()).await?;
    let user: RespUserInfo<'_> = serde_json::from_slice(&user)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={format!("/users/{}/edit/submit", user_id)}>
                <div>
                    <label>
                        {lang.tr(&lang::user_edit_description_prompt())}<br />
                        <textarea name={"description_markdown"}>{user.description.content_markdown.as_deref().or(user.description.content_html.as_deref()).or(user.description.content_text.as_deref()).unwrap()}</textarea>
                    </label>
                </div>
                <div>
                    <label>
                        {lang.tr(&lang::user_edit_password_prompt())}<br />
                        <input name={"password"} type={"password"} value={""} autocomplete={"new-password"} />
                    </label>
                </div>
                <div>
                    <label>
                        <BoolCheckbox name={"is_bot"} value={user.base.is_bot} />
                        {lang.tr(&lang::user_edit_is_bot_checkbox_label())}<br />
                    </label>
                </div>
                <button type={"submit"}>{lang.tr(&lang::user_edit_submit())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_user_edit_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let mut body: serde_json::map::Map<String, serde_json::Value> =
        serde_urlencoded::from_bytes(&body)?;

    // ignore password field if blank
    if let Some(password) = body.get("password") {
        if password == "" {
            body.remove("password");
        }
    }

    body.insert("is_bot".to_owned(), body.contains_key("is_bot").into());

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!("{}/api/unstable/users/~me", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/users/{}", user_id))
        .body("Successfully created.".into())?)
}

async fn page_user_suspend(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let title = lang.tr(&lang::USER_SUSPEND_TITLE);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <p>
                {lang.tr(&lang::user_suspend_question())}
            </p>
            <form method={"POST"} action={format!("/users/{}/suspend/submit", user_id)}>
                <a href={format!("/users/{}", user_id)}>{lang.tr(&lang::no_cancel())}</a>
                {" "}
                <button type={"submit"}>{lang.tr(&lang::user_suspend_yes())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_user_suspend_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/users/{}",
                    ctx.backend_host, user_id
                ))
                .body(r#"{"suspended":true}"#.into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/users/{}", user_id))
        .body("Successfully suspended.".into())?)
}

async fn handler_user_suspend_undo(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::patch(format!(
                    "{}/api/unstable/users/{}",
                    ctx.backend_host, user_id
                ))
                .body(r#"{"suspended":false}"#.into())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/users/{}", user_id))
        .body("Successfully unsuspended.".into())?)
}

async fn page_user_your_note_edit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let user = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/{}?include_your=true",
                    ctx.backend_host, user_id,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let user = hyper::body::to_bytes(user.into_body()).await?;
    let user: RespUserInfo<'_> = serde_json::from_slice(&user)?;

    let title = lang.tr(&lang::YOUR_NOTE_EDIT);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={format!("/users/{}/your_note/edit/submit", user_id)}>
                <div>
                    <textarea name={"content_text"} autofocus={""}>
                        {user.your_note.map(|x| x.content_text)}
                    </textarea>
                </div>
                <button type={"submit"}>{lang.tr(&lang::save())}</button>
            </form>
        </HTPage>
    }))
}

async fn handler_user_your_note_edit_submit(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (user_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::put(format!(
                    "{}/api/unstable/users/{}/your_note",
                    ctx.backend_host, user_id
                ))
                .body(serde_json::to_vec(&body)?.into())?,
                &req_parts.headers,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/users/{}", user_id))
        .body("Successfully created.".into())?)
}

async fn page_home(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_req(&req);
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    if base_data.login.is_none() {
        return page_all_inner(req.headers(), &cookies, &base_data, req.uri().query(), ctx).await;
    }

    #[derive(Deserialize)]
    struct Query<'a> {
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&PostListQuery {
                        in_your_follows: Some(true),
                        include_your: Some(true),
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

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespList<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={"lotide"}>
            {
                if api_res.items.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {lang.tr(&lang::NOTHING)}
                            {" "}
                            {lang.tr(&lang::HOME_FOLLOW_PROMPT1)}
                            {" "}
                            <a href={"/communities"}>{lang.tr(&lang::HOME_FOLLOW_PROMPT2)}</a>
                        </p>
                    })
                } else {
                    None
                }
            }
            <ul>
                {api_res.items.iter().map(|post| {
                    PostItem { post, in_community: false, no_user: false, lang: &lang }
                }).collect::<Vec<_>>()}
            </ul>
            {
                api_res.next_page.map(|next_page| {
                    render::rsx! {
                        <a href={format!("/?page={}", next_page)}>
                            {lang.tr(&lang::POSTS_PAGE_NEXT)}
                        </a>
                    }
                })
            }
        </HTPage>
    }))
}

async fn page_all(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    page_all_inner(req.headers(), &cookies, &base_data, req.uri().query(), ctx).await
}

async fn page_all_inner(
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    base_data: &crate::PageBaseData,
    query: Option<&str>,
    ctx: Arc<crate::RouteContext>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);

    #[derive(Deserialize)]
    struct Query<'a> {
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(query.unwrap_or(""))?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&PostListQuery {
                        use_aggregate_filters: Some(true),
                        page: query.page.as_deref(),
                        ..Default::default()
                    })?,
                ))
                .body(Default::default())?,
                headers,
                cookies,
            )?)
            .await?,
    )
    .await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespList<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={base_data} lang={&lang} title={"lotide"}>
            <h1>{lang.tr(&lang::all_title())}</h1>
            {
                if api_res.items.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {lang.tr(&lang::NOTHING_YET)}
                        </p>
                    })
                } else {
                    None
                }
            }
            <ul>
                {api_res.items.iter().map(|post| {
                    PostItem { post, in_community: false, no_user: false, lang: &lang }
                }).collect::<Vec<_>>()}
            </ul>
            {
                api_res.next_page.map(|next_page| {
                    render::rsx! {
                        <a href={format!("/all?page={}", next_page)}>
                            {lang.tr(&lang::POSTS_PAGE_NEXT)}
                        </a>
                    }
                })
            }
        </HTPage>
    }))
}

async fn page_flags(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    use futures_util::TryFutureExt;

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let lang = crate::get_lang_for_headers(req.headers());

    #[derive(Deserialize)]
    struct Query {
        to_this_site_admin: Option<bool>,
        to_community: Option<i64>,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/flags?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&FlagListQuery {
                        to_this_site_admin: query.to_this_site_admin,
                        to_community: query.to_community,
                    })?,
                ))
                .body(Default::default())?,
                req.headers(),
                &cookies,
            )?)
            .await?,
    )
    .map_err(crate::Error::from)
    .and_then(|api_res| hyper::body::to_bytes(api_res.into_body()).map_err(crate::Error::from))
    .await;

    let title = match query {
        Query {
            to_this_site_admin: Some(true),
            to_community: None,
        } => lang.tr(&lang::FLAGS_TITLE_SITE_ADMIN),
        Query {
            to_this_site_admin: None,
            to_community: Some(_),
        } => lang.tr(&lang::FLAGS_TITLE_COMMUNITY),
        _ => lang.tr(&lang::FLAGS_TITLE_OTHER),
    };

    match api_res {
        Err(crate::Error::RemoteError((status, message))) => {
            let mut res = html_response(render::html! {
                <HTPage base_data={&base_data} lang={&lang} title={&title}>
                    <h1>{title.as_ref()}</h1>
                    <div class={"errorBox"}>{message}</div>
                </HTPage>
            });

            *res.status_mut() = status;

            Ok(res)
        }
        Err(other) => Err(other),
        Ok(api_res) => {
            let api_res: RespList<RespFlagInfo<'_>> = serde_json::from_slice(&api_res)?;

            Ok(html_response(render::html! {
                <HTPage base_data={&base_data} lang={&lang} title={&title}>
                    <h1>{title.as_ref()}</h1>
                    {
                        if api_res.items.is_empty() {
                            Some(render::rsx! {
                                <p>
                                    {lang.tr(&lang::NOTHING)}
                                </p>
                            })
                        } else {
                            None
                        }
                    }
                    <ul>
                    {api_res.items.iter().map(|flag| {
                        FlagItem { flag, in_community: query.to_community.is_some(), lang: &lang }
                    }).collect::<Vec<_>>()}
                    </ul>
                </HTPage>
            }))
        }
    }
}

async fn page_local(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data =
        fetch_base_data(&ctx.backend_host, &ctx.http_client, req.headers(), &cookies).await?;

    let lang = crate::get_lang_for_headers(req.headers());

    #[derive(Deserialize)]
    struct Query<'a> {
        page: Option<Cow<'a, str>>,
    }

    let query: Query = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;

    let api_res = res_to_error(
        ctx.http_client
            .request(for_client(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts?{}",
                    ctx.backend_host,
                    serde_urlencoded::to_string(&PostListQuery {
                        use_aggregate_filters: Some(true),
                        in_any_local_community: Some(true),
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

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: RespList<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={"lotide"}>
            <h1>{lang.tr(&lang::local_title())}</h1>
            {
                if api_res.items.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {lang.tr(&lang::NOTHING_YET)}
                        </p>
                    })
                } else {
                    None
                }
            }
            <ul>
                {api_res.items.iter().map(|post| {
                    PostItem { post, in_community: false, no_user: false, lang: &lang }
                }).collect::<Vec<_>>()}
            </ul>
            {
                api_res.next_page.map(|next_page| {
                    render::rsx! {
                        <a href={format!("/local?page={}", next_page)}>
                            {lang.tr(&lang::POSTS_PAGE_NEXT)}
                        </a>
                    }
                })
            }
        </HTPage>
    }))
}

pub fn route_root() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async(hyper::Method::GET, page_home)
        .with_child(
            "about",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_about),
        )
        .with_child(
            "all",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_all),
        )
        .with_child("comments", comments::route_comments())
        .with_child("communities", communities::route_communities())
        .with_child(
            "flags",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_flags),
        )
        .with_child("forgot_password", forgot_password::route_forgot_password())
        .with_child(
            "local",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_local),
        )
        .with_child(
            "login",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_login)
                .with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_login_submit),
                ),
        )
        .with_child(
            "logout",
            crate::RouteNode::new().with_handler_async(hyper::Method::POST, handler_logout),
        )
        .with_child(
            "lookup",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_lookup),
        )
        .with_child("moderation", moderation::route_moderation())
        .with_child(
            "modlog",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_modlog),
        )
        .with_child(
            "my_invitations",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_my_invitations)
                .with_child(
                    "create",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_my_invitations_create),
                ),
        )
        .with_child(
            "new_community",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_new_community)
                .with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_new_community_submit),
                ),
        )
        .with_child(
            "notifications",
            crate::RouteNode::new().with_handler_async(hyper::Method::GET, page_notifications),
        )
        .with_child("posts", posts::route_posts())
        .with_child(
            "signup",
            crate::RouteNode::new()
                .with_handler_async(hyper::Method::GET, page_signup)
                .with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async(hyper::Method::POST, handler_signup_submit),
                ),
        )
        .with_child("static", r#static::route_static())
        .with_child(
            "users",
            crate::RouteNode::new().with_child_parse::<i64, _>(
                crate::RouteNode::new()
                    .with_handler_async(hyper::Method::GET, page_user)
                    .with_child(
                        "edit",
                        crate::RouteNode::new()
                            .with_handler_async(hyper::Method::GET, page_user_edit)
                            .with_child(
                                "submit",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_user_edit_submit,
                                ),
                            ),
                    )
                    .with_child(
                        "suspend",
                        crate::RouteNode::new()
                            .with_handler_async(hyper::Method::GET, page_user_suspend)
                            .with_child(
                                "submit",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_user_suspend_submit,
                                ),
                            )
                            .with_child(
                                "undo",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_user_suspend_undo,
                                ),
                            ),
                    )
                    .with_child(
                        "your_note/edit",
                        crate::RouteNode::new()
                            .with_handler_async(hyper::Method::GET, page_user_your_note_edit)
                            .with_child(
                                "submit",
                                crate::RouteNode::new().with_handler_async(
                                    hyper::Method::POST,
                                    handler_user_your_note_edit_submit,
                                ),
                            ),
                    ),
            ),
        )
}
