mod accept;
mod stream;

use self::accept::accept_std_stream;
use self::stream::StdStream;
use crate::server_helper::ServeHandler;

pub struct ConfigServer {
    handler: Option<ServeHandler>,
}
