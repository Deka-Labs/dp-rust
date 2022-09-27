use core::sync::atomic::AtomicBool;

use super::{states::State, I2COperationFuture};
use heapless::spsc::Queue;

#[derive(Debug, Default)]
pub enum Command<'buf> {
    Read(u8, &'buf mut [u8]),
    Write(u8, &'buf [u8]),

    #[default]
    NoOp,
}

impl<'buf> Command<'buf> {
    pub fn address(&self) -> u8 {
        match self {
            Command::Read(a, _) => *a,
            Command::Write(a, _) => *a,
            Command::NoOp => 0,
        }
    }

    pub fn is_read(&self) -> bool {
        if let Command::Read(_, _) = self {
            return true;
        }
        return false;
    }

    pub fn is_write(&self) -> bool {
        if let Command::Write(_, _) = self {
            return true;
        }
        return false;
    }

    pub fn is_noop(&self) -> bool {
        if let Command::NoOp = self {
            return true;
        }
        return false;
    }

    pub fn write_buf(&self) -> &[u8] {
        assert!(self.is_write());
        if let Command::Write(_, b) = self {
            return b;
        }
        unreachable!()
    }

    pub fn read_buf(&mut self) -> &mut [u8] {
        assert!(self.is_read());
        if let Command::Read(_, b) = self {
            return &mut *b;
        }
        unreachable!()
    }
}

pub struct Transaction<const MAX_COMMANDS: usize> {
    pub(crate) commands: Queue<Command<'static>, MAX_COMMANDS>,

    pub(crate) buffer_position: usize,

    pub(crate) states: [State; MAX_COMMANDS],
    state_position: usize,
}

impl<const MAX_COMMANDS: usize> Transaction<MAX_COMMANDS> {
    pub const fn new() -> Self {
        Self {
            commands: Queue::new(),
            buffer_position: 0,
            states: [State::Begin; MAX_COMMANDS],
            state_position: 0,
        }
    }

    pub fn enqueue_commands<const IN_SIZE: usize>(
        &mut self,
        commands: [Command<'static>; IN_SIZE],
    ) -> Result<I2COperationFuture, [Command<'static>; IN_SIZE]> {
        // This is single producer queue so we should protect it
        critical_section::with(|_| {
            let avaiable_space = self.commands.capacity() - self.commands.len() + 1; // + 1 To insert NoOp command
            if avaiable_space < IN_SIZE {
                return Err(commands); // Not enough space
            }

            // If no commands is executing our position is cureent
            let pos = if self.commands.len() == 0 {
                self.state_position
            } else {
                self.get_next_state_position()
            };

            for c in commands {
                self.commands.enqueue(c).ok();
            }

            self.commands.enqueue(Command::NoOp).ok();

            self.states[pos] = State::Begin;
            Ok(I2COperationFuture::new(pos))
        })
    }

    fn get_next_state_position(&self) -> usize {
        (self.state_position + 1) % MAX_COMMANDS
    }

    pub(crate) fn state_mut(&mut self) -> &mut State {
        &mut self.states[self.state_position]
    }

    pub fn address(&self) -> u8 {
        if let Some(cmd) = self.command() {
            return cmd.address();
        }
        unreachable!()
    }

    pub fn command(&self) -> Option<&Command> {
        self.commands.peek()
    }

    fn command_mut<'a>(&'a mut self) -> Option<&'a mut Command<'static>> {
        let mut it = self.commands.iter_mut();
        it.nth(0)
    }

    pub fn is_read(&self) -> bool {
        if let Some(cmd) = self.command() {
            return cmd.is_read();
        }
        return false;
    }

    pub fn is_write(&self) -> bool {
        if let Some(cmd) = self.command() {
            return cmd.is_write();
        }
        return false;
    }

    pub fn byte_to_write(&mut self) -> Option<u8> {
        let buf = self.command().unwrap().write_buf();
        let buf_size = buf.len();

        // Empty send command
        if buf_size == 0 {
            return None;
        }

        // No more data to send
        if self.buffer_position >= buf_size {
            return None;
        }

        let out = Some(buf[self.buffer_position]);
        self.next_byte();
        out
    }

    ///  sets byte to read
    pub fn set_byte_to_read(&mut self, new_val: u8) {
        let buf_pos = self.buffer_position;
        let buf = self.command_mut().unwrap().read_buf();
        let buf_size = buf.len();

        // Empty read command
        if buf_size == 0 {
            return;
        }

        // No more data to read
        if buf_pos >= buf_size {
            return;
        }

        buf[buf_pos] = new_val;
        self.next_byte();
    }

    pub fn next_byte(&mut self) {
        self.buffer_position += 1;
    }

    pub fn last_bytes_to_read(&mut self) -> bool {
        let buf_pos = self.buffer_position;
        let buf = self.command_mut().unwrap().read_buf();
        let buf_size = buf.len();

        if buf_pos == buf_size - 1 {
            return true;
        }

        return false;
    }

    pub fn next_command(&mut self) -> bool {
        critical_section::with(|_| {
            self.commands.dequeue();
            self.buffer_position = 0;

            if let Some(c) = self.command() {
                if c.is_noop() {
                    self.commands.dequeue(); // Remove NoOp command
                    if let State::Fail(_) = *self.state_mut() {
                        // Do not change failed state to finished
                    } else {
                        *self.state_mut() = State::Finished
                    }
                    self.state_position = self.get_next_state_position();
                    return false;
                }
                return true;
            }

            return false;
        })
    }

    pub fn have_more_commands(&self) -> bool {
        self.commands.len() > 0
    }

    pub fn skip_transaction(&mut self) -> bool {
        // Skip until NoOp finded
        while self.next_command() {
            // Do nothing
        }

        self.have_more_commands()
    }

    pub fn finished(&self, pos: usize) -> bool {
        let s = self.states[pos];
        match s {
            State::Fail(_) => true,
            State::Finished => true,
            _ => false,
        }
    }
}
