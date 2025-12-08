use num_traits::zero;
use serde::{Deserialize, Serialize};
use utils::math::matrix::Matrix;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSpace {
    A: Matrix<f64>,
    B: Matrix<f64>,
    C: Matrix<f64>,
    D: Matrix<f64>,
    x: Matrix<f64>, // State vector
}

impl StateSpace {
    pub fn new(A: Matrix<f64>, B: Matrix<f64>, C: Matrix<f64>, D: Matrix<f64>, x0: Matrix<f64>) -> Self {
        StateSpace { A, B, C, D, x: x0 }
    }
    pub fn from_tf(num: Vec<f64>, den: Vec<f64>, x0: Vec<f64>) -> Self {
        let n = den.len() - 1;
        
        let mut A = Matrix::<f64>::zero(n, n);
        let mut B = Matrix::<f64>::zero(n, 1);
        let mut C = Matrix::<f64>::zero(1, n);
        let mut D = Matrix::<f64>::zero(1, 1);
        C.set(n-1, 0, 1.0).unwrap();
        if num.len() == den.len() {
            D.set(0, 0, num[0] / den[0]).unwrap();
        }
        for k in 0..n {
            if k < n - 1 {
                A.set(k, k + 1, 1.0).unwrap();
            }
            A.set(n - 1, k, -den[k + 1] / den[0]).unwrap();
        }
        for k in 1..num.len() {
            B.set(k - 1, 0, num[k] / den[0]).unwrap();
        }
        StateSpace { A, B, C, D, x: Matrix::from_vec(x0.into_iter().map(|v| vec![v]).collect()) }
    }
    pub fn from_zpk(zeros: Vec<f64>, poles: Vec<f64>, gain: f64, x0: Vec<f64>) -> Self {
        let n = poles.len();
        let mut num = vec![1.0; 1];
        let mut den = vec![1.0; 1];
        for i in 0..poles.len() {
            den = StateSpace::cauchy(den, vec![-poles[i], 1.0]);
        }
        for i in 0..zeros.len() {
            num = StateSpace::cauchy(num, vec![-zeros[i], 1.0]);
        }
        num = num.into_iter().map(|x| x * gain).collect();
        num.reverse();
        den.reverse();
        while num.len() < den.len() {
            num.insert(0, 0.0);
        }
        StateSpace::from_tf(num, den, x0)
    }
    pub fn cauchy(a: Vec<f64>, b: Vec<f64>) -> Vec<f64> {
        let mut result = vec![0.0; a.len() + b.len() - 1];
        for i in 0..a.len() {
            for j in 0..b.len() {
                result[i + j] += a[i] * b[j];
            }
        }
        result
    }
    pub fn update(&mut self, u: &Matrix<f64>) -> Matrix<f64> {
        // x(k+1) = A*x(k) + B*u(k)
        self.x = self.A.clone() * self.x.clone() + self.B.clone() * u.clone();
        // y(k) = C*x(k) + D*u(k)
        let y = self.C.clone() * self.x.clone() + self.D.clone() * u.clone();
        y
    }
    pub fn get_input_size(&self) -> usize {
        self.B.cols
    }
}