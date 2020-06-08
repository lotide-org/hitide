use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

mod communities;

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
    get_cookie_map(
        req.headers()
            .get(hyper::header::COOKIE)
            .map(|x| x.to_str())
            .transpose()?,
    )
    .map_err(Into::into)
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

#[derive(Deserialize, Debug)]
struct RespLoginInfoUser {
    id: i64,
}

#[derive(Deserialize, Debug)]
struct RespLoginInfo {
    user: RespLoginInfoUser,
}

#[derive(Debug)]
struct PageBaseData {
    login: Option<RespLoginInfo>,
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

#[derive(Deserialize, Debug)]
struct RespMinimalAuthorInfo<'a> {
    id: i64,
    username: &'a str,
    local: bool,
    host: &'a str,
}

#[derive(Deserialize, Debug)]
struct RespPostListPost<'a> {
    id: i64,
    title: &'a str,
    href: &'a str,
    author: Option<RespMinimalAuthorInfo<'a>>,
    created: &'a str,
    community: RespMinimalCommunityInfo<'a>,
}

#[render::component]
fn HTPage<'base_data, Children: render::Render>(
    base_data: &'base_data PageBaseData,
    children: Children,
) {
    render::rsx! {
        <>
            <render::html::HTML5Doctype />
            <html>
                <body>
                    <header class={"mainHeader"}>
                        <div><a href={"/"}>{"lotide"}</a></div>
                        <div>
                            {
                                match base_data.login {
                                    Some(_) => None,
                                    None => {
                                        Some(render::rsx! {
                                            <a href={"/login"}>{"Login"}</a>
                                        })
                                    }
                                }
                            }
                        </div>
                    </header>
                    {children}
                </body>
            </html>
        </>
    }
}

#[render::component]
fn PostItem<'post>(post: &'post RespPostListPost<'post>, in_community: bool) {
    render::rsx! {
        <li>
            <a href={post.href}>
                {post.title}
            </a>
            <br />
            {"Submitted by "}<UserLink user={post.author.as_ref()} />
            {
                if !in_community {
                    Some(render::rsx! {
                        <>{" to "}<CommunityLink community={&post.community} /></>
                    })
                } else {
                    None
                }
            }
        </li>
    }
}

#[derive(Deserialize, Debug)]
struct RespMinimalCommunityInfo<'a> {
    id: i64,
    name: &'a str,
    local: bool,
    host: &'a str,
}

struct UserLink<'user> {
    user: Option<&'user RespMinimalAuthorInfo<'user>>,
}

impl<'user> render::Render for UserLink<'user> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        match self.user {
            None => "[unknown]".render_into(writer),
            Some(user) => {
                let href = format!("/users/{}", user.id);
                (render::rsx! {
                    <a href={&href}>
                        {
                            (if user.local {
                                user.username.into()
                            } else {
                                Cow::Owned(format!("{}@{}", user.username, user.host))
                            }).as_ref()
                        }
                    </a>
                })
                .render_into(writer)
            }
        }
    }
}

struct CommunityLink<'community> {
    community: &'community RespMinimalCommunityInfo<'community>,
}
impl<'community> render::Render for CommunityLink<'community> {
    fn render_into<W: std::fmt::Write>(self, writer: &mut W) -> std::fmt::Result {
        let community = &self.community;

        let href = format!("/communities/{}", community.id);
        (render::rsx! {
            <a href={&href}>
            {
                (if community.local {
                    community.name.into()
                } else {
                    Cow::Owned(format!("{}@{}", community.name, community.host))
                }).as_ref()
            }
            </a>
        })
        .render_into(writer)
    }
}

fn html_response(html: String) -> hyper::Response<hyper::Body> {
    let mut res = hyper::Response::new(html.into());
    res.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html"),
    );
    res
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
    let cookies_string = req
        .headers()
        .get(hyper::header::COOKIE)
        .map(|x| x.to_str())
        .transpose()?
        .map(|x| x.to_owned());
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
            "signup",
            crate::RouteNode::new()
                .with_handler_async("GET", page_signup)
                .with_child(
                    "submit",
                    crate::RouteNode::new().with_handler_async("POST", handler_signup_submit),
                ),
        )
}
