use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};

use super::{Error, TRANSACTION};

pub struct I2COperationFuture {
    position: usize,
}

impl I2COperationFuture {
    pub fn new(position: usize) -> Self {
        Self { position }
    }

    pub fn ready(&self) -> Poll<Result<(), Error>> {
        let ctx = unsafe { &mut TRANSACTION };

        if ctx.finished(self.position) {
            use super::states::State;

            return match ctx.states[self.position] {
                State::Fail(e) => Poll::Ready(Err(e)),
                State::Finished => Poll::Ready(Ok(())),
                _ => unreachable!(),
            };
        }

        return Poll::Pending;
    }

    pub fn block(&self) -> Result<(), Error> {
        let mut status = self.ready();
        while let Poll::Pending = status {
            status = self.ready();
        }

        if let Poll::Ready(r) = status {
            return r;
        }
        unreachable!()
    }
}
