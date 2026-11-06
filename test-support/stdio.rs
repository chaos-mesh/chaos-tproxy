use std::io;
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::{ensure, Context as _};
use hyper::client::conn::Builder;
use hyper::{Body, Request, StatusCode, Version};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

use super::BINARY;

const TIMEOUT: Duration = Duration::from_secs(5);

struct ChildIo {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl AsyncRead for ChildIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdout).poll_read(cx, buf)
    }
}

impl AsyncWrite for ChildIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stdin).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stdin).poll_shutdown(cx)
    }
}

#[tokio::test]
async fn http2_configuration_errors_preserve_connection_and_stdin_eof_exits() {
    let mut child = Command::new(BINARY)
        .arg("-i")
        .env("RUST_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let io = ChildIo {
        stdin: child.stdin.take().unwrap(),
        stdout: child.stdout.take().unwrap(),
    };
    let mut stderr = child.stderr.take().unwrap();
    let stderr_reader = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await.map(|_| bytes)
    });
    let mut driver = None;

    let requests: anyhow::Result<()> = async {
        let (mut sender, connection) =
            timeout(TIMEOUT, Builder::new().http2_only(true).handshake(io))
                .await
                .context("HTTP/2 handshake timed out")??;
        driver = Some(tokio::spawn(connection));

        for (json, expected_status) in [
            ("{}", StatusCode::OK),
            (r#""notconfig""#, StatusCode::BAD_REQUEST),
            ("{}", StatusCode::OK),
        ] {
            timeout(TIMEOUT, async {
                let request = Request::put("http://localhost/")
                    .header("content-type", "application/json")
                    .body(Body::from(json))?;
                let response = sender.send_request(request).await?;
                ensure!(response.version() == Version::HTTP_2, "expected HTTP/2");
                ensure!(
                    response.status() == expected_status,
                    "PUT {json} returned {}, expected {expected_status}",
                    response.status()
                );
                let body = hyper::body::to_bytes(response.into_body()).await?;
                if expected_status == StatusCode::BAD_REQUEST {
                    ensure!(
                        String::from_utf8_lossy(&body).contains("invalid type"),
                        "missing configuration error in {body:?}"
                    );
                } else {
                    ensure!(
                        body.is_empty(),
                        "unexpected successful response body {body:?}"
                    );
                }
                Ok::<_, anyhow::Error>(())
            })
            .await
            .with_context(|| format!("PUT {json} timed out"))??;
        }
        Ok(())
    }
    .await;

    if let Some(driver) = driver {
        driver.abort();
        let _ = driver.await;
    }
    if requests.is_err() {
        child.kill().await.unwrap();
    }
    let exited = timeout(TIMEOUT, child.wait()).await;
    if exited.is_err() {
        child.kill().await.unwrap();
    }
    let stderr = stderr_reader.await.unwrap().unwrap();
    let stderr = String::from_utf8_lossy(&stderr);
    requests.unwrap_or_else(|error| panic!("{:#}\nstderr:\n{}", error, stderr));
    let status = exited
        .unwrap_or_else(|_| panic!("stdin EOF did not stop the process\nstderr:\n{}", stderr))
        .unwrap();
    assert_eq!(status.code(), Some(0), "stderr:\n{stderr}");
}
