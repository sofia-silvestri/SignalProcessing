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
pub struct MovingAverage {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
}
impl MovingAverage {
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
impl StreamProcessor for MovingAverage {
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
        let order = self.get_statics::<usize>("order")?.get_value();
        let mut output_signal = Vec::<f64>::new();
        let half_order = order / 2;
        let mut value_sum: f64 = 0.0;
        let mut count: usize = 0;
        let input_signal = self.recv_input::<Vec<f64>>("input")?;
        {
            let _lock = self.lock.lock().unwrap();
            for k in 0..half_order {
                value_sum += input_signal[k];
                count += 1;
            }
            for k in 0..input_signal.len() {
                output_signal.push(value_sum / count as f64);
                if k < order {
                    value_sum += input_signal[k + half_order];
                    count += 1;
                } else if k > input_signal.len() - half_order - 1 {
                    value_sum -= input_signal[k - half_order - 1];
                    count -= 1;
                } else {
                    value_sum += input_signal[k + half_order] - input_signal[k - half_order - 1];
                }
            }
        }
        self.send_output::<Vec<f64>>("output", output_signal)?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}