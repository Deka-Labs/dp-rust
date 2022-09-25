use core::sync::atomic::AtomicBool;

use super::states::State;

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
    pub(crate) commands: [Option<Command<'static>>; MAX_COMMANDS],

    pub(crate) command_position: usize,
    pub(crate) buffer_position: usize,
    pub(crate) state: State,

    pub(crate) future_readed: AtomicBool,
}

impl<const MAX_COMMANDS: usize> Transaction<MAX_COMMANDS> {
    pub fn new<const INPUT_CMD_SIZE: usize>(
        mut commands: [Command<'static>; INPUT_CMD_SIZE],
    ) -> Self {
        // We cannot fill more commands than buffer capacity
        assert!(INPUT_CMD_SIZE <= MAX_COMMANDS);
        // Empty transactions is not allowed
        assert!(INPUT_CMD_SIZE > 0);

        let mut commands_option = [(); MAX_COMMANDS].map(|_| None);
        for i in 0..INPUT_CMD_SIZE {
            commands_option[i] = Some(core::mem::take(&mut commands[i]))
        }

        Self {
            commands: commands_option,
            command_position: 0,
            buffer_position: 0,
            state: State::Begin,

            future_readed: AtomicBool::new(false),
        }
    }

    pub fn address(&self) -> u8 {
        if let Some(cmd) = &self.commands[self.command_position] {
            return cmd.address();
        }
        unreachable!()
    }

    pub fn command(&self) -> Option<&Command> {
        if self.command_position < self.commands.len() {
            return self.commands[self.command_position].as_ref();
        }

        None
    }

    fn command_mut<'a>(&'a mut self) -> Option<&'a mut Command<'static>> {
        if self.command_position < self.commands.len() {
            return self.commands[self.command_position].as_mut();
        }

        None
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
        self.command_position += 1;
        self.buffer_position = 0;
        self.state = State::Begin;
        self.command().is_some()
    }

    pub fn finished(&self) -> bool {
        if let State::Fail(_) = self.state {
            return true;
        }

        if let State::Finished = self.state {
            return true;
        }

        return false;
    }

    pub fn future_read(&self) -> bool {
        self.future_readed
            .load(core::sync::atomic::Ordering::Relaxed)
    }
}
