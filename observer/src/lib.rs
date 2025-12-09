pub mod alpha_beta_gamma;
pub mod ekf;
pub mod ukf;
use std::ffi::c_char;
use data_model::modules::{Version,ModuleStructFFI};
use processor_engine::stream_processor::StreamProcessor;
use processor_engine::ffi::{TraitObjectRepr, export_stream_processor, get_error_return};
#[unsafe(no_mangle)]
pub static MODULE: ModuleStructFFI  = ModuleStructFFI {
    name: b"\0".as_ptr() as *const c_char,
    description: b"\0".as_ptr() as *const c_char,
    authors: b"\0".as_ptr() as *const c_char,
    release_date: b"\0".as_ptr() as *const c_char,
    version: Version{ major: 0,minor: 0,build: 0},
    dependencies: std::ptr::null(),
    dependency_number: 0,
    provides: std::ptr::null(),
    provides_lengths: 0,
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
        "AlphaBetaGamma" => {
            proc = Box::new(alpha_beta_gamma::AlphaBetaGamma::new(block_name_str));
            export_stream_processor(proc)
        }
        "Ekf" => {
            proc = Box::new(ekf::Ekf::new(block_name_str));
            export_stream_processor(proc)
        }
        "Ukf" => {
            proc = Box::new(ukf::Ukf::new(block_name_str));
            export_stream_processor(proc)
        }
        _ => {
            eprintln!("Processor block {} not found", proc_block_str);
            get_error_return(1)
        }
    }
}