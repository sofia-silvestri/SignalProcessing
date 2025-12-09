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

use std::time::SystemTime;

#[derive(StreamBlockMacro)]
pub struct AlphaBetaGamma {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
}
impl AlphaBetaGamma {
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
        let _ = ret.new_input::<f64>("input");
        let _ = ret.new_output::<f64>("output");
        let _ = ret.new_statics::<f64>("alpha",0.0,None);
        let _ = ret.new_statics::<f64>("beta",0.0,None);
        let _ = ret.new_statics::<f64>("gamma",0.0,None);
        let _ = ret.new_state::<Vec<f64>>("state", vec![0.0; 3]);
        let _ = ret.new_state::<SystemTime>("last_update", SystemTime::now());
        let _ = ret.new_state::<bool>("init", false);
        ret
    }
}
impl StreamProcessor for AlphaBetaGamma {
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
        let alpha = self.get_statics::<f64>("alpha")?.get_value();
        let beta = self.get_statics::<f64>("beta")?.get_value();
        let gamma = self.get_statics::<f64>("gamma")?.get_value();
        let mut state = self.get_state_value::<Vec<f64>>("state")?;
        let init = self.get_state_value::<bool>("init")?;
        let mut last_update = self.get_state_value::<SystemTime>("last_update")?;
        let input_signal = self.recv_input::<f64>("input")?;
        {
            let _lock = self.lock.lock().unwrap();
            if init {
                let delta_time = (last_update.elapsed().unwrap().as_micros() as f64)/1.0e6;
                state[0] = state[0] + state[1]*delta_time + 0.5*delta_time*delta_time*state[2];
                let error = input_signal - state[0];
                state[0] = state[0] + alpha*error;
                state[1] = state[1] + beta*error;
                state[2] = state[2] + gamma*error;
            } else {
                state[0] = input_signal;
                
            }
            last_update = SystemTime::now();
        }
        self.set_state_value("last_update", last_update);
        self.set_state_value("init", true);
        self.set_state_value("state", state.clone());
        self.send_output::<f64>("output", state[0]);
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}