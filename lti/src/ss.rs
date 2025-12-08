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
pub struct Ss {
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
impl Ss {
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
        ret.new_statics("A", Matrix::<f64>::new(1,1), None);
        ret.new_statics("B", Matrix::<f64>::new(1,1), None);
        ret.new_statics("C", Matrix::<f64>::new(1,1), None);
        ret.new_statics("D", Matrix::<f64>::new(1,1), None);
        ret.new_statics("x0", Matrix::<f64>::new(1,1), None);
        ret.new_input::<Vec<f64>>("input");
        ret.new_output::<Vec<f64>>("output");
        ret
    }
}
impl StreamProcessor for Ss {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let A = self.get_statics::<Matrix<f64>>("A")?.get_value();
        let B = self.get_statics::<Matrix<f64>>("B")?.get_value();
        let C = self.get_statics::<Matrix<f64>>("C")?.get_value();
        let D = self.get_statics::<Matrix<f64>>("D")?.get_value();
        let x0 = self.get_statics::<Matrix<f64>>("x0")?.get_value();
        if A.is_square() == false {
            return Err(StreamingError::InvalidStatics)
        }
        if A.rows != B.rows {
            return Err(StreamingError::InvalidStatics)
        }
        if A.cols != C.cols {
            return Err(StreamingError::InvalidStatics)
        }
        if B.cols != D.cols {
            return Err(StreamingError::InvalidStatics)
        }
        if C.rows != D.rows {
            return Err(StreamingError::InvalidStatics)
        }
        if x0.rows != A.rows || x0.cols != 1 {
            return Err(StreamingError::InvalidStatics)
        }
        self.model = StateSpace::new(A, B, C, D, x0);
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