#[derive(Debug, Clone)]
pub enum Plugin {
    WASM(Vec<u8>),
}
