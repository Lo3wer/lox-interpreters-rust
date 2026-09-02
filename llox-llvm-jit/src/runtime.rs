#[unsafe(no_mangle)]
pub extern "C" fn lox_print_number(x: f64) {
    if x.fract() == 0.0 {
        println!("{:.0}", x);
    } else {
        println!("{}", x);
    }
}

#[used]
static RUNTIME_FNS: [extern "C" fn(f64); 1] = [lox_print_number];
