use std::{io::{BufRead, Error, Seek, SeekFrom, Write}, result};

enum CommandType {
    Arithmetic(String),
    Push,
    Pop,
    Label,
    Goto,
    If,
    Function,
    Return,
    Call,
    Empty
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

    fn match_arithmetic(command: String) -> Option<CommandType> {
        return match command.as_str() {
            "add" | 
            "sub" |
            "neg" |
            "and" |
            "not" |
            "eq" |
            "gt" |
            "lt" |
            "or" => Some(CommandType::Arithmetic(command)),

            _ => None
        }
    }

    fn split_command(&self) -> Vec<&str> {
        let input = self.cur_line.as_ref().expect("Split command should not be called with an empty line");

        input.split_whitespace().collect()
    }

    pub fn command_type(&self) -> CommandType {
        let split_line = self.split_command();
        let command = split_line.get(0).expect("index zero should exist");
        
        let result = Parser::<W>::match_arithmetic(command.to_string());
        if result.is_some() {
            return result.expect("uhh");
        }

        match command {
            &"push" => return CommandType::Push,
            &"pop" => return CommandType::Pop,
            _ => todo!()
        }

        CommandType::Empty
    }

    pub fn arg1(&self) -> Option<String> {
        let index = match self.command_type() {
            CommandType::Arithmetic(_) => 0,
            _ => 1
        };

        self.split_command().get(index).and_then(|x| Some(x.to_string()))
    }

    pub fn arg2(&self) -> Option<String> {
        self.split_command().get(2).and_then(|x| Some(x.to_string()))
    }
}

struct CodeWriter<W: Write> {
    out_stream: W,
}

impl<W: Write> CodeWriter<W> {
    
    pub fn new(out_stream: W) -> CodeWriter<W> {
        CodeWriter { out_stream }
    }

    fn map_vreg(register: String) -> String {
        match register.as_str() {
            "local" => "LCL".to_string(),
            "argument" => "ARG".to_string(),
            "this" => "THIS".to_string(),
            "that" => "THAT".to_string(),
            "temp" => "TEMP".to_string(),
            _ => register
        }
    }

    fn write_decrement_sp() -> String {
        "@SP\n AM=M-1\n".to_string()
    }

    fn write_pop_d() -> String {
        let mut result = String::new();
        result.push_str(Self::write_decrement_sp().as_str());
        result.push_str(" D=M // pop D\n");

        result
    }

    fn write_pop_a() -> String {
        let mut result = String::new();
        result.push_str(Self::write_decrement_sp().as_str());
        result.push_str(" A=M // pop A\n");

        result
    }

    fn write_push_d() -> String {
        "@SP\n A=M\n M=D\n @SP\n M=M+1 // push D\n".to_string()
    }

    fn load_const(val: i16) -> String {
        format!("@{val}\n D=A\n")
    }

    fn write_push_const(val: i16) -> String {
        let mut result = String::new();
        result.push_str(Self::load_const(val).as_str());
        result.push_str(Self::write_push_d().as_str());

        result
    }

    /// sets the A register to the location that THIS or THAT points to
    fn load_pointer_segment(index: i16) -> String {
        let segment;
        if index == 0 {
            segment = "THIS"
        } else {
            segment = "THAT"
        }

        format!("@{segment}\n A=M\n")
    }

    /// sets the A register to the base address of segment + index
    fn load_vreg_address(segment: String, index: i16) -> String {
        let segment = Self::map_vreg(segment);
        format!("@{index}\n D=A\n @{segment}\n A=D+A\n")
    }

    /// return an assembly var name that equates to the given namespace and index
    fn static_var(namespace: String, index: i16) -> String {
        let namespace = namespace.to_lowercase();
        todo!()
    }

    fn load_static_address(namespace: String, index: i16) -> String {
        let static_var = Self::static_var(namespace, index);
        
        format!("@{static_var}")
    }

    pub fn write_push_pop(command: CommandType, segment: String, index: i16) -> String {
        let result = String::new();

        match command {
            CommandType::Push => {
                todo!()
            },
            CommandType::Pop => {
                todo!()
            },
            _ => return String::new()
        }
    }

    pub fn write_arithmetic(&mut self, command: String) -> Result<(), String> {
        todo!()
    }
}