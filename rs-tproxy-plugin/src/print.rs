mod buildin {
    extern "C" {
        pub fn print(ptr: *const u8, len: u32);
    }
}

pub fn print(str: impl AsRef<str>) {
    let data = str.as_ref().as_bytes();
    unsafe { buildin::print(data.as_ptr(), data.len() as u32) }
}
