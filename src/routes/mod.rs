use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

use crate::components::{Content, HTPage, PostItem, UserLink};
use crate::resp_types::{RespPostCommentInfo, RespPostListPost};
use crate::PageBaseData;

mod communities;
mod posts;
mod r#static;

const COOKIE_AGE: u32 = 60 * 60 * 24 * 365;

type CookieMap<'a> = std::collections::HashMap<&'a str, ginger::Cookie<'a>>;

fn get_cookie_map<'a>(src: Option<&'a str>) -> Result<CookieMap<'a>, ginger::ParseError> {
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
    get_cookie_map(get_cookies_string(req)?).map_err(Into::into)
}

fn get_cookies_string<'a>(
    req: &'a hyper::Request<hyper::Body>,
) -> Result<Option<&'a str>, crate::Error> {
    Ok(req
        .headers()
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

async fn page_comment(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

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
        <HTPage base_data={&base_data}>
            <p>
                <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
                <Content src={&comment} />
            </p>
            <form method={"POST"} action={format!("/comments/{}/submit_reply", comment.id)}>
                <div>
                    <textarea name={"content_text"}>{()}</textarea>
                </div>
                <button r#type={"submit"}>{"Reply"}</button>
            </form>
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

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

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
        <HTPage base_data={&base_data}>
            <p>
                <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
                <br />
                <Content src={&comment} />
            </p>
            <div id={"delete"}>
                <h2>{"Delete this comment?"}</h2>
                <form method={"POST"} action={format!("/comments/{}/delete/confirm", comment.id)}>
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

    let cookies = get_cookie_map_for_req(&req)?;

    res_to_error(
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
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, "/")
        .body("Successfully deleted.".into())?)
}

async fn handler_comment_like(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (comment_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

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
        .header(hyper::header::LOCATION, format!("/comments/{}", comment_id))
        .body("Successfully liked.".into())?)
}

async fn handler_comment_submit_reply(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct CommentsRepliesCreateResponsePost {
        id: i64,
    }
    #[derive(Deserialize)]
    struct CommentsRepliesCreateResponse {
        post: CommentsRepliesCreateResponsePost,
    }

    let (comment_id,) = params;

    let cookies_string = get_cookies_string(&req)?.map(ToOwned::to_owned);
    let cookies_string = cookies_string.as_deref();
    let cookies = get_cookie_map(cookies_string)?;

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/comments/{}/replies",
                    ctx.backend_host, comment_id
                ))
                .body(body.into())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: CommentsRepliesCreateResponse = serde_json::from_slice(&api_res)?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(
            hyper::header::LOCATION,
            format!("/posts/{}", api_res.post.id),
        )
        .body("Successfully posted.".into())?)
}

async fn page_login(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <form method={"POST"} action={"/login/submit"}>
                <p>
                    <input r#type={"text"} name={"username"} />
                </p>
                <p>
                    <input r#type={"password"} name={"password"} />
                </p>
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
    if res.status().is_success() {
        Ok(res)
    } else {
        let bytes = hyper::body::to_bytes(res.into_body()).await?;
        Err(crate::Error::InternalStr(format!(
            "Error in remote response: {}",
            String::from_utf8_lossy(&bytes)
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

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::post(format!("{}/api/unstable/logins", ctx.backend_host))
                    .body(body.into())?,
            )
            .await?,
    )
    .await?;

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

    let api_res: Option<Vec<LookupResult>> = if let Some(query) = &query {
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
        .await?;

        let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
        Some(serde_json::from_slice(&api_res)?)
    } else {
        None
    };

    match api_res {
        Some(items) if !items.is_empty() => Ok(hyper::Response::builder()
            .status(hyper::StatusCode::FOUND)
            .header(
                hyper::header::LOCATION,
                format!("/communities/{}", items[0].id),
            )
            .body("Redirecting…".into())?),
        _ => {
            Ok(html_response(render::html! {
                <HTPage base_data={&base_data}>
                    <h1>{"Lookup"}</h1>
                    <form method={"GET"} action={"/lookup"}>
                        <input r#type={"text"} name={"query"} value={query.as_deref().unwrap_or("")} />
                    </form>
                    {
                        match api_res {
                            None => None,
                            Some(_) => {
                                // non-empty case is handled above
                                Some(render::rsx! { <p>{"Nothing found."}</p> })
                            },
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

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <h1>{"New Community"}</h1>
            <form method={"POST"} action={"/new_community/submit"}>
                <div>
                    <label>
                        {"Name: "}<input r#type={"text"} name={"name"} required={"true"} />
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
    let cookies_string = get_cookies_string(&req)?.map(ToOwned::to_owned);
    let cookies_string = cookies_string.as_deref();
    let cookies = get_cookie_map(cookies_string)?;

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

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
                    .body(body.into())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;
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

async fn page_signup(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <form method={"POST"} action={"/signup/submit"}>
                <p>
                    <input r#type={"text"} name={"username"} />
                </p>
                <p>
                    <input r#type={"password"} name={"password"} />
                </p>
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

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let mut body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    body["login"] = true.into();
    let body = serde_json::to_vec(&body)?;

    let api_res = res_to_error(
        ctx.http_client
            .request(
                hyper::Request::post(format!("{}/api/unstable/users", ctx.backend_host))
                    .body(body.into())?,
            )
            .await?,
    )
    .await?;

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

async fn page_home(
    _: (),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let for_user = base_data.login.is_some();

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(if for_user {
                    format!("{}/api/unstable/users/me/following:posts", ctx.backend_host,)
                } else {
                    format!("{}/api/unstable/posts", ctx.backend_host,)
                })
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <ul>
                {api_res.iter().map(|post| {
                    PostItem { post, in_community: false }
                }).collect::<Vec<_>>()}
            </ul>
        </HTPage>
    }))
}

pub fn route_root() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_home)
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
}
