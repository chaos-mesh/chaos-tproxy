use anyhow::Result;
use iproute2_rs::ip::iproute::{del_routes, get_routes, Action, IPRoute};
use rtnetlink::packet::RouteMessage;
use rtnetlink::{new_connection, IpVersion};

pub async fn get_routes_noblock() -> Result<Vec<RouteMessage>> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    let routes = get_routes(&handle, IpVersion::V4).await?;

    Ok(routes
        .into_iter()
        .filter(|route| route.header.table != 255)
        .collect())
}

pub async fn del_routes_noblock(msgs: Vec<RouteMessage>) -> Result<()> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    for msg in msgs {
        del_routes(&handle, msg).await?
    }
    Ok(())
}

pub async fn load_routes(msgs: Vec<RouteMessage>) -> Result<()> {
    let (connection, mut handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    for route in msgs {
        IPRoute {
            action: Action::Add,
            msg: route.clone(),
        }
        .execute(&mut handle)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("can not recover ROUTE MSG: {:?}, error: {}", route, e)
        });
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::proxy::net::routes::{del_routes_noblock, get_routes_noblock, load_routes};

    #[ignore]
    #[tokio::test]
    async fn test_get_del_routes() {
        let mut routes = get_routes_noblock().await.unwrap();
        del_routes_noblock(routes.clone()).await.unwrap();

        routes.reverse();

        load_routes(routes).await.unwrap();
    }
}
