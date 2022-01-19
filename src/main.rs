#![allow(unused_braces)]

use crate::resp_types::RespLoginInfo;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use trout::http02::RoutingFailureExtHttp;

mod components;
mod config;
mod lang;
mod query_types;
mod resp_types;
mod routes;
mod util;

pub use lang::Translator;

use self::config::Config;

#[derive(Deserialize, Serialize, PartialEq, Clone, Copy, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SortType {
    Hot,
    New,
    Top,
}

impl SortType {
    pub const VALUES: &'static [SortType] = &[SortType::Hot, SortType::New, SortType::Top];

    pub fn as_str(&self) -> &'static str {
        match self {
            SortType::Hot => "hot",
            SortType::New => "new",
            SortType::Top => "top",
        }
    }

    pub fn lang_key(&self) -> lang::LangKey<'static> {
        match self {
            SortType::Hot => lang::sort_hot(),
            SortType::New => lang::sort_new(),
            SortType::Top => lang::sort_top(),
        }
    }
}

pub type HttpClient = hyper::Client<hyper_tls::HttpsConnector<hyper::client::HttpConnector>>;

pub struct RouteContext {
    backend_host: String,
    http_client: HttpClient,
}

pub type RouteNode<P> = trout::Node<
    P,
    hyper::Request<hyper::Body>,
    std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<hyper::Response<hyper::Body>, Error>> + Send>,
    >,
    Arc<RouteContext>,
>;

#[derive(Debug)]
pub enum Error {
    Internal(Box<dyn std::error::Error + Send>),
    InternalStr(String),
    UserError(hyper::Response<hyper::Body>),
    RoutingError(trout::RoutingFailure),
    RemoteError((hyper::StatusCode, String)),
    InternalUserError(String),
}

impl<T: 'static + std::error::Error + Send> From<T> for Error {
    fn from(err: T) -> Error {
        Error::Internal(Box::new(err))
    }
}

#[derive(Debug)]
pub struct PageBaseData {
    pub login: Option<RespLoginInfo>,
}

impl PageBaseData {
    pub fn is_site_admin(&self) -> bool {
        match &self.login {
            None => false,
            Some(login) => login.user.is_site_admin,
        }
    }
}

pub fn simple_response(
    code: hyper::StatusCode,
    text: impl Into<hyper::Body>,
) -> hyper::Response<hyper::Body> {
    let mut res = hyper::Response::new(text.into());
    *res.status_mut() = code;
    res
}

lazy_static::lazy_static! {
    static ref LANG_MAP: HashMap<unic_langid::LanguageIdentifier, fluent::FluentResource> = {
        let mut result = HashMap::new();

        result.insert(unic_langid::langid!("de"), fluent::FluentResource::try_new(include_str!("../res/lang/de.ftl").to_owned()).expect("Failed to parse translation"));
        result.insert(unic_langid::langid!("en"), fluent::FluentResource::try_new(include_str!("../res/lang/en.ftl").to_owned()).expect("Failed to parse translation"));
        result.insert(unic_langid::langid!("eo"), fluent::FluentResource::try_new(include_str!("../res/lang/eo.ftl").to_owned()).expect("Failed to parse translation"));
        result.insert(unic_langid::langid!("fr"), fluent::FluentResource::try_new(include_str!("../res/lang/fr.ftl").to_owned()).expect("Failed to parse translation"));

        result
    };

    static ref LANGS: Vec<unic_langid::LanguageIdentifier> = {
        LANG_MAP.keys().cloned().collect()
    };
}

pub fn get_lang_for_headers(headers: &hyper::header::HeaderMap) -> Translator {
    let default = unic_langid::langid!("en");
    let languages = match headers
        .get(hyper::header::ACCEPT_LANGUAGE)
        .and_then(|x| x.to_str().ok())
    {
        Some(accept_language) => {
            let requested = fluent_langneg::accepted_languages::parse(accept_language);
            fluent_langneg::negotiate_languages(
                &requested,
                &LANGS,
                Some(&default),
                fluent_langneg::NegotiationStrategy::Filtering,
            )
        }
        None => vec![&default],
    };

    let mut bundle = fluent::concurrent::FluentBundle::new(languages.iter().copied());
    for lang in languages {
        if let Err(errors) = bundle.add_resource(&LANG_MAP[lang]) {
            for err in errors {
                match err {
                    fluent::FluentError::Overriding { .. } => {}
                    _ => {
                        log::error!("Failed to add language resource: {:?}", err);
                        break;
                    }
                }
            }
        }
    }

    Translator::new(bundle)
}

pub fn get_lang_for_req(req: &hyper::Request<hyper::Body>) -> Translator {
    get_lang_for_headers(req.headers())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let config = Config::load().expect("Failed to load config");

    let routes = Arc::new(routes::route_root());
    let context = Arc::new(RouteContext {
        backend_host: config.backend_host,
        http_client: hyper::Client::builder().build(hyper_tls::HttpsConnector::new()),
    });

    let server = hyper::Server::bind(&(std::net::Ipv6Addr::UNSPECIFIED, config.port).into()).serve(
        hyper::service::make_service_fn(|_| {
            let routes = routes.clone();
            let context = context.clone();
            async {
                Ok::<_, hyper::Error>(hyper::service::service_fn(move |req| {
                    let routes = routes.clone();
                    let context = context.clone();
                    async move {
                        let result = match routes.route(req, context) {
                            Ok(fut) => fut.await,
                            Err(err) => Err(Error::RoutingError(err)),
                        };
                        Ok::<_, hyper::Error>(match result {
                            Ok(val) => val,
                            Err(Error::UserError(res)) => res,
                            Err(Error::RoutingError(err)) => err.to_simple_response(),
                            Err(err) => {
                                log::error!("Error: {:?}", err);

                                simple_response(
                                    hyper::StatusCode::INTERNAL_SERVER_ERROR,
                                    "Internal Server Error",
                                )
                            }
                        })
                    }
                }))
            }
        }),
    );

    server.await?;

    Ok(())
}
