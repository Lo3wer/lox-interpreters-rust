use std::ffi::CStr;
use std::ffi::c_char;
use std::process;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoxValue {
    pub tag: u8,
    pub bits: u64,
}

pub const TAG_NUMBER: u8 = 0;
pub const TAG_BOOL: u8 = 1;
pub const TAG_NIL: u8 = 2;
pub const TAG_STRING: u8 = 3;
pub const TAG_CLOSURE: u8       = 4; // bits: *const LoxClosure
pub const TAG_CLASS: u8         = 5; // bits: *const LoxClass
pub const TAG_INSTANCE: u8      = 6; // bits: *const LoxInstance
pub const TAG_BOUND_METHOD: u8  = 7; // bits: *const LoxBoundMethod

#[unsafe(no_mangle)]
pub extern "C" fn lox_print_value(v: LoxValue) {
    match v.tag {
        TAG_NUMBER => {
            let n = f64::from_bits(v.bits);
            if n.fract() == 0.0 {
                println!("{:.0}", n);
            } else {
                println!("{n}");
            }
        }
        TAG_BOOL => println!("{}", v.bits != 0),
        TAG_NIL => println!("nil"),
        TAG_STRING => println!("{}", unsafe { CStr::from_ptr(v.bits as *const i8) }.to_string_lossy()),
        _ => {}
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lox_runtime_error(line: u32, msg: *const c_char) {
    if msg.is_null() {
        eprintln!("[Error]: Received a null pointer");
        return;
    }
    let message = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    eprintln!("[line {line}] Error: {message}");
    process::exit(70);
}

#[used]
static RUNTIME_FNS: [extern "C" fn(LoxValue); 1] = [lox_print_value];
