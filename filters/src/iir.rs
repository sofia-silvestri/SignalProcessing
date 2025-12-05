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
pub struct Iir {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
}
impl Iir {
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
        ret.new_statics::<Vec<f64>>("a_coefficient", Vec::<f64>::new(), None);
        ret.new_statics::<Vec<f64>>("b_coefficient", Vec::<f64>::new(), None);
        ret.new_state::<Vec<f64>>("outputs_memory", Vec::<f64>::new());
        ret.new_state::<Vec<f64>>("inputs_memory", Vec::<f64>::new());
        ret
    }
}
impl StreamProcessor for Iir {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let order = self.get_statics::<usize>("order")?.get_value();
        let a_coefficient = self.get_statics::<Vec<f64>>("a_coefficient")?.get_value();
        let b_coefficient = self.get_statics::<Vec<f64>>("b_coefficient")?.get_value();
        if a_coefficient.len() != order + 1 || b_coefficient.len() != order {
            return Err(StreamingError::InvalidStatics);
        }
        let memory = vec![0.0; order];
        self.set_state_value("inputs_memory", memory.clone())?;
        self.set_state_value("outputs_memory", memory)?;
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
        let a_coefficient = self.get_statics::<Vec<f64>>("a_coefficient")?.get_value();
        let b_coefficient = self.get_statics::<Vec<f64>>("b_coefficient")?.get_value();
        let mut input_memory = self.get_state_value::<Vec<f64>>("inputs_memory")?;
        let mut output_memory = self.get_state_value::<Vec<f64>>("outputs_memory")?;
        let order = self.get_statics::<usize>("order")?.get_value();
        let mut output_signal = Vec::<f64>::new();
        let input_signal = self.recv_input::<Vec<f64>>("input")?;
        for k in 0..input_signal.len() {
            let _lock = self.lock.lock().unwrap();
            let mut value = b_coefficient[0]*input_signal[k];
            for index in 1..order {
                value += b_coefficient[index]*input_memory[order - k];
                value += a_coefficient[index]*output_memory[order - k];
            }
            output_signal.push(value);
            output_memory.remove(0);
            output_memory.push(value);
            input_memory.remove(0);
            input_memory.push(input_signal[k]);
        }
        self.set_state_value("inputs_memory", input_memory)?;
        self.set_state_value("outputs_memory", output_memory)?;
        self.send_output::<Vec<f64>>("output", output_signal)?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}