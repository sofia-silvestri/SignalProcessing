use core::num;
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
use utils::math::matrix::Matrix;
use crate::state_space::StateSpace;

#[derive(StreamBlockMacro)]
pub struct Tf {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
    model:      StateSpace,
}
impl Tf {
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
            model: StateSpace::new(Matrix::new(1,1), Matrix::new(1,1), Matrix::new(1,1), Matrix::new(1,1), Matrix::new(1,1)),

        };
        let _ = ret.new_input::<Vec<f64>>("input");
        let _ = ret.new_output::<Vec<f64>>("output");
        let _ = ret.new_statics::<Vec<f64>>("numerator", vec![1.0], None);
        let _ = ret.new_statics::<Vec<f64>>("denominator", vec![1.0], None);
        let _ = ret.new_statics::<Vec<f64>>("x0", vec![0.0], None);
        ret
    }
}
impl StreamProcessor for Tf {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let mut numerator = self.get_statics::<Vec<f64>>("numerator")?.get_value();
        let denominator = self.get_statics::<Vec<f64>>("denominator")?.get_value();
        let x0_vec = self.get_statics::<Vec<f64>>("x0")?.get_value();
        if denominator.len() == 0 || denominator[0] == 0.0 {
            return Err(StreamingError::InvalidStatics)
        }
        let size = denominator.len();
        if numerator.len() == 0 {
            return Err(StreamingError::InvalidStatics)
        }
        while numerator.len() < size {
            numerator.insert(0, 0.0);
        }
        self.model = StateSpace::from_tf(numerator, denominator, x0_vec);
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
        let input = self.recv_input::<Vec<f64>>("input")?;
        let u = Matrix::from_vec(input.into_iter().map(|v| vec![v]).collect());
        if u.rows != self.model.get_input_size() || u.cols != 1 {
            self.stop()?;
            return Err(StreamingError::InvalidInput);
        }
        let y: Matrix<f64>;
        {
            let _guard = self.lock.lock().unwrap();
            y = self.model.update(&u);
        }
        self.send_output::<Vec<f64>>("output", y.to_vec().into_iter().map(|v| v[0]).collect())?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}