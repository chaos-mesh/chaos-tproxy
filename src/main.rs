pub mod tproxy;
pub mod parser;
use tproxy::tproxy::Tproxy;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Tproxy().await?;
    Ok(())
}
