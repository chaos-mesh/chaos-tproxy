use anyhow::{anyhow, Result};
use futures_util::future::join_all;
use iproute2_rs::ip::iproute::{del_routes, get_routes, Action, IPRoute};
use rtnetlink::packet::RouteMessage;
use rtnetlink::{IpVersion, Handle};


pub async fn get_routes_noblock(handle: &Handle) -> Result<Vec<RouteMessage>> {
    let routes = get_routes(handle, IpVersion::V4).await?;
    Ok(routes
        .into_iter()
        .filter(|route| route.header.table != 255)
        .collect())
}

pub async fn del_routes_noblock(handle: &Handle, msgs: Vec<RouteMessage>) -> Result<()> {
    let results = join_all(msgs.into_iter().map(|msg| del_routes(handle, msg))).await;
    match results
        .into_iter()
        .filter(|result| result.is_err())
        .map(|r| r.unwrap_err())
        .reduce(|accum, item| anyhow!("{} \n {}", accum, item))
    {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

pub async fn load_routes(handle: &mut Handle, msgs: Vec<RouteMessage>) -> Result<()> {
    for route in msgs {
        IPRoute {
            action: Action::Add,
            msg: route.clone(),
        }
        .execute(handle)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("can not recover ROUTE MSG: {:?}, error: {}", route, e)
        });
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use rtnetlink::new_connection;
    use tokio::spawn;
    use crate::proxy::net::routes::{del_routes_noblock, get_routes_noblock, load_routes};

    #[ignore]
    #[tokio::test]
    async fn test_get_del_routes() {
        let (conn, mut handle, _) = new_connection().unwrap();

        spawn(conn);

        let mut routes = get_routes_noblock(&handle).await.unwrap();
        del_routes_noblock(&handle, routes.clone()).await.unwrap();

        routes.reverse();

        load_routes(&mut handle, routes).await.unwrap();
    }
}
