use wasmer_runtime::{Array, Ctx, WasmPtr};

pub fn println(ctx: &mut Ctx, ptr: WasmPtr<u8, Array>, len: u32) {
    let memory = ctx.memory(0);

    // Use helper method on `WasmPtr` to read a utf8 string
    let string = ptr.get_utf8_string(memory, len).unwrap();

    // Print it!
    println!("{}", string);
}

pub fn eprintln(ctx: &mut Ctx, ptr: WasmPtr<u8, Array>, len: u32) {
    let memory = ctx.memory(0);

    // Use helper method on `WasmPtr` to read a utf8 string
    let string = ptr.get_utf8_string(memory, len).unwrap();

    // Print it!
    eprintln!("{}", string);
}
