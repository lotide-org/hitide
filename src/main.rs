#![feature(proc_macro_hygiene)]
#![allow(unused_braces)]

use crate::resp_types::RespLoginInfo;
use std::sync::Arc;
use trout::hyper::RoutingFailureExtHyper;

mod components;
mod resp_types;
mod routes;
mod util;

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

pub fn simple_response(
    code: hyper::StatusCode,
    text: impl Into<hyper::Body>,
) -> hyper::Response<hyper::Body> {
    let mut res = hyper::Response::new(text.into());
    *res.status_mut() = code;
    res
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend_host = std::env::var("BACKEND_HOST").expect("Missing BACKEND_HOST");

    let port = match std::env::var("PORT") {
        Ok(port_str) => port_str.parse().expect("Failed to parse port"),
        _ => 4333,
    };

    let routes = Arc::new(routes::route_root());
    let context = Arc::new(RouteContext {
        backend_host,
        http_client: hyper::Client::builder().build(hyper_tls::HttpsConnector::new()),
    });

    let server = hyper::Server::bind(&(std::net::Ipv6Addr::UNSPECIFIED, port).into()).serve(
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
                            Err(Error::Internal(err)) => {
                                eprintln!("Error: {:?}", err);

                                simple_response(
                                    hyper::StatusCode::INTERNAL_SERVER_ERROR,
                                    "Internal Server Error",
                                )
                            }
                            Err(Error::InternalStr(err)) => {
                                eprintln!("Error: {}", err);

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
