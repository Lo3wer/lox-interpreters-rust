use std::cell::RefCell;
use std::ffi::{CStr, c_char};
use std::marker::PhantomData;
use std::rc::Rc;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LoxValue {
    pub tag: u8,
    pub bits: u64,
}

#[repr(C)]
pub struct LoxString {
    pub length: usize,
    pub chars: *mut u8,
}

pub const TAG_NUMBER: u8 = 0;
pub const TAG_BOOL: u8 = 1;
pub const TAG_NIL: u8 = 2;
pub const TAG_STRING: u8 = 3; // bits: *mut LoxString
pub const TAG_CLOSURE: u8 = 4; // bits: *const LoxClosure
pub const TAG_CLASS: u8 = 5; // bits: *const LoxClass
pub const TAG_INSTANCE: u8 = 6; // bits: *const LoxInstance
pub const TAG_BOUND_METHOD: u8 = 7; // bits: *const LoxBoundMethod
pub const TAG_UNDEFINED: u8 = 8; // internal sentinel for uninitialized globals

struct OwnedString {
    _object: Box<LoxString>,
    _bytes: Box<[u8]>,
}

#[derive(Default)]
struct StringHeap {
    strings: Vec<OwnedString>,
}

impl StringHeap {
    fn alloc(&mut self, mut bytes: Box<[u8]>) -> *mut LoxString {
        let mut object = Box::new(LoxString {
            length: bytes.len(),
            chars: bytes.as_mut_ptr(),
        });

        let ptr = object.as_mut() as *mut LoxString;

        self.strings.push(OwnedString {
            _object: object,
            _bytes: bytes,
        });

        ptr
    }
}

thread_local! {
    static ACTIVE_STRING_HEAP: RefCell<Option<StringHeap>> =
        const { RefCell::new(None) };
}

// Keep the guard on the thread whose heap it owns.
pub(crate) struct StringHeapGuard(PhantomData<Rc<()>>);

impl StringHeapGuard {
    pub(crate) fn new() -> Result<Self, &'static str> {
        ACTIVE_STRING_HEAP.with(|slot| {
            let mut heap = slot.borrow_mut();
            if heap.is_some() {
                return Err("nested JIT execution is not supported");
            }
            *heap = Some(StringHeap::default());
            Ok(Self(PhantomData))
        })
    }
}

impl Drop for StringHeapGuard {
    fn drop(&mut self) {
        ACTIVE_STRING_HEAP.with(|slot| drop(slot.borrow_mut().take()));
    }
}

fn alloc_owned_string(bytes: Box<[u8]>) -> *mut LoxString {
    ACTIVE_STRING_HEAP.with(|slot| {
        let mut heap = slot.borrow_mut();
        // Allocation without an active execution is an internal contract error.
        // Do not unwind through the C ABI.
        let Some(heap) = heap.as_mut() else {
            std::process::abort();
        };
        heap.alloc(bytes)
    })
}

/// # Safety
/// The value must reference a live, immutable LoxString for this borrow.
unsafe fn string_bytes(value: &LoxValue) -> &[u8] {
    let string = unsafe { &*(value.bits as *const LoxString) };
    if string.length == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(string.chars, string.length) }
}

/// # Safety
/// A heap guard must be active. Nonempty input must contain `len` readable bytes.
/// The returned object is valid until the guard is dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lox_string_alloc(data: *const u8, len: usize) -> *mut LoxString {
    let bytes = if len == 0 {
        Vec::new().into_boxed_slice()
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }
            .to_vec()
            .into_boxed_slice()
    };
    alloc_owned_string(bytes)
}

/// # Safety
/// Both operands must be live strings and a heap guard must be active.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lox_string_concat(a: LoxValue, b: LoxValue) -> LoxValue {
    let left = unsafe { string_bytes(&a) };
    let right = unsafe { string_bytes(&b) };
    let Some(length) = left
        .len()
        .checked_add(right.len())
        .filter(|length| *length <= isize::MAX as usize)
    else {
        std::process::abort();
    };
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(left);
    bytes.extend_from_slice(right);
    LoxValue {
        tag: TAG_STRING,
        bits: alloc_owned_string(bytes.into_boxed_slice()) as u64,
    }
}

/// # Safety
/// A string value must reference a live object and buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lox_print_value(v: LoxValue) {
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
        TAG_STRING => println!("{}", String::from_utf8_lossy(unsafe { string_bytes(&v) })),
        _ => {}
    }
}

/// # Safety
/// A non-null message must point to a readable NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lox_runtime_error(line: u32, msg: *const c_char) {
    if msg.is_null() {
        eprintln!("[Error]: Received a null pointer");
        return;
    }
    let message = unsafe { CStr::from_ptr(msg) }.to_string_lossy();
    eprintln!("[line {line}] Error: {message}");
}

#[used]
static RUNTIME_FNS: [unsafe extern "C" fn(LoxValue); 1] = [lox_print_value];
