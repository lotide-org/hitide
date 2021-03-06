use hitide_icons::ICONS_MAP;
use std::sync::Arc;

const FILE_MAIN_CSS: &[u8] = include_bytes!("../../res/main.css");

pub fn route_static() -> crate::RouteNode<()> {
    crate::RouteNode::new().with_child_str(
        crate::RouteNode::new().with_handler_async(hyper::Method::GET, handler_static_get),
    )
}

async fn handler_static_get(
    params: (String,),
    _ctx: Arc<crate::RouteContext>,
    _req: hyper::Request<hyper::Body>,
) -> Result<hyper::Response<hyper::Body>, crate::Error> {
    if params.0 == "main.css" {
        let mut resp = hyper::Response::new(FILE_MAIN_CSS.into());
        resp.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("text/css"),
        );

        Ok(resp)
    } else if let Some(icon) = ICONS_MAP.get(params.0.as_str()) {
        let mut resp = hyper::Response::new(icon.content.into());
        resp.headers_mut().insert(
            hyper::header::CONTENT_TYPE,
            hyper::header::HeaderValue::from_static("image/svg+xml"),
        );
        resp.headers_mut().insert(
            hyper::header::CACHE_CONTROL,
            hyper::header::HeaderValue::from_static("public, max-age=31536000, immutable"),
        );

        Ok(resp)
    } else {
        Err(crate::Error::RoutingError(trout::RoutingFailure::NotFound))
    }
}
