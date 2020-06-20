use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

mod communities;
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
    username: Cow<'a, str>,
    local: bool,
    host: Cow<'a, str>,
}

#[derive(Deserialize, Debug)]
struct RespPostListPost<'a> {
    id: i64,
    title: Cow<'a, str>,
    href: Option<Cow<'a, str>>,
    content_text: Option<Cow<'a, str>>,
    #[serde(borrow)]
    author: Option<RespMinimalAuthorInfo<'a>>,
    created: Cow<'a, str>,
    #[serde(borrow)]
    community: RespMinimalCommunityInfo<'a>,
}

#[derive(Deserialize, Debug)]
struct RespPostCommentInfo<'a> {
    id: i64,
    #[serde(borrow)]
    author: Option<RespMinimalAuthorInfo<'a>>,
    created: Cow<'a, str>,
    content_text: Cow<'a, str>,
    #[serde(borrow)]
    replies: Option<Vec<RespPostCommentInfo<'a>>>,
}

#[derive(Deserialize, Debug)]
struct RespPostInfo<'a> {
    #[serde(flatten, borrow)]
    pub base: RespPostListPost<'a>,
    #[serde(borrow)]
    pub comments: Vec<RespPostCommentInfo<'a>>,
}

impl<'a> AsRef<RespPostListPost<'a>> for RespPostInfo<'a> {
    fn as_ref(&self) -> &RespPostListPost<'a> {
        &self.base
    }
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
                <head>
                    <meta charset={"utf-8"} />
                    <link rel={"stylesheet"} href={"/static/main.css"} />
                </head>
                <body>
                    <header class={"mainHeader"}>
                        <div class={"left"}><a href={"/"}>{"lotide"}</a></div>
                        <div class={"right"}>
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

fn abbreviate_link(href: &str) -> &str {
    // Attempt to find the hostname from the URL
    match href.find("://") {
        Some(idx1) => match href[(idx1 + 3)..].find('/') {
            Some(idx2) => Some(&href[(idx1 + 3)..(idx1 + 3 + idx2)]),
            None => None,
        },
        None => None,
    }
    .unwrap_or(href)
}

#[render::component]
fn PostItem<'post>(post: &'post RespPostListPost<'post>, in_community: bool) {
    render::rsx! {
        <li>
            <a href={format!("/posts/{}", post.id)}>
                {post.title.as_ref()}
            </a>
            {
                if let Some(href) = &post.href {
                    Some(render::rsx! {
                        <>
                            {" "}
                            <em><a href={href.as_ref()}>{abbreviate_link(&href)}{" ↗"}</a></em>
                        </>
                    })
                } else {
                    None
                }
            }
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
    name: Cow<'a, str>,
    local: bool,
    host: Cow<'a, str>,
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
                                user.username.as_ref().into()
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
                    community.name.as_ref().into()
                } else {
                    Cow::Owned(format!("{}@{}", community.name, community.host))
                }).as_ref()
            }
            </a>
        })
        .render_into(writer)
    }
}

#[render::component]
fn Comment<'comment>(comment: &'comment RespPostCommentInfo<'comment>) {
    render::rsx! {
        <li>
            <small><cite><UserLink user={comment.author.as_ref()} /></cite>{":"}</small>
            <br />
            {comment.content_text.as_ref()}
            <br />
            <div>
                <a href={format!("/comments/{}", comment.id)}>{"reply"}</a>
            </div>

            {
                match &comment.replies {
                    Some(replies) => {
                        Some(render::rsx! {
                            <ul>
                                {
                                    replies.iter().map(|reply| {
                                        render::rsx! {
                                            <Comment comment={reply} />
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                }
                            </ul>
                        })
                    },
                    None => None,
                }
            }
        </li>
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
                <br />
                {comment.content_text.as_ref()}
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

async fn page_post(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies = get_cookie_map_for_req(&req)?;

    let base_data = fetch_base_data(&ctx.backend_host, &ctx.http_client, &cookies).await?;

    let api_res = res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::get(format!(
                    "{}/api/unstable/posts/{}",
                    ctx.backend_host, post_id
                ))
                .body(Default::default())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;
    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;

    let post: RespPostInfo = serde_json::from_slice(&api_res)?;

    Ok(html_response(render::html! {
        <HTPage base_data={&base_data}>
            <h1>{post.as_ref().title.as_ref()}</h1>
            <p>
                {"Submitted by "}<UserLink user={post.as_ref().author.as_ref()} />
                {" to "}<CommunityLink community={&post.as_ref().community} />
            </p>
            {
                match &post.as_ref().href {
                    None => None,
                    Some(href) => {
                        Some(render::rsx! {
                            <p><a href={href.as_ref()}>{href.as_ref()}</a></p>
                        })
                    }
                }
            }
            {
                match &post.as_ref().content_text {
                    None => None,
                    Some(content_text) => {
                        Some(render::rsx! {
                            <p>{content_text.as_ref()}</p>
                        })
                    }
                }
            }
            <div>
                <h2>{"Comments"}</h2>
                {
                    if base_data.login.is_some() {
                        Some(render::rsx! {
                            <form method={"POST"} action={format!("/posts/{}/submit_reply", post.as_ref().id)}>
                                <div>
                                    <textarea name={"content_text"}>{()}</textarea>
                                </div>
                                <button r#type={"submit"}>{"Post Comment"}</button>
                            </form>
                        })
                    } else {
                        None
                    }
                }
                <ul>
                    {
                        post.comments.iter().map(|comment| {
                            render::rsx! {
                                <Comment comment={comment} />
                            }
                        }).collect::<Vec<_>>()
                    }
                </ul>
            </div>
        </HTPage>
    }))
}

async fn handler_post_submit_reply(
    params: (i64,),
    ctx: Arc<crate::RouteContext>,
    req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    let (post_id,) = params;

    let cookies_string = get_cookies_string(&req)?.map(ToOwned::to_owned);
    let cookies_string = cookies_string.as_deref();
    let cookies = get_cookie_map(cookies_string)?;

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

    res_to_error(
        ctx.http_client
            .request(with_auth(
                hyper::Request::post(format!(
                    "{}/api/unstable/posts/{}/replies",
                    ctx.backend_host, post_id
                ))
                .body(body.into())?,
                &cookies,
            )?)
            .await?,
    )
    .await?;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::LOCATION, format!("/posts/{}", post_id))
        .body("Successfully posted.".into())?)
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
            "posts",
            crate::RouteNode::new().with_child_parse::<i64, _>(
                crate::RouteNode::new()
                    .with_handler_async("GET", page_post)
                    .with_child(
                        "submit_reply",
                        crate::RouteNode::new()
                            .with_handler_async("POST", handler_post_submit_reply),
                    ),
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
        .with_child("static", r#static::route_static())
}
