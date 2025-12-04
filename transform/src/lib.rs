pub mod fft;
use std::ffi::c_char;
use data_model::modules::{Version,ModuleStructFFI};
use processor_engine::stream_processor::StreamProcessor;
use processor_engine::ffi::{TraitObjectRepr, export_stream_processor, get_error_return};
#[unsafe(no_mangle)]
pub static MODULE: ModuleStructFFI  = ModuleStructFFI {
    name: b"Digital Fourier Transform\0".as_ptr() as *const c_char,
    description: b"The library provides digital Fourier transform functionalities.\0".as_ptr() as *const c_char,
    authors: b"Sofia Silvestri\0".as_ptr() as *const c_char,
    release_date: b"2025/12/06\0".as_ptr() as *const c_char,
    version: Version{ major: 1,minor: 0,build: 0},
    dependencies: std::ptr::null(),
    dependency_number: 0,
    provides: [b"Fft\0".as_ptr() as *const c_char].as_ptr(),
    provides_lengths: 1,
};
#[unsafe(no_mangle)]
pub extern "C" fn get_processor_modules(proc_block: *const u8, 
    proc_block_len: usize, 
    block_name: *const u8, 
    block_name_len: usize) -> TraitObjectRepr {
    let proc_block_str = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(proc_block, proc_block_len)).unwrap()
    };
    let block_name_str = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(block_name, block_name_len)).unwrap()
    };
    let proc: Box<dyn StreamProcessor>;
    match proc_block_str {
        "Fft" => {
            proc = Box::new(fft::FftProcessor::new(block_name_str));
            export_stream_processor(proc)
        }
        _ => {
            eprintln!("Processor block {} not found", proc_block_str);
            get_error_return(1)
        }
    }
}