use std::collections::HashMap;
use std::any::Any;
use std::fmt::Debug;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use stream_proc_macro::{StreamBlockMacro};
use data_model::streaming_data::{StreamingError, StreamingState};
use data_model::memory_manager::{DataTrait, StaticsTrait, State, Parameter, Statics};
use processor_engine::stream_processor::{StreamBlock, StreamBlockDyn, StreamProcessor};
use processor_engine::connectors::{ConnectorTrait, Input, Output};

#[derive(StreamBlockMacro)]
pub struct MedianFilter {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
}
impl MedianFilter {
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
        };
        ret.new_input::<Vec<f64>>("input");
        ret.new_output::<Vec<f64>>("output");
        ret.new_statics::<usize>("order", 0, None);
        ret
    }
}
impl StreamProcessor for MedianFilter {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        self.set_state(StreamingState::Initial);
        Ok(())
    }
    fn run(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Stopped) {
            return Err(StreamingError::InvalidStateTransition);
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        self.set_state(StreamingState::Running);
        while !self.check_state(StreamingState::Stopped) {
            self.process()?;
        }
        Ok(())
    }
    fn process(&mut self) -> Result<(), StreamingError> {
        let output_signal: Vec<f64>;
        let order = self.get_statics::<usize>("order")?.get_value();
        {
            let input_signal = self.recv_input::<Vec<f64>>("input")?;
            let _lock = self.lock.lock().unwrap();
            let mut input_window: Vec<f64> = Vec::with_capacity(order);
            output_signal = input_signal.iter().map(|&x| {
                input_window.push(x);
                if input_window.len() > order {
                    input_window.remove(0);
                }
                let mut sorted_window = input_window.clone();
                sorted_window.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let median = if sorted_window.len() % 2 == 1 {
                    sorted_window[sorted_window.len() / 2]
                } else {
                    let mid = sorted_window.len() / 2;
                    (sorted_window[mid - 1] + sorted_window[mid]) / 2.0
                };
                median
            }).collect();
        }
        self.send_output::<Vec<f64>>("output", output_signal)?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}