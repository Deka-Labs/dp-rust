use super::Error;

#[derive(Debug, Default, PartialEq)]
pub enum State {
    /// A first state
    #[default]
    Begin,

    /// A start bit generated
    StartGenerated,

    /// Address send
    AddressSend,

    /// Byte read/write.
    ByteProcesseing,

    LastByte,

    /// Failed to transfer
    Fail(Error),

    /// All bytes transfered
    Finished,
}
