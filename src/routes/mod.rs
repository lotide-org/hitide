use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

use crate::components::{
    Comment, Content, HTPage, MaybeFillInput, MaybeFillTextArea, PostItem, ThingItem, UserLink,
};
use crate::resp_types::{RespPostCommentInfo, RespPostListPost, RespThingInfo, RespUserInfo};
use crate::util::author_is_me;
use crate::PageBaseData;

mod communities;
mod posts;
mod r#static;

const COOKIE_AGE: u32 = 60 * 60 * 24 * 365;

#[derive(Deserialize)]
struct ReturnToParams<'a> {
    return_to: Option<Cow<'a, str>>,
}

type CookieMap<'a> = std::collections::HashMap<&'a str, ginger::Cookie<'a>>;

fn get_cookie_map(src: Option<&str>) -> Result<CookieMap, ginger::ParseError> {
    match src {
        None => Ok(Default::default()),
        Some(src) => {
            use fallible_iterator::FallibleIterator;

            fallible_iterator::convert(ginger::parse_cookies(src))
                .map(|cookie| Ok((cookie.name, cookie)))
                .collect()
        }
    }
}

fn get_cookie_map_for_req<'a>(
    req: &'a hyper::Request<hyper::Body>,
) -> Result<CookieMap<'a>, crate::Error> {
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

fn with_auth(
    mut new_req: hyper::Request<hyper::Body>,
    cookies: &CookieMap<'_>,
) -> Result<hyper::Request<hyper::Body>, hyper::header::InvalidHeaderValue> {
    let token = cookies.get("hitideToken").map(|c| c.value);
    if let Some(token) = token {
        new_req.headers_mut().insert(
            hyper::header::AUTHORIZATION,
            hyper::header::HeaderValue::from_str(&format!("Bearer {}", token))?,
        );
    }

    Ok(new_req)
}

async fn fetch_base_data(
    backend_host: &str,
    http_client: &crate::HttpClient,
    cookies: &CookieMap<'_>,
) -> Result<PageBaseData, crate::Error> {
    let login = {
        let api_res = http_client
            .request(with_auth(
                hyper::Request::get(format!("{}/api/unstable/logins/~current", backend_host))
                    .body(Default::default())?,
                &cookies,
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

async fn page_about(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"About lotide"}>
            <h2>{"What is lotide?"}</h2>
            <p>
                {"lotide is an attempt to build a federated forum. "}
                {"Users can create communities to share links and text posts and discuss them with other users, including those registered on other servers through "}
                <a href={"https://activitypub.rocks"}>{"ActivityPub"}</a>{"."}
            </p>
            <p>
                {"For more information or to view the source code, check out the "}
                <a href={"https://sr.ht/~vpzom/lotide/"}>{"SourceHut page"}</a>{"."}
            </p>
        </HTPage>
    }))
}

async fn page_comment(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_comment_inner(comment_id, &cookies, ctx, None, None).await
}

async fn page_comment_inner(
    comment_id: i64,
    cookies: &CookieMap<'_>,
    ctx: Arc<crate::RouteContext>,
    display_error: Option<String>,
    prev_values: Option<&serde_json::Value>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/comments/{}{}",
                    ctx.backend_host,
                    comment_id,
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
    let comment: RespPostCommentInfo<'_> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Comment"}>
            <p>
                <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
                <Content src={&comment} />
            </p>
            <div class={"actionList"}>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <>
                                {
                                    if comment.your_vote.is_some() {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/unlike", comment.id)}>
                                                <button type={"submit"}>{"Unlike"}</button>
                                            </form>
                                        }
                                    } else {
                                        render::rsx! {
                                            <form method={"POST"} action={format!("/comments/{}/like", comment.id)}>
                                                <button type={"submit"}>{"Like"}</button>
                                            </form>
                                        }
                                    }
                                }
                            </>
                        })
                    } else {
                        None
                    }
                }
                {
                    if author_is_me(&comment.author, &base_data.login) {
                        Some(render::rsx! {
                            <a href={format!("/comments/{}/delete", comment.id)}>{"delete"}</a>
                        })
                    } else {
                        None
                    }
                }
            </div>
            {
                display_error.map(|msg| {
                    render::rsx! {
                        <div class={"errorBox"}>{msg}</div>
                    }
                })
            }
            {
                if base_data.login.is_some() {
                    Some(render::rsx! {
                        <form method={"POST"} action={format!("/comments/{}/submit_reply", comment.id)}>
                            <div>
                                <MaybeFillTextArea values={&prev_values} name={"content_markdown"} default_value={None} />
                            </div>
                            <button r#type={"submit"}>{"Reply"}</button>
                        </form>
                    })
                } else {
                    None
                }
            }
            <ul>
                {
                    comment.replies.as_ref().unwrap().iter().map(|reply| {
                        render::rsx! {
                            <Comment comment={reply} base_data={&base_data} />
                        }
                    }).collect::<Vec<_>>()
                }
            </ul>
        </HTPage>
    }))
}

async fn page_comment_delete(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    page_comment_delete_inner(comment_id, ctx, &req.headers(), &cookies, None).await
}

async fn page_comment_delete_inner(
    comment_id: i64,
    ctx: Arc<crate::RouteContext>,
    headers: &hyper::header::HeaderMap,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let referer = headers
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/comments/{}",
                    ctx.backend_host, comment_id
                ))
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let comment: RespPostCommentInfo<'_> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Delete Comment"}>
            <p>
                <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
                <br />
                <Content src={&comment} />
            </p>
            <div id={"delete"}>
                <h2>{"Delete this comment?"}</h2>
                {
                    display_error.map(|msg| {
                        render::rsx! {
                            <div class={"errorBox"}>{msg}</div>
                        }
                    })
                }
                <form method={"POST"} action={format!("/comments/{}/delete/confirm", comment.id)}>
                    {
                        if let Some(referer) = referer {
                            Some(render::rsx! {
                                <input type={"hidden"} name={"return_to"} value={referer} />
                            })
                        } else {
                            None
                        }
                    }
                    <a href={format!("/comments/{}/", comment.id)}>{"No, cancel"}</a>
                    {" "}
                    <button r#type={"submit"}>{"Yes, delete"}</button>
                </form>
            </div>
        </HTPage>
    }))
}

async fn handler_comment_delete_confirm(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: ReturnToParams = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::delete(format!(
                    "{}/api/unstable/comments/{}",
                    ctx.backend_host, comment_id,
                ))
                .body("".into())?,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(
                hyper::header::LOCATION,
                if let Some(return_to) = &body.return_to {
                    &return_to
                } else {
                    "/"
                },
            )
            .body("Successfully deleted.".into())?),
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_comment_delete_inner(comment_id, ctx, &req_parts.headers, &cookies, Some(message))
                .await
        }
        Err(other) => Err(other),
    }
}

async fn handler_comment_like(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let referer = req
        .headers()
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/comments/{}/like",
                    ctx.backend_host, comment_id
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
            (if let Some(referer) = referer {
                Cow::Borrowed(referer)
            } else {
                format!("/comments/{}", comment_id).into()
            })
            .as_ref(),
        )
        .body("Successfully liked.".into())?)
}

async fn handler_comment_unlike(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let referer = req
        .headers()
        .get(hyper::header::REFERER)
        .and_then(|x| x.to_str().ok());

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/comments/{}/unlike",
                    ctx.backend_host, comment_id
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
            (if let Some(referer) = referer {
                Cow::Borrowed(referer)
            } else {
                format!("/comments/{}", comment_id).into()
            })
            .as_ref(),
        )
        .body("Successfully unliked.".into())?)
}

async fn handler_comment_submit_reply(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let (req_parts, body) = req.into_parts();

    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let body = hyper::body::to_bytes(body).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/comments/{}/replies",
                    ctx.backend_host, comment_id
                ))
                .body(serde_json::to_vec(&body)?.into())?,
                &cookies,
            )?)
            .await?,
    )
    .await;

    match api_res {
        Ok(_) => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::SEE_OTHER)
            .header(hyper::header::LOCATION, format!("/comments/{}", comment_id))
            .body("Successfully posted.".into())?),
        Err(crate::Error::RemoteError((status, message))) if status.is_client_error() => {
            page_comment_inner(comment_id, &cookies, ctx, Some(message), Some(&body)).await
        }
        Err(other) => Err(other),
    }
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
    let cookies = get_cookie_map_for_headers(&req_parts.headers)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Login"}>
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
                        <td><label for={"input_username"}>{"Username:"}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_password"}>{"Password:"}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                        </td>
                    </tr>
                </table>
                <button r#type={"submit"}>{"Login"}</button>
            </form>
            <p>
                {"Or "}<a href={"/signup"}>{"create a new account"}</a>
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

async fn page_lookup(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    #[derive(Deserialize)]
    struct LookupQuery<'a> {
        query: Option<Cow<'a, str>>,
    }

    let query: LookupQuery<'_> = serde_urlencoded::from_str(req.uri().query().unwrap_or(""))?;
    let query = query.query;

    #[derive(Deserialize)]
    struct LookupResult {
        id: i64,
    }

    let api_res: Option<Result<Vec<LookupResult>, String>> = if let Some(query) = &query {
        let api_res = res_to_error(
            ctx.http_client
                .request(
                    hyper::Request::get(format!(
                        "{}/api/unstable/actors:lookup/{}",
                        ctx.backend_host,
                        urlencoding::encode(&query)
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
        Some(Ok(items)) if !items.is_empty() => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::FOUND)
            .header(
                hyper::header::LOCATION,
                format!("/communities/{}", items[0].id),
            )
            .body("Redirecting…".into())?),
        api_res => {
            Ok(html_response(render::html! {
                <HTPage base_data={&base_data} title={"Lookup"}>
                    <h1>{"Lookup"}</h1>
                    <form method={"GET"} action={"/lookup"}>
                        <input r#type={"text"} name={"query"} value={query.as_deref().unwrap_or("")} />
                    </form>
                    {
                        match api_res {
                            None => None,
                            Some(Ok(_)) => {
                                // non-empty case is handled above
                                Some(render::rsx! { <p>{Cow::Borrowed("Nothing found.")}</p> })
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

    page_new_community_inner(ctx, &cookies, None, None).await
}

async fn page_new_community_inner(
    ctx: Arc<crate::RouteContext>,
    cookies: &CookieMap<'_>,
    display_error: Option<String>,
    prev_values: Option<&serde_json::Value>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"New Community"}>
            <h1>{"New Community"}</h1>
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
                        {"Name: "}<MaybeFillInput values={&prev_values} r#type={"text"} name={"name"} required={true} id={"input_name"} />
                    </label>
                </div>
                <div>
                    <button r#type={"submit"}>{"Create"}</button>
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
            .request(with_auth(
                hyper::Request::post(format!("{}/api/unstable/communities", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
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
            page_new_community_inner(ctx, &cookies, Some(message), Some(&body)).await
        }
        Err(other) => Err(other),
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
    prev_values: Option<&serde_json::Value>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_headers(&headers)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"Register"}>
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
                        <td><label for={"input_username"}>{"Username:"}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"text"} name={"username"} required={true} id={"input_username"} />
                        </td>
                    </tr>
                    <tr>
                        <td><label for={"input_password"}>{"Password:"}</label></td>
                        <td>
                            <MaybeFillInput values={&prev_values} r#type={"password"} name={"password"} required={true} id={"input_password"} />
                        </td>
                    </tr>
                </table>
                <button r#type={"submit"}>{"Register"}</button>
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
    let mut body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    body["login"] = true.into();

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

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

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
    let things: Vec<RespThingInfo> = serde_json::from_slice(&things)?;

    let title = user.as_ref().username.as_ref();

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title>
            <h1>{title}</h1>
            <div><em>{format!("@{}@{}", user.as_ref().username, user.as_ref().host)}</em></div>
            {
                if user.as_ref().local {
                    None
                } else if let Some(remote_url) = &user.as_ref().remote_url {
                    Some(render::rsx! {
                        <div class={"infoBox"}>
                            {"This is a remote user, information on this page may be incomplete. "}
                            <a href={remote_url.as_ref()}>{"View at Source ↗"}</a>
                        </div>
                    })
                } else {
                    None // shouldn't ever happen
                }
            }
            {
                if let Some(login) = &base_data.login {
                    if login.user.id == user_id {
                        Some(render::rsx! { <a href={format!("/users/{}/edit", user_id)}>{"Edit"}</a> })
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            <p>{user.description.as_ref()}</p>
            {
                if things.is_empty() {
                    Some(render::rsx! { <p>{"Looks like there's nothing here."}</p> })
                } else {
                    None
                }
            }
            <ul>
                {
                    things.iter().map(|thing| {
                        ThingItem { thing }
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

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let is_me = match &base_data.login {
        None => false,
        Some(login) => login.user.id == user_id,
    };

    if !is_me {
        let mut res = html_response(render::html! {
            <HTPage base_data={&base_data} title={"Edit Profile"}>
                <h1>{"Edit Profile"}</h1>
                <div class={"errorBox"}>{"You can only edit your own profile."}</div>
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
        <HTPage base_data={&base_data} title={"Edit Profile"}>
            <h1>{"Edit Profile"}</h1>
            <form method={"POST"} action={format!("/users/{}/edit/submit", user_id)}>
                <div>
                    <label>
                        {"Profile Description:"}<br />
                        <textarea name={"description"}>{user.description.as_ref()}</textarea>
                    </label>
                </div>
                <button type={"submit"}>{"Save"}</button>
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
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::patch(format!("{}/api/unstable/users/me", ctx.backend_host))
                    .body(serde_json::to_vec(&body)?.into())?,
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
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    if base_data.login.is_none() {
        return page_all_inner(&cookies, &base_data, ctx).await;
    }

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/users/me/following:posts",
                    ctx.backend_host
                ))
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"lotide"}>
            {
                if api_res.is_empty() {
                    Some(render::rsx! {
                        <p>
                            {"Looks like there's nothing here. Why not "}
                            <a href={"/communities"}>{"follow some communities"}</a>
                            {"?"}
                        </p>
                    })
                } else {
                    None
                }
            }
            <ul>
                {api_res.iter().map(|post| {
                    PostItem { post, in_community: false, no_user: false }
                }).collect::<Vec<_>>()}
            </ul>
        </HTPage>
    }))
}

async fn page_all(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    page_all_inner(&cookies, &base_data, ctx).await
}

async fn page_all_inner(
    cookies: &CookieMap<'_>,
    base_data: &crate::PageBaseData,
    ctx: Arc<crate::RouteContext>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!("{}/api/unstable/posts", ctx.backend_host))
                    .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data} title={"lotide"}>
            <h1>{"The Whole Known Network"}</h1>
            {
                if api_res.is_empty() {
                    Some(render::rsx! {
                        <p>
                        {"Looks like there's nothing here (yet!)."}
                        </p>
                    })
                } else {
                    None
                }
            }
            <ul>
                {api_res.iter().map(|post| {
                    PostItem { post, in_community: false, no_user: false }
                }).collect::<Vec<_>>()}
            </ul>
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
        .with_child(
            "comments",
            crate::RouteNode::new().with_child_parse::<i64, _>(
                crate::RouteNode::new()
                    .with_handler_async("GET", page_comment)
                    .with_child(
                        "delete",
                        crate::RouteNode::new()
                            .with_handler_async("GET", page_comment_delete)
                            .with_child(
                                "confirm",
                                crate::RouteNode::new()
                                    .with_handler_async("POST", handler_comment_delete_confirm),
                            ),
                    )
                    .with_child(
                        "like",
                        crate::RouteNode::new().with_handler_async("POST", handler_comment_like),
                    )
                    .with_child(
                        "unlike",
                        crate::RouteNode::new().with_handler_async("POST", handler_comment_unlike),
                    )
                    .with_child(
                        "submit_reply",
                        crate::RouteNode::new()
                            .with_handler_async("POST", handler_comment_submit_reply),
                    ),
            ),
        )
        .with_child("communities", communities::route_communities())
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
                    ),
            ),
        )
}
