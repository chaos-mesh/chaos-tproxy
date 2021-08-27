use http::header::CONTENT_LENGTH;
use http::HeaderMap;
use hyper::{Body, Client, Method, Request};
use rs_tproxy_proxy::handler::http::action::{apply_request_action, Actions, ReplaceAction};

#[tokio::test]
#[ignore]
async fn test_http_content_length_replace() {
    let client = Client::new();
    let data = serde_json::to_string("Hallo").unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("http://127.0.0.1:8080/set-body")
        .body(Body::from(data.clone()))
        .unwrap();

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_LENGTH,
        (data.len() - 2).to_string().parse().unwrap(),
    );
    let actions = Actions {
        abort: false,
        delay: None,
        replace: Some(ReplaceAction {
            path: None,
            method: None,
            body: None,
            code: None,
            queries: None,
            headers: Some(headers),
        }),
        patch: None,
    };

    let req = apply_request_action(req, &actions).await.unwrap();
    let err = client.request(req).await.err().unwrap();
    assert!(err.is_incomplete_message());
}
