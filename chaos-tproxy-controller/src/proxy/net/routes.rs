use rtnetlink::{IpVersion, new_connection};
use rtnetlink::packet::RouteMessage;
use anyhow::Result;
use iproute2_rs::ip::iproute::{Action, del_routes, get_routes, IPRoute};

pub fn get_routes_noblock() -> Result<Vec<RouteMessage>> {
    let routes:Vec<RouteMessage> = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on( async {
            let (connection, handle, _) = new_connection().unwrap();
            tokio::spawn(connection);
            get_routes(&handle, IpVersion::V4).await
        })?;

    Ok(routes.into_iter().filter(|route| route.header.table != 255).collect())
}

pub fn del_routes_noblock(msgs :Vec<RouteMessage>) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on( async {
            let (connection, handle, _) = new_connection().unwrap();
            tokio::spawn(connection);
            for msg in msgs {
                del_routes(&handle,msg).await?
            }
            Ok(())
        }
        )
}

pub fn load_routes(msgs : Vec<RouteMessage>) -> Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on( async {
            let (connection, mut handle, _) = new_connection().unwrap();
            tokio::spawn(connection);

            for route in msgs {
                IPRoute {
                    action: Action::Add,
                    msg: route.clone(),
                }
                    .execute(&mut handle)
                    .await.unwrap_or_else(|e|
                    tracing::error!("can not recover ROUTE MSG: {:?}, error: {}",route, e)
                );
            }
            Ok(())
        })
}

#[cfg(test)]
mod test {
    use crate::proxy::net::routes::{del_routes_noblock, get_routes_noblock, load_routes};

    #[test]
    fn test_get_del_routes() {
        let mut routes = get_routes_noblock().unwrap();
        del_routes_noblock(routes.clone()).unwrap();

        routes.reverse();

        load_routes(routes).unwrap();
    }
}