use std::collections::HashMap;
use std::any::Any;
use std::fmt::Debug;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use rustfft::{FftPlanner, Fft, num_complex::Complex};
use stream_proc_macro::{StreamBlockMacro};
use data_model::streaming_data::{StreamingError, StreamingState};
use data_model::memory_manager::{DataTrait, StaticsTrait, State, Parameter, Statics};
use processor_engine::stream_processor::{StreamBlock, StreamBlockDyn, StreamProcessor};
use processor_engine::connectors::{ConnectorTrait, Input, Output};

#[derive(StreamBlockMacro)]
pub struct FftProcessor {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
    fft_core:   Option<Arc<dyn Fft<f64>>>,
}
impl FftProcessor {
    pub fn new(name: &'static str) -> Self {
        let mut ret = Self {
            name,
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            parameters: HashMap::new(),
            statics: HashMap::new(),
            state: HashMap::new(),
            lock: Arc::new(Mutex::new(())),
            proc_state: Arc::new(Mutex::new(StreamingState::Null)),
            fft_core: None,
        };
        ret.new_input::<Vec<f64>>("real_signal");
        ret.new_input::<Vec<Complex<f64>>>("complex_signal");
        ret.new_output::<Vec<Complex<f64>>>("output_transform");
        ret.new_statics::<usize>("fft_size", 1024, None);
        ret.new_statics::<bool>("inverse", false, None);
        ret.new_statics::<bool>("complex_input", false, None);
        ret
    }
}
impl StreamProcessor for FftProcessor {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let fft_size = self.get_statics::<usize>("fft_size")?.get_value();
        let inverse = self.get_statics::<bool>("inverse")?.get_value();
        let mut planner = FftPlanner::new();
        if inverse {
            self.fft_core = Some(planner.plan_fft_inverse(fft_size));
        } else {
            self.fft_core = Some(planner.plan_fft_forward(fft_size));
        }
        self.set_state(StreamingState::Initial);
        Ok(())
    }
    fn run(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Stopped) {
            return Err(StreamingError::InvalidStateTransition);
        }
        self.set_state(StreamingState::Running);
        while !self.check_state(StreamingState::Stopped) {
            self.process()?;
        }
        Ok(())
    }
    fn process(&mut self) -> Result<(), StreamingError> {
        let complex_input = self.get_statics::<bool>("complex_input")?.get_value();
        if complex_input {
            let mut input_signal = self.recv_input::<Vec<Complex<f64>>>("complex_signal")?;
            self.fft_core.as_ref().unwrap().process(&mut input_signal);
            self.send_output::<Vec<Complex<f64>>>("output_transform", input_signal)?;
        } else {
            let mut input_signal = self.recv_input::<Vec<f64>>("real_signal")?;
            let mut input_signal: Vec<Complex<f64>> = input_signal.into_iter()
                .map(|x| Complex{ re: x, im: 0.0 })
                .collect();
            self.fft_core.as_ref().unwrap().process(&mut input_signal);
            self.send_output::<Vec<Complex<f64>>>("output_transform", input_signal)?;
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    #[test]
    fn test_fft() {
        let size = 16384;
        let repetition = 10000;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(size);

        let mut signal: Vec<Complex<f64>> = (0..size)
            .map(|x| Complex::new(x as f64, 0.0))
            .collect();
        let start = Instant::now();
        for _ in 0..repetition {
            let _ = fft.process(&mut signal);
        }
        let duration = start.elapsed();
        println!("Mean time is: {:?}", (duration.as_secs_f64()) / repetition as f64);
    }
}