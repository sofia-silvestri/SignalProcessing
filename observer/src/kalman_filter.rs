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

use std::time::SystemTime;

#[derive(StreamBlockMacro)]
pub struct KalmanFilter {
    name:       &'static str,
    inputs:     HashMap<&'static str, Box<dyn ConnectorTrait>>,
    outputs:    HashMap<&'static str, Box<dyn ConnectorTrait>>,
    parameters: HashMap<&'static str, Box<dyn DataTrait>>,
    statics:    HashMap<&'static str, Box<dyn StaticsTrait>>,
    state:      HashMap<&'static str, Box<dyn DataTrait>>,
    lock:       Arc<Mutex<()>>,
    proc_state: Arc<Mutex<StreamingState>>,
}
impl KalmanFilter {
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
        let _ = ret.new_input::<Vec<f64>>("input");
        let _ = ret.new_output::<Vec<f64>>("output");
        let _ = ret.new_statics::<Matrix<f64>>("A", Matrix::identity(1), None);
        let _ = ret.new_statics::<Matrix<f64>>("B", Matrix::identity(1), None);
        let _ = ret.new_statics::<Matrix<f64>>("H", Matrix::identity(1), None);
        let _ = ret.new_statics::<Matrix<f64>>("Q", Matrix::identity(1), None);
        let _ = ret.new_statics::<Matrix<f64>>("R", Matrix::identity(1), None);
        let _ = ret.new_statics::<Matrix<f64>>("P0", Matrix::identity(1), None);
        let _ = ret.new_statics::<Vec<f64>>("initial_state", vec![], None);
        let _ = ret.new_state::<Vec<f64>>("state", vec![]);
        let _ = ret.new_state::<Matrix<f64>>("P", Matrix::identity(1));
        ret
    }
}
impl StreamProcessor for KalmanFilter {
    fn init(&mut self) -> Result<(), StreamingError> {
        if self.check_state(StreamingState::Running) {
            return Err(StreamingError::InvalidStateTransition)
        }
        if !self.is_initialized() {
            return Err(StreamingError::InvalidStatics)
        }
        let A = self.get_statics::<Matrix<f64>>("A")?.get_value();
        if !A.is_square() {
            return Err(StreamingError::InvalidStatics)
        }
        let B = self.get_statics::<Matrix<f64>>("B")?.get_value();
        if B.rows != A.rows {
            return Err(StreamingError::InvalidStatics)
        }
        let H = self.get_statics::<Matrix<f64>>("H")?.get_value();
        if H.cols != A.rows {
            return Err(StreamingError::InvalidStatics)
        }
        let Q = self.get_statics::<Matrix<f64>>("Q")?.get_value();
        if Q.rows != A.rows || !Q.is_square() {
            return Err(StreamingError::InvalidStatics)
        }
        let R = self.get_statics::<Matrix<f64>>("R")?.get_value();
        if R.rows != H.rows || !R.is_square() {
            return Err(StreamingError::InvalidStatics)
        }
        let P0 = self.get_statics::<Matrix<f64>>("P0")?.get_value();
        if P0.rows != A.rows || !P0.is_square() {
            return Err(StreamingError::InvalidStatics)
        }
        let initial_state = self.get_statics::<Vec<f64>>("initial_state")?.get_value();
        if initial_state.len() != A.rows {
            return Err(StreamingError::InvalidStatics)
        }
        let _ = self.set_state_value("state", initial_state.clone());
        let _ = self.set_state_value("P", P0.clone());
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
        let A = self.get_statics::<Matrix<f64>>("A")?.get_value();
        let B = self.get_statics::<Matrix<f64>>("B")?.get_value();
        let H = self.get_statics::<Matrix<f64>>("H")?.get_value();
        let Q = self.get_statics::<Matrix<f64>>("Q")?.get_value();
        let R = self.get_statics::<Matrix<f64>>("R")?.get_value();
        let mut P = self.get_state_value::<Matrix<f64>>("P")?;
        let mut state = self.get_state_value::<Vec<f64>>("state")?;
        let input = self.recv_input::<Vec<f64>>("input")?;
        {
            let _lock = self.lock.lock().unwrap();
            let u = Matrix::from_vec(vec![input.clone()]).transpose();
            let x_prior = &A * &Matrix::from_vec(vec![state.clone()]) + &B * &u;
            let P_prior = &A * &P * A.transpose() + Q;
            let y = &Matrix::from_vec(vec![input.clone()]).transpose() - &(&H * &x_prior);
            let S = &H * &P_prior * H.transpose() + R;
            let K = &P_prior * &H.transpose() * S.inverse().unwrap();
            let x_post = &x_prior + &(&K * &y);
            P = (Matrix::identity(K.rows) - &K * &H) * P_prior;
            state = x_post.to_vec()[0].clone();
        }
        let _ = self.set_state_value("state", state.clone());
        let _ = self.set_state_value("P", P);
        self.send_output("output", state)?;
        Ok(())
    }
    fn stop(&mut self) -> Result<(), StreamingError> {
        self.set_state(StreamingState::Stopped);
        Ok(())
    }
}