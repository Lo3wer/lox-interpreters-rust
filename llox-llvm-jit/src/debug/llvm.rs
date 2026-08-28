use inkwell::module::Module;
use inkwell::targets::{FileType, TargetMachine};

pub fn dump_ir(module: &Module<'_>) {
    println!("LLVM IR Representation:\n{}", module.print_to_string());
}

pub fn dump_assembly(module: &Module<'_>, machine: &TargetMachine) {
    let buffer = machine
        .write_to_memory_buffer(module, FileType::Assembly)
        .expect("failed to generate assembly representation");

    println!(
        "Assembly Representation:\n{}",
        std::str::from_utf8(buffer.as_slice()).expect("assembly was not valid UTF-8")
    );
}
