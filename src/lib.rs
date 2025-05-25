use std::io::{BufRead, Error, Seek, SeekFrom};

enum CommandType {
    Arithmetic,
    Push,
    Pop,
    Label,
    Goto,
    If,
    Function,
    Return,
    Call
}

#[repr(u16)]
enum VRegister {
    StackPointer = 0,
    Local = 1,
    Arg = 2,
    This = 3,
    That = 4,
    Temp = 5,
    R13 = 13,
    R14 = 14,
    R15 = 15
}

fn get_vpointer(this:bool) -> u16 {
    if this {
        return VRegister::This as u16;
    }
    VRegister::That as u16
}

fn get_vregister_address(segment: VRegister, offset: u16) -> Result<u16,String> {
    match segment {
        VRegister::Temp => {
            if offset < 8 {
                return Ok(segment as u16 + offset);
            }
            return Err("Offset over 8 while using Temp".into())
        }
        _ => {
            return Ok(segment as u16);
        }
    }
}


struct Parser<W: Seek + BufRead> {
    input: W,
    has_lines_remaining: bool,
    cur_line: Option<String>,
    /// line number not including empty lines or comments
    line: usize,
    /// actual line number
    line_raw: usize
}

impl<W: Seek + BufRead> Parser<W> {
    pub fn new(input: W) -> Parser<W> {
        Parser { input, has_lines_remaining: false, cur_line: None, line: 0, line_raw: 0 }
    }

    pub fn has_more_lines(&self) -> bool {
        self.has_lines_remaining
    }

    pub fn advance(&mut self) -> Result<(), String> {
        let mut next_string = String::new();
        loop {
            let bytes_read = self.input.read_line(&mut next_string);
            if bytes_read.as_ref().is_ok_and(|x| x < &1){
                self.has_lines_remaining = false;
                self.cur_line = None;

                return Err("EOF".to_string())
            }
            else if bytes_read.as_ref().is_err() {
                self.has_lines_remaining = false;
                self.cur_line = None;

                return Err(bytes_read.unwrap_err().to_string());
            }
            else {
                next_string = next_string.trim().to_string();

                let comment = next_string.find("//");
                if comment.is_some() {
                    let loc = comment.unwrap();
                    next_string.replace_range(loc.., "");
                }

                if next_string.is_empty() {
                    self.line_raw += 1;
                    continue;
                }

                self.has_lines_remaining = true;
                self.cur_line = Some(next_string);
                self.line += 1;
                
                return Ok(())
            }
        }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        self.input.seek(SeekFrom::Start(0))?;

        Ok(())
    }
}