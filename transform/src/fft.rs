use std::collections::HashMap;
use std::any::Any;
use std::fmt::Display;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use stream_proc_macro::{StreamBlockMacro};
use data_model::streaming_data::{StreamingError, StreamingState};
use data_model::memory_manager::{DataTrait, StaticsTrait, State, Parameter, Statics};
use processor_engine::stream_processor::{StreamBlock, StreamBlockDyn, StreamProcessor};
use processor_engine::connectors::{ConnectorTrait, Input, Output};
use utils::math::complex::Complex;
use utils::math::numbers::factorize;

pub struct FftCore {
    size: usize,
    weights: Vec<Complex<f64>>,
    factorization: Vec<usize>,
}

impl FftCore
{
    pub fn new(inverse: bool, size: usize) -> Self {
        let mut weights: Vec<Complex<f64>> = Vec::with_capacity(size);
        for i in 0..size {
            let angle: f64 = if inverse {
                2.0 * std::f64::consts::PI * (i as f64) / (size as f64)
            } else {
                -2.0 * std::f64::consts::PI * (i as f64) / (size as f64)
            };
            weights.push(Complex::new(angle.cos(), angle.sin()));
        }
        let factorization = factorize(size as u64)
            .iter()
            .map(|&x| x as usize)
            .collect();
        Self {
            size,
            weights,
            factorization: factorization,
        }
    }

    fn fft_process(&self,
        input: &Vec<Complex<f64>>,
        size: usize,
        index_factor: usize,
        start: usize,
        step: usize) -> Vec<Complex<f64>>
    {
        if size == 1 {
            return input.clone();
        }
        let mut output : Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); size];

        if size == 2 {
            output[0] = input[0].clone() + input[1].clone();
            output[1] = input[0].clone() - input[1].clone();
            
            return output;
        }
        let chunk_number = self.factorization[index_factor];
        let chunk_size = size / chunk_number;
        let mut chunks : Vec<Vec<Complex<f64>>> = vec![vec![Complex::new(0.0, 0.0); chunk_size]; chunk_number];
        for i in 0..chunk_number {
            let chunk_fft = self.fft_process(
                input,
                chunk_size,
                index_factor + 1,
                start + i * step,
                step * chunk_number,
            );
            chunks[i] = chunk_fft;
        }
        for k in 0..size {
            for i in 0..chunk_number as usize {
                let index_sel = k % chunk_size as usize;
                output[k] += chunks[i][index_sel] * self.weights[index_sel];
            }
        }
        output
    }

    pub fn fft_real(&self, input: &Vec<f64>) -> Vec<Complex<f64>>
    {
        let mut input: Vec<Complex<f64>> = input.iter().map(|&x| Complex::new(x, 0.0)).collect();
        if input.len() < self.size {
            let mut padded_input = input.clone();
            for _ in input.len()..self.size {
                padded_input.push(Complex::new(0.0, 0.0));
            }
            input = padded_input;
        }
        self.fft_process(&input, self.size, 0, 0, 1)
    }
    pub fn fft_complex(&self, input: &Vec<Complex<f64>>) -> Vec<Complex<f64>> 
    {
        if input.len() < self.size {
            let mut padded_input = input.clone();
            for _ in input.len()..self.size {
                padded_input.push(Complex::new(0.0, 0.0));
            }
            return self.fft_process(&padded_input, self.size, 0, 0, 1);
        }
        self.fft_process(input, self.size, 0, 0, 1)
    }
}

#[derive(StreamBlockMacro)]
pub struct Fft {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
    fft_core:   Option<FftCore>,
}
impl Fft {
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
impl StreamProcessor for Fft {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let fft_size = self.get_statics::<usize>("fft_size")?.get_value();
        let inverse = self.get_statics::<bool>("inverse")?.get_value();
        self.fft_core = Some(FftCore::new(inverse, fft_size));
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
        let fft_size = self.get_statics::<usize>("fft_size")?.get_value();
        let complex_input = self.get_statics::<bool>("complex_input")?.get_value();
        let output_signal: Vec<Complex<f64>>;

        if complex_input {
            let input_signal = self.recv_input::<Vec<Complex<f64>>>("complex_signal")?;
            output_signal = self.fft_core.as_ref().unwrap().fft_complex(&input_signal);
        } else {
            let input_signal = self.recv_input::<Vec<f64>>("real_signal")?;
            output_signal = self.fft_core.as_ref().unwrap().fft_real(&input_signal);
        }
        self.send_output::<Vec<Complex<f64>>>("output_transform", output_signal)?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}