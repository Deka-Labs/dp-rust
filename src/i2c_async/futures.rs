use core::{
    sync::atomic::{AtomicBool, Ordering},
    task::Poll,
};

use super::{Error, TRANSACTION};

pub struct I2COperationFuture {
    future_read: &'static AtomicBool,
}

impl I2COperationFuture {
    pub fn new(future_read: &'static AtomicBool) -> Self {
        Self { future_read }
    }

    pub fn ready(&self) -> Poll<Result<(), Error>> {
        let ctx = unsafe { &mut TRANSACTION };

        if ctx.finished() {
            use super::states::State;
            self.future_read.store(true, Ordering::Relaxed);

            return match ctx.state {
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
