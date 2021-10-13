mod buildin {
    extern "C" {
        pub fn println(ptr: *const u8, len: u32);
        pub fn eprintln(ptr: *const u8, len: u32);
    }
}

pub fn println(str: impl AsRef<str>) {
    let data = str.as_ref().as_bytes();
    unsafe { buildin::println(data.as_ptr(), data.len() as u32) }
}

pub fn eprintln(str: impl AsRef<str>) {
    let data = str.as_ref().as_bytes();
    unsafe { buildin::eprintln(data.as_ptr(), data.len() as u32) }
}
