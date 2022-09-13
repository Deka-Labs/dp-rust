use core::{
    cmp::min,
    fmt::{write, Arguments, Error, Write},
};

pub fn format_string<'s>(buffer: &'s mut [u8], args: Arguments) -> Result<&'s str, Error> {
    let mut w = FormatBuffer::new(buffer);
    write(&mut w, args)?;
    w.as_str().ok_or(Error)
}

struct FormatBuffer<'s> {
    buffer: &'s mut [u8],
    used: usize,
}

impl<'s> FormatBuffer<'s> {
    fn new(buffer: &'s mut [u8]) -> Self {
        Self { buffer, used: 0 }
    }

    fn as_str(self) -> Option<&'s str> {
        if self.used <= self.buffer.len() {
            use core::str::from_utf8_unchecked;
            Some(unsafe { from_utf8_unchecked(&self.buffer[..self.used]) })
        } else {
            None
        }
    }
}

impl<'s> Write for FormatBuffer<'s> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        if self.used > self.buffer.len() {
            return Err(Error);
        }
        let remaining_buf = &mut self.buffer[self.used..];
        let raw_s = s.as_bytes();
        let write_num = min(raw_s.len(), remaining_buf.len());
        remaining_buf[..write_num].copy_from_slice(&raw_s[..write_num]);
        self.used += raw_s.len();
        if write_num < raw_s.len() {
            Err(Error)
        } else {
            Ok(())
        }
    }
}
