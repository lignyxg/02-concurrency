use std::fmt::{Debug, Display, Formatter};
use std::ops::{Add, AddAssign, Mul};
use std::sync::mpsc;
use std::{fmt, thread};

use anyhow::anyhow;

use crate::vector::{dot_product, Vector};

pub struct Matrix<T> {
    data: Vec<T>,
    row: usize,
    col: usize,
}

impl<T> Matrix<T> {
    pub fn new(data: impl Into<Vec<T>>, row: usize, col: usize) -> Self {
        Self {
            data: data.into(),
            row,
            col,
        }
    }
    pub fn row(&self, idx: usize) -> Vec<T>
    where
        T: Clone,
    {
        let r = &self.data[idx * self.col..idx * self.col + self.col];
        r.to_vec()
    }

    pub fn col(&self, idx: usize) -> Vec<T>
    where
        T: Copy,
    {
        self.data[idx..]
            .iter()
            .step_by(self.col)
            .copied()
            .collect::<Vec<_>>()
    }
}

impl<T> Display for Matrix<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for i in 0..self.row {
            for j in 0..self.col {
                write!(f, "{}", self.data[i * self.col + j])?;
                if j != self.col - 1 {
                    write!(f, " ")?;
                }
            }
            if i != self.row - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, "}}")
    }
}

impl<T> Debug for Matrix<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Matrix(row={}, col={}):\n{}", self.row, self.col, self)
    }
}

impl<T> Mul for Matrix<T>
where
    T: Copy + Default + Add<Output = T> + AddAssign + Mul<Output = T> + Send + 'static,
{
    type Output = Matrix<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        multiply_concurrent(&self, &rhs).unwrap()
    }
}

pub struct MsgInput<T> {
    idx: usize,
    row: Vector<T>,
    col: Vector<T>,
}

impl<T> MsgInput<T> {
    pub fn new(idx: usize, row: Vec<T>, col: Vec<T>) -> Self {
        Self {
            idx,
            row: Vector::new(row),
            col: Vector::new(col),
        }
    }
}

pub struct MsgOutput<T> {
    idx: usize,
    v: T,
}

pub struct Msg<T> {
    input: MsgInput<T>,
    // sender to send result back
    sender: oneshot::Sender<MsgOutput<T>>,
}

impl<T> Msg<T> {
    pub fn new(input: MsgInput<T>, sender: oneshot::Sender<MsgOutput<T>>) -> Self {
        Self { input, sender }
    }
}

const NUM_THREADS: usize = 4;

pub fn multiply<T>(a: &Matrix<T>, b: &Matrix<T>) -> anyhow::Result<Matrix<T>>
where
    T: Copy + Default + Add<Output = T> + AddAssign + Mul<Output = T>,
{
    if a.col != b.row {
        return Err(anyhow!("Matrix multiply error: a.col != b.row"));
    }

    let mut data = vec![T::default(); a.row * b.col];
    for i in 0..a.row {
        for j in 0..b.col {
            for k in 0..a.col {
                data[i * b.col + j] += a.data[i * a.col + k] * b.data[k * b.col + j];
            }
        }
    }

    Ok(Matrix {
        data,
        row: a.row,
        col: b.col,
    })
}

pub fn multiply_concurrent<T>(a: &Matrix<T>, b: &Matrix<T>) -> anyhow::Result<Matrix<T>>
where
    T: Copy + Default + Add<Output = T> + AddAssign + Mul<Output = T> + Send + 'static,
{
    let senders = (0..NUM_THREADS)
        .map(|_| {
            let (tx, rx) = mpsc::channel::<Msg<T>>();
            thread::spawn(move || {
                for msg in rx {
                    let v = dot_product(msg.input.row, msg.input.col)?;
                    if let Err(e) = msg.sender.send(MsgOutput {
                        idx: msg.input.idx,
                        v,
                    }) {
                        eprintln!("send error: {:?}", e)
                    }
                }
                Ok::<_, anyhow::Error>(())
            });
            tx
        })
        .collect::<Vec<_>>();

    let mut data = vec![T::default(); a.row * b.col];
    let mut receivers = Vec::with_capacity(a.row * b.col);
    for i in 0..a.row {
        for j in 0..b.col {
            let idx = i * b.col + j;
            let msg_input = MsgInput::new(idx, a.row(i), b.col(j));
            let (tx, rx) = oneshot::channel::<MsgOutput<T>>();
            let msg = Msg::new(msg_input, tx);
            if let Err(e) = senders[idx % NUM_THREADS].send(msg) {
                eprintln!("error send from multiply: {:?}", e)
            }
            receivers.push(rx);
        }
    }

    for rx in receivers {
        let output = rx.recv()?;
        data[output.idx] = output.v;
    }
    Ok(Matrix::new(data, a.row, b.col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_multiply() -> anyhow::Result<()> {
        let a = Matrix::new([1, 2, 3, 4, 5, 6], 2, 3);
        let b = Matrix::new([1, 2, 3, 4, 5, 6], 3, 2);
        let c = multiply_concurrent(&a, &b)?;
        assert_eq!(c.row, 2);
        assert_eq!(c.col, 2);
        assert_eq!(format!("{:?}", c), "Matrix(row=2, col=2):\n{22 28, 49 64}");
        Ok(())
    }

    #[test]
    fn test_matrix_mul() -> anyhow::Result<()> {
        let a = Matrix::new([1, 2, 3, 4, 5, 6], 2, 3);
        let b = Matrix::new([1, 2, 3, 4, 5, 6], 3, 2);
        let c = a * b;
        assert_eq!(c.row, 2);
        assert_eq!(c.col, 2);
        assert_eq!(format!("{:?}", c), "Matrix(row=2, col=2):\n{22 28, 49 64}");
        Ok(())
    }
}
