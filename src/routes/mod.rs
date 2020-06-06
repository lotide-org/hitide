use serde_derive::Deserialize;
use std::borrow::Cow;
use std::sync::Arc;

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

fn with_auth(mut new_req: hyper::Request<hyper::Body>, cookies: &CookieMap<'_>) -> Result<hyper::Request<hyper::Body>, hyper::header::InvalidHeaderValue> {
    let token = cookies.get("hitideToken").map(|c| c.value);
    if let Some(token) = token {
        new_req.headers_mut()
            .insert(hyper::header::AUTHORIZATION, hyper::header::HeaderValue::from_str(&format!("Bearer {}", token))?);
    }

    Ok(new_req)
}

#[derive(Deserialize, Debug)]
struct RespMinimalAuthorInfo<'a> {
    id: i64,
    username: &'a str,
    local: bool,
    host: &'a str,
}

pub fn route_root() -> crate::RouteNode<()> {
    crate::RouteNode::new()
        .with_handler_async("GET", page_home)
        .with_child(
            "login",
            crate::RouteNode::new()
            .with_handler_async("GET", page_login)
            .with_child(
                "submit",
                crate::RouteNode::new()
                .with_handler_async("POST", handler_login_submit)
            )
        )
}

#[render::component]
fn HTPage<Children: render::Render>(children: Children) {
    render::rsx! {
        <>
            <render::html::HTML5Doctype />
            <html>
                <body>
                    <header class={"mainHeader"}>
                        <div>{"lotide"}</div>
                        <div>
                            <a href={"/login"}>{"Login"}</a>
                        </div>
                    </header>
                    {children}
                </body>
            </html>
        </>
    }
}

struct UserLink<'user> {
    user: Option<RespMinimalAuthorInfo<'user>>,
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
                }).render_into(writer)
            }
        }
    }
}

fn html_response(html: String) -> hyper::Response<hyper::Body> {
    let mut res = hyper::Response::new(html.into());
    res.headers_mut().insert(hyper::header::CONTENT_TYPE, hyper::header::HeaderValue::from_static("text/html"));
    res
}

async fn page_login(_: (), _ctx: Arc<crate::RouteContext>, _req: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    Ok(html_response(render::html! {
        <HTPage>
            <form method={"POST"} action={"/login/submit"}>
                <p>
                    <input r#type={"text"} name={"username"} />
                </p>
                <p>
                    <input r#type={"password"} name={"password"} />
                </p>
                <button r#type={"submit"}>{"Login"}</button>
            </form>
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

async fn handler_login_submit(_: (), ctx: Arc<crate::RouteContext>, req: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize)]
    struct LoginsCreateResponse<'a> {
        token: &'a str,
    }

    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body: serde_json::Value = serde_urlencoded::from_bytes(&body)?;
    let body = serde_json::to_vec(&body)?;

    let api_res = res_to_error(ctx.http_client.request(
        hyper::Request::post(format!("{}/api/unstable/logins", ctx.backend_host))
            .body(body.into())?
    ).await?).await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: LoginsCreateResponse = serde_json::from_slice(&api_res)?;

    let token = api_res.token;

    Ok(hyper::Response::builder()
        .status(hyper::StatusCode::SEE_OTHER)
        .header(hyper::header::SET_COOKIE, format!("hitideToken={}; Path=/; Max-Age={}", token, COOKIE_AGE))
        .header(hyper::header::LOCATION, "/")
        .body("Successfully logged in.".into())?)
}

async fn page_home(_: (), ctx: Arc<crate::RouteContext>, req: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    #[derive(Deserialize, Debug)]
    struct RespMinimalCommunityInfo<'a> {
        id: i64,
        name: &'a str,
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

    let cookies = get_cookie_map(req.headers().get(hyper::header::COOKIE).map(|x| x.to_str()).transpose()?)?;

    println!("{:?}", cookies);

    let api_res = res_to_error(ctx.http_client.request(
            with_auth(
                hyper::Request::get(format!("{}/api/unstable/users/me/following:posts", ctx.backend_host))
                    .body(Default::default())?,
                &cookies,
            )?
    ).await?).await?;

    let api_res = hyper::body::to_bytes(api_res.into_body()).await?;
    let api_res: Vec<RespPostListPost<'_>> = serde_json::from_slice(&api_res)?;

    println!("{:?}", api_res);

    Ok(html_response(render::html! {
        <HTPage>
            <ul>
                {api_res.into_iter().map(|post| {
                    render::rsx! {
                        <li>
                            <a href={post.href}>
                                {post.title}
                            </a>
                            <br />
                            {"Submitted by "}<UserLink user={post.author} />
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
        </HTPage>
    }))
}
