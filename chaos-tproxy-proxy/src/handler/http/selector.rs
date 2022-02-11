use http::header::HeaderMap;
use http::{Method, Request, Response, StatusCode, Uri};
use hyper::Body;
use wildmatch::WildMatch;

#[derive(Debug, Clone)]
pub struct Selector {
    pub port: Option<u16>,
    pub path: Option<WildMatch>,
    pub method: Option<Method>,
    pub code: Option<StatusCode>,
    pub request_headers: Option<HeaderMap>,
    pub response_headers: Option<HeaderMap>,
}

pub fn select_request(port: u16, request: &Request<Body>, selector: &Selector) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector
            .path
            .iter()
            .all(|p| p.matches(request.uri().path()))
        && selector.method.iter().all(|m| request.method() == m)
        && selector.request_headers.iter().all(|fields| {
            fields
                .iter()
                .all(|(header, value)| request.headers().get_all(header).iter().any(|f| f == value))
        })
}

pub fn select_response(
    port: u16,
    uri: &Uri,
    method: &Method,
    request_headers: &HeaderMap,
    response: &Response<Body>,
    selector: &Selector,
) -> bool {
    selector.port.iter().all(|p| port == *p)
        && selector.path.iter().all(|p| p.matches(uri.path()))
        && selector.method.iter().all(|m| method == m)
        && selector.code.iter().all(|code| response.status() == *code)
        && selector.request_headers.iter().all(|fields| {
            fields
                .iter()
                .all(|(header, value)| request_headers.get_all(header).iter().any(|f| f == value))
        })
        && selector.response_headers.iter().all(|fields| {
            fields.iter().all(|(header, value)| {
                response
                    .headers()
                    .get_all(header)
                    .iter()
                    .any(|f| f == value)
            })
        })
}

#[cfg(test)]
mod tests {
    use http::Request;
    use hyper::Body;

    use crate::handler::http::selector::{select_request, Selector};

    #[test]
    fn test_select_request() {
        let port = 1025;
        let selector = Selector {
            port: Some(1025),
            path: None,
            method: None,
            code: None,
            request_headers: None,
            response_headers: None,
        };
        let req = Request::builder().body(Body::empty()).unwrap();
        assert_eq!(select_request(port, &req, &selector), true);

        let mut selector = Selector {
            port: None,
            path: Some(wildmatch::WildMatch::new("/src")),
            method: None,
            code: None,
            request_headers: None,
            response_headers: None,
        };
        let req = Request::builder()
            .uri("http://www.google.com/src/")
            .body(Body::empty())
            .unwrap();
        assert_eq!(select_request(0, &req, &selector), false);

        selector.path = Some(wildmatch::WildMatch::new("src"));
        assert_eq!(select_request(0, &req, &selector), false);

        selector.path = Some(wildmatch::WildMatch::new("/src/"));
        assert_eq!(select_request(0, &req, &selector), true);

        selector.path = Some(wildmatch::WildMatch::new("/src*"));
        assert_eq!(select_request(0, &req, &selector), true);

        selector.path = Some(wildmatch::WildMatch::new("/src?"));
        assert_eq!(select_request(0, &req, &selector), true);
    }
}
