use serde_derive::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use crate::components::{
    BoolCheckbox, ContentView, FlagItem, HTPage, MaybeFillInput, NotificationItem, PostItem,
    ThingItem,
};
use crate::query_types::{FlagListQuery, PostListQuery};
use crate::resp_types::{
    JustStringID, RespFlagInfo, RespInstanceInfo, RespList, RespNotification, RespPostListPost,
    RespThingInfo, RespUserInfo,
};
use crate::PageBaseData;

mod comments;
mod communities;
mod forgot_password;
mod posts;
mod r#static;

const COOKIE_AGE: u32 = 60 * 60 * 24 * 365;

#[derive(Deserialize)]
struct ReturnToParams<'a> {
    return_to: Option<Cow<'a, str>>,
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
    use std::convert::TryInto;

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

    let title = lang.tr("about_title", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <ContentView src={&api_res.description} />
            <p>
                {
                    lang.tr(
                        "about_versions",
                        Some(&fluent::fluent_args![
                            "hitide_version" => env!("CARGO_PKG_VERSION"),
                            "backend_name" => api_res.software.name,
                            "backend_version" => api_res.software.version
                        ])
                    )
                }
            </p>
            <h2>{lang.tr("about_what_is", None)}</h2>
            <p>
                {lang.tr("about_text1", None)}
                {" "}<a href={"https://activitypub.rocks"}>{"ActivityPub"}</a>{"."}
            </p>
            <p>
                {lang.tr("about_text2", None)}
                {" "}
                <a href={"https://sr.ht/~vpzom/lotide/"}>{lang.tr("about_sourcehut", None)}</a>{"."}
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

    let title = lang.tr("login", None);

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
                        <td><label for={"input_username"}>{lang.tr("username_prompt", None)}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_password"}>{lang.tr("password_prompt", None)}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                        </td>
                    </tr>
                </table>
                <button r#type={"submit"}>{lang.tr("login", None)}</button>
            </form>
            <br />
            <p>
                {lang.tr("or_start", None)}{" "}<a href={"/signup"}>{lang.tr("login_signup_link", None)}</a>
            </p>
            <p>
                <a href={"/forgot_password"}>{lang.tr("forgot_password", None)}</a>
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
            let title = lang.tr("lookup_title", None);
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
                                Some(render::rsx! { <p>{lang.tr("lookup_nothing", None)}</p> })
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

    let title = lang.tr("community_create", None);

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
            <form method={"POST"} action={"/new_community/submit"}>
                <div>
                    <label>
                        {lang.tr("name_prompt", None)}{" "}<MaybeFillInput values={&prev_values} r#type={"text"} name={"name"} required={true} id={"input_name"} />
                    </label>
                </div>
                <div>
                    <button r#type={"submit"}>{lang.tr("community_create_submit", None)}</button>
                </div>
            </form>
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

    let title = lang.tr("notifications", None);

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
                            Some(render::rsx! { <p>{lang.tr("nothing", None)}</p> })
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
    page_signup_inner(ctx, req.headers(), None, None).await
}

async fn page_signup_inner(
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::HeaderMap,
    display_error: Option<String>,
    prev_values: Option<&HashMap<Cow<'_, str>, serde_json::Value>>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let lang = crate::get_lang_for_headers(headers);
    let cookies = get_cookie_map_for_headers(headers)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, headers, &cookies).await?;

    let title = lang.tr("register", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            <form method={"POST"} action={"/signup/submit"}>
                <table>
                    <tr>
                        <td><label for={"input_username"}>{lang.tr("username_prompt", None)}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_password"}>{lang.tr("password_prompt", None)}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_email_address"}>{lang.tr("signup_email_address_prompt", None)}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"email"} name={"email_address"} required={false} id={"input_email_address"} />
                        </td>
                    </tr>
                </table>
                <button r#type={"submit"}>{lang.tr("register", None)}</button>
            </form>
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
            page_signup_inner(ctx, &req_parts.headers, Some(message), Some(&body)).await
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
                            {lang.tr("user_remote_note", None)}
                            {" "}
                            <a href={remote_url.as_ref()}>{lang.tr("view_at_source", None)}{" ↗"}</a>
                        </div>
                    })
                } else {
                    None // shouldn't ever happen
                }
            }
            {
                if base_data.is_site_admin() && user.as_ref().local {
                    Some(render::rsx! {
                        <>
                            {
                                if user.suspended == Some(true) {
                                    Some(render::rsx! {
                                        <div class={"infoBox"}>
                                            {lang.tr("user_suspended_note", None)}
                                            {" "}
                                            <form method={"POST"} action={format!("/users/{}/suspend/undo", user_id)} class={"inline"}>
                                                <button type={"submit"}>{lang.tr("user_suspend_undo", None)}</button>
                                            </form>
                                        </div>
                                    })
                                } else {
                                    None
                                }
                            }
                            {
                                if user.suspended == Some(false) {
                                    Some(render::rsx! {
                                        <div>
                                            <a href={format!("/users/{}/suspend", user_id)}>{lang.tr("user_suspend", None)}</a>
                                            </div>
                                    })
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
                            {lang.tr("your_note", None)}{" ("}<a href={format!("/users/{}/your_note/edit", user_id)}>{lang.tr("edit", None)}</a>{"):"}
                            <pre>{your_note.content_text.as_ref()}</pre>
                        </div>
                    }
                })
            }
            {
                if user.your_note.is_none() && base_data.login.is_some() {
                    Some(render::rsx! {
                        <div>
                            <a href={format!("/users/{}/your_note/edit", user_id)}>{lang.tr("your_note_add", None)}</a>
                        </div>
                    })
                } else {
                    None
                }
            }
            {
                if let Some(login) = &base_data.login {
                    if login.user.id == user_id {
                        Some(render::rsx! { <a href={format!("/users/{}/edit", user_id)}>{lang.tr("edit", None)}</a> })
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
                    Some(render::rsx! { <p>{lang.tr("nothing", None)}</p> })
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

    let title = lang.tr("user_edit_title", None);

    let is_me = match &base_data.login {
        None => false,
        Some(login) => login.user.id == user_id,
    };

    if !is_me {
        let mut res = html_response(render::html! {
            <HTPage base_data={&base_data} lang={&lang} title={&title}>
                <h1>{title.as_ref()}</h1>
                <div class={"errorBox"}>{lang.tr("user_edit_not_you", None)}</div>
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
                        {lang.tr("user_edit_description_prompt", None)}<br />
                        <textarea name={"description_markdown"}>{user.description.content_markdown.as_deref().or(user.description.content_html.as_deref()).or(user.description.content_text.as_deref()).unwrap()}</textarea>
                    </label>
                </div>
                <div>
                    <label>
                        {lang.tr("user_edit_password_prompt", None)}<br />
                        <input name={"password"} type={"password"} value={""} autocomplete={"new-password"} />
                    </label>
                </div>
                <div>
                    <label>
                        <BoolCheckbox name={"is_bot"} value={user.base.is_bot} />
                        {lang.tr("user_edit_is_bot_checkbox_label", None)}<br />
                    </label>
                </div>
                <button type={"submit"}>{lang.tr("user_edit_submit", None)}</button>
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

    let title = lang.tr("user_suspend_title", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <p>
                {lang.tr("user_suspend_question", None)}
            </p>
            <form method={"POST"} action={format!("/users/{}/suspend/submit", user_id)}>
                <a href={format!("/users/{}", user_id)}>{lang.tr("no_cancel", None)}</a>
                {" "}
                <button type={"submit"}>{lang.tr("user_suspend_yes", None)}</button>
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

    let title = lang.tr("your_note_edit", None);

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} lang={&lang} title={&title}>
            <h1>{title.as_ref()}</h1>
            <form method={"POST"} action={format!("/users/{}/your_note/edit/submit", user_id)}>
                <div>
                    <textarea name={"content_text"} autofocus={""}>
                        {user.your_note.map(|x| x.content_text)}
                    </textarea>
                </div>
                <button type={"submit"}>{lang.tr("save", None)}</button>
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
                            {lang.tr("nothing", None)}
                            {" "}
                            {lang.tr("home_follow_prompt1", None)}
                            {" "}
                            <a href={"/communities"}>{lang.tr("home_follow_prompt2", None)}</a>
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
                            {lang.tr("posts_page_next", None)}
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
            <h1>{lang.tr("all_title", None)}</h1>
            {
                if api_res.items.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {lang.tr("nothing_yet", None)}
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
                            {lang.tr("posts_page_next", None)}
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
        } => lang.tr("flags_title_site_admin", None),
        Query {
            to_this_site_admin: None,
            to_community: Some(_),
        } => lang.tr("flags_title_community", None),
        _ => lang.tr("flags_title_other", None),
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
                                    {lang.tr("nothing", None)}
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
            <h1>{lang.tr("local_title", None)}</h1>
            {
                if api_res.items.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {lang.tr("nothing_yet", None)}
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
                            {lang.tr("posts_page_next", None)}
                        </a>
                    }
                })
            }
        </HTPage>
    }))
}

pub fn route_root() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_home)
        .with_child(
            "about",
            crate::RouteNode::new().with_handler_async("GET", page_about),
        )
        .with_child(
            "all",
            crate::RouteNode::new().with_handler_async("GET", page_all),
        )
        .with_child("comments", comments::route_comments())
        .with_child("communities", communities::route_communities())
        .with_child(
            "flags",
            crate::RouteNode::new().with_handler_async("GET", page_flags),
        )
        .with_child("forgot_password", forgot_password::route_forgot_password())
        .with_child(
            "local",
            crate::RouteNode::new().with_handler_async("GET", page_local),
        )
        .with_child(
            "login",
            crate::RouteNode::new()
                .with_handler_async("GET", page_login)
                .with_child(
                    "submit",
                    crate::RouteNode::new().with_handler_async("POST", handler_login_submit),
                ),
        )
        .with_child(
            "logout",
            crate::RouteNode::new().with_handler_async("POST", handler_logout),
        )
        .with_child(
            "lookup",
            crate::RouteNode::new().with_handler_async("GET", page_lookup),
        )
        .with_child(
            "new_community",
            crate::RouteNode::new()
                .with_handler_async("GET", page_new_community)
                .with_child(
                    "submit",
                    crate::RouteNode::new()
                        .with_handler_async("POST", handler_new_community_submit),
                ),
        )
        .with_child(
            "notifications",
            crate::RouteNode::new().with_handler_async("GET", page_notifications),
        )
        .with_child("posts", posts::route_posts())
        .with_child(
            "signup",
            crate::RouteNode::new()
                .with_handler_async("GET", page_signup)
                .with_child(
                    "submit",
                    crate::RouteNode::new().with_handler_async("POST", handler_signup_submit),
                ),
        )
        .with_child("static", r#static::route_static())
        .with_child(
            "users",
            crate::RouteNode::new().with_child_parse::<i64, _>(
                crate::RouteNode::new()
                    .with_handler_async("GET", page_user)
                    .with_child(
                        "edit",
                        crate::RouteNode::new()
                            .with_handler_async("GET", page_user_edit)
                            .with_child(
                                "submit",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_user_edit_submit),
                            ),
                    )
                    .with_child(
                        "suspend",
                        crate::RouteNode::new()
                            .with_handler_async("GET", page_user_suspend)
                            .with_child(
                                "submit",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_user_suspend_submit),
                            )
                            .with_child(
                                "undo",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_user_suspend_undo),
                            ),
                    )
                    .with_child(
                        "your_note/edit",
                        crate::RouteNode::new()
                            .with_handler_async("GET", page_user_your_note_edit)
                            .with_child(
                                "submit",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_user_your_note_edit_submit),
                            ),
                    ),
            ),
        )
}
