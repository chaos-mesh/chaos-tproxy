use std::cell::Cell;
use std::convert::TryInto;

use log::logger;
use rs_tproxy_plugin::logger::{Metadata, Record};
use wasmer_runtime::{Array, Ctx, WasmPtr};

fn read_data(ctx: &mut Ctx, ptr: WasmPtr<u8, Array>, len: u32) -> Option<Vec<u8>> {
    let memory = ctx.memory(0);
    Some(
        ptr.deref(memory, 0, len)?
            .into_iter()
            .map(Cell::get)
            .collect(),
    )
}

pub fn log_enabled(ctx: &mut Ctx, ptr: WasmPtr<u8, Array>, len: u32) -> i32 {
    let data = read_data(ctx, ptr, len).unwrap();
    let meta = serde_json::from_slice::<Metadata>(&data)
        .unwrap()
        .try_into()
        .unwrap();
    if logger().enabled(&meta) {
        1
    } else {
        0
    }
}

pub fn log_log(ctx: &mut Ctx, ptr: WasmPtr<u8, Array>, len: u32) {
    let data = read_data(ctx, ptr, len).unwrap();
    let raw_record = serde_json::from_slice::<Record>(&data).unwrap();
    logger().log(
        &raw_record
            .build(format_args!("{}", raw_record.content))
            .unwrap(),
    )
}

pub fn log_flush() {
    logger().flush()
}
