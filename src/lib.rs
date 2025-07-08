use std::io::{self, BufRead, Error, Seek, SeekFrom, Write};

#[derive(PartialEq, Eq)]
pub enum CommandType {
    Arithmetic(String),
    Push,
    Pop,
    Label,
    Goto,
    If,
    Function,
    Return,
    Call,
    Empty,
}

#[derive(Debug)]
pub struct Parser<W: Seek + BufRead> {
    input: W,
    has_lines_remaining: bool,
    cur_line: Option<String>,
    /// line number not including empty lines or comments
    pub line: usize,
    /// actual line number
    pub line_raw: usize,
}

/// Defines the VM label type for translating into assembly labels
/// The contained string defines the `namespace` the label exists in (ie. (VmFunction.LabelName))
#[derive(Debug)]
pub enum LabelType {
    Static,
    FunctionLabel,
    FunctionCall,
    FunctionRet,
}

impl<W: Seek + BufRead> Parser<W> {
    pub fn new(input: W) -> Parser<W> {
        Parser {
            input,
            has_lines_remaining: false,
            cur_line: None,
            line: 0,
            line_raw: 0,
        }
    }

    pub fn has_more_lines(&self) -> bool {
        self.has_lines_remaining
    }

    pub fn advance(&mut self) -> io::Result<()> {
        let mut next_string = String::new();
        loop {
            let bytes_read = self.input.read_line(&mut next_string)?;
            if bytes_read < 1 {
                self.has_lines_remaining = false;
                self.cur_line = None;

                return Err(Error::from(io::ErrorKind::UnexpectedEof));
            } else {
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

                return Ok(());
            }
        }
    }

    pub fn reset(&mut self) -> io::Result<()> {
        self.input.seek(SeekFrom::Start(0))?;
        self.line = 0;
        self.line_raw = 0;
        self.cur_line = None;

        Ok(())
    }

    pub fn set_file(&mut self, file: W) -> io::Result<()> {
        self.input = file;
        self.reset()?;

        Ok(())
    }

    fn match_arithmetic(command: String) -> Option<CommandType> {
        return match command.as_str() {
            "add" | "sub" | "neg" | "and" | "not" | "eq" | "gt" | "lt" | "or" => {
                Some(CommandType::Arithmetic(command))
            }

            _ => None,
        };
    }

    fn split_command(&self) -> Vec<&str> {
        let input = self
            .cur_line
            .as_ref()
            .expect("Split command should not be called with an empty line");

        input.split_whitespace().collect()
    }

    pub fn command_type(&self) -> CommandType {
        let split_line = self.split_command();
        let command = split_line.first().expect("index zero should exist");

        let result = Parser::<W>::match_arithmetic(command.to_string());
        if let Some(item) = result {
            return item;
        };

        match *command {
            "push" => CommandType::Push,
            "pop" => CommandType::Pop,
            "label" => CommandType::Label,
            "goto" => CommandType::Goto,
            "if-goto" => CommandType::If,
            "call" => CommandType::Call,
            "function" => CommandType::Function,
            "return" => CommandType::Return,
            _ => todo!(),
        }
    }

    pub fn arg1(&self) -> Option<String> {
        let index = match self.command_type() {
            CommandType::Arithmetic(_) => 0,
            _ => 1,
        };

        self.split_command().get(index).map(|x| x.to_string())
    }

    pub fn arg2(&self) -> Option<String> {
        self.split_command().get(2).map(|x| x.to_string())
    }
}

#[derive(Debug)]
pub struct CodeWriter<W: Write + Seek> {
    out_stream: W,
    namespace: String,
    cur_func: String,
    call_count: usize,
}

impl<W: Write + Seek> CodeWriter<W> {
    pub fn new(out_stream: W) -> CodeWriter<W> {
        CodeWriter {
            out_stream,
            namespace: String::new(),
            cur_func: String::new(),
            call_count: 0,
        }
    }

    pub fn set_namespace(&mut self, new_namespace: String) {
        self.namespace = new_namespace;
    }

    pub fn get_namespace(&self) -> &String {
        &self.namespace
    }

    fn map_vreg(register: &String) -> String {
        match register.as_str() {
            "local" => "LCL".to_string(),
            "argument" => "ARG".to_string(),
            "this" => "THIS".to_string(),
            "that" => "THAT".to_string(),
            _ => register.to_owned(),
        }
    }

    fn decrement_sp() -> String {
        "@SP\n AM=M-1\n".to_string()
    }

    fn pop_d() -> String {
        Self::decrement_sp() + " D=M // pop D\n"
    }

    /// does not use the D register
    fn pop_a() -> String {
        Self::decrement_sp() + " A=M // pop A\n"
    }

    fn push_d() -> String {
        "@SP\n A=M\n M=D\n @SP\n M=M+1 // push D\n".to_string()
    }

    /// the address to M must be loaded in A first
    #[allow(dead_code)]
    fn push_m() -> String {
        "D=M\n ".to_owned() + &Self::push_d()
    }

    /// loads val into D
    fn load_const(val: i16) -> String {
        format!("@{val}\n D=A\n")
    }

    /// pushes val onto the stack
    fn push_const(val: i16) -> String {
        Self::load_const(val) + &Self::push_d()
    }
    /// gets the value of M value of label_name and pushes onto the stack
    fn push_label(label_name: &str) -> String {
        format!("@{label_name}\n") + &Self::push_m()
    }

    /// sets the A register to the location that THIS or THAT points to
    fn load_pointer_segment(index: i16) -> String {
        let segment = if index == 0 { "THIS" } else { "THAT" };

        format!("@{segment}\n")
    }
    /// returns an assembly label formatted for use in the VM
    fn get_label(&mut self, label_type: LabelType, label_name: Option<&String>) -> String {
        let label_name = if let Some(label) = label_name {
            label
        } else {
            &String::new()
        };
        let namespace = &self.namespace;
        let function_name = &self.cur_func;

        match label_type {
            LabelType::Static => format!("{namespace}.{label_name}"),
            LabelType::FunctionCall => format!("{namespace}.{function_name}"),
            LabelType::FunctionRet => {
                let call_count = self.call_count;
                self.call_count += 1;
                format!("{namespace}.{function_name}$ret.{call_count}")
            }
            LabelType::FunctionLabel => {
                format!("{namespace}.{function_name}${label_name}")
            }
        }
    }

    /// sets target_reg to the base address of segment + index
    fn load_vreg_address(segment: &String, index: i16, target_reg: char) -> String {
        let segment = Self::map_vreg(segment);
        format!("@{index}\n D=A\n @{segment}\n A=M\n {target_reg}=D+A\n")
    }
    /// calculates the label for the static value at index and loads it into A
    fn load_static_address(&mut self, index: i16) -> String {
        let static_var = self.get_label(LabelType::Static, Some(&index.to_string()));

        format!("@{static_var}\n")
    }

    /// writes a push or pop VM command to out_stream
    pub fn write_push_pop(
        &mut self,
        command: CommandType,
        segment: String,
        index: i16,
    ) -> io::Result<()> {
        let push_comment = format!("// push {segment} {index}\n\n");
        let pop_comment = format!("// pop {segment} {index}\n\n");

        let result = match command {
            CommandType::Push if &segment == "pointer" => {
                Self::load_pointer_segment(index) + "D=M\n " + &Self::push_d() + &push_comment
            }
            CommandType::Push if &segment == "static" => {
                self.load_static_address(index) + "D=M\n" + &Self::push_d() + &push_comment
            }
            CommandType::Push if &segment == "constant" => Self::push_const(index) + &push_comment,
            CommandType::Push if &segment == "temp" => {
                let mut error_comment = "";
                if index > 7 {
                    error_comment = "// Warning: access to segment 'temp' above index 7 will cause overflow related errors\n";
                    eprint!("{}", error_comment);
                }
                Self::load_const(index)
                    + "@5\n A=D+A\n D=M\n"
                    + &Self::push_d()
                    + &push_comment
                    + error_comment
            }
            CommandType::Push => {
                Self::load_vreg_address(&segment, index, 'A')
                    + "D=M\n "
                    + &Self::push_d()
                    + &push_comment
            }
            CommandType::Pop if &segment == "pointer" => {
                Self::pop_d() + &Self::load_pointer_segment(index) + "M=D\n" + &pop_comment
            }
            CommandType::Pop if &segment == "static" => {
                Self::pop_d() + &self.load_static_address(index) + "M=D\n" + &pop_comment
            }
            CommandType::Pop if &segment == "constant" => {
                Self::pop_d() + &format!("@{index}\n M=D\n") + &pop_comment
            }
            CommandType::Pop if &segment == "temp" => {
                let mut error_comment = "";
                if index > 7 {
                    error_comment = "// Warning: access to segment 'temp' above index 7 will cause overflow related errors\n";
                    eprint!("{}", error_comment);
                }

                Self::load_const(index)
                    + "@5\n D=D+A\n @R13\n M=D\n"
                    + &Self::pop_d()
                    + "@R13\n A=M\n M=D\n"
                    + &pop_comment
                    + error_comment
            }
            CommandType::Pop => {
                Self::load_vreg_address(&segment, index, 'D')
                    + "@R13\n M=D\n"
                    + &Self::pop_d()
                    + "@R13\n A=M\n M=D\n"
                    + &pop_comment
            }
            _ => return Ok(()),
        };

        self.out_stream.write_all(result.as_bytes())?;
        Ok(())
    }
    /// pops the bottom two values of the stack and performs the given operation on them, pushing
    /// the result back onto the stack
    fn do_stack_op_two(op: String) -> String {
        Self::pop_d() + &Self::pop_a() + &op + "\n" + &Self::push_d()
    }
    /// pops the bottom value of the stack and performs the given operation on it, pushing the
    /// result back onto the stack
    fn do_stack_op_one(op: String) -> String {
        Self::pop_d() + &op + "\n" + &Self::push_d()
    }
    /// compares the bottom two values on the stack using the assembly jump_op given, pushing
    /// true(1) if the jump_op condition is met or false(0) otherwise
    fn do_compare_stack_two(&mut self, jump_op: String) -> String {
        let current_pos = self
            .out_stream
            .stream_position()
            .expect("Getting the position should work ath this stage");
        Self::do_stack_op_two(
            format!(
                "D=D-A\n @IF{if_label}\n D;{jump_op}\n D=0\n @ENDIF{endif_label}\n 0;JMP\n (IF{if_label})\n D=-1\n (ENDIF{endif_label})\n",
                if_label = current_pos,
                endif_label = current_pos + 1
            )
        ) + "// if then\n"
    }

    /// writes the provided VM arithmetic command to the out_stream
    pub fn write_arithmetic(&mut self, command: String) -> io::Result<()> {
        let result = match command.as_str() {
            "add" => Self::do_stack_op_two("D=D+A".to_string()),
            "sub" => Self::do_stack_op_two("D=A-D".to_string()),
            "neg" => Self::do_stack_op_one("D=-D".to_string()),
            "eq" => self.do_compare_stack_two("JEQ".to_string()),
            "gt" => self.do_compare_stack_two("JLT".to_string()),
            "lt" => self.do_compare_stack_two("JGT".to_string()),
            "and" => Self::do_stack_op_two("D=D&A".to_string()),
            "or" => Self::do_stack_op_two("D=D|A".to_string()),
            "not" => Self::do_stack_op_one("D=!D".to_string()),
            _ => panic!("Unexpected arithmetic command encountered: {}", command),
        };

        self.out_stream.write_all(result.as_bytes())?;
        Ok(())
    }

    /// writes the `label` VM command to the out_stream
    pub fn write_label(&mut self, label_name: String) -> io::Result<()> {
        let comment = format!("// label {label_name}\n");
        let label = self.get_label(LabelType::FunctionLabel, Some(&label_name));
        self.out_stream
            .write_all(format!("({label})\n{comment}").as_bytes())
    }
    /// writes the `goto` VM command to the out_stream
    pub fn write_goto(&mut self, label_name: String) -> io::Result<()> {
        let comment = format!("// goto {label_name}\n");
        let label = self.get_label(LabelType::FunctionLabel, Some(&label_name));

        let output = format!("@{label}\n 0;JMP\n{comment}");
        self.out_stream.write_all(output.as_bytes())
    }
    /// writes the `if-goto` VM command to the out_stream
    pub fn write_if(&mut self, label_name: String) -> io::Result<()> {
        let comment = format!("// if-goto {label_name}\n");
        let label = self.get_label(LabelType::FunctionLabel, Some(&label_name));

        let output = Self::pop_d() + &format!("@{label}\n D;JNE\n") + &comment;

        self.out_stream.write_all(output.as_bytes())
    }

    /// set reg to temp_var i
    fn get_temp_var(i: usize, reg: &str) -> String {
        let i_str = i.to_string();
        format!("@R{i_str}\n{reg}=M\n") // get temp_var i and set reg to that value
    }

    /// store D in temp_var i
    fn store_temp_var(i: usize) -> String {
        let i_str = i.to_string();
        format!("@R{i_str}\nM=D\n")
    }

    /// writes the `return` VM command to the out_stream
    pub fn write_return(&mut self) -> io::Result<()> {
        let comment = "// return\n";
        let result = "@LCL\nD=M\n".to_owned()
            + &Self::store_temp_var(13) // R13 is frame
            + "@5\nD=D-A\n" // D = frame-5
            + &Self::store_temp_var(14) // R14 is ret_address

            + &Self::pop_d() // get the return value
            + "@ARG\nA=M\nM=D\n" // set head of callee stack to be the return value
            + "D=A\n@SP\nM=D+1\n" // set SP to ARG + 1 (new head containing the return value)

            + &Self::get_temp_var(13, "D")
            + "A=D-1\nD=M\n@THAT\nM=D\n" // restore THAT

            + &Self::get_temp_var(13, "D")
            + "@2\nA=D-A\nD=M\n@THIS\nM=D\n" // restore THIS

            + &Self::get_temp_var(13, "D")
            + "@3\nA=D-A\nD=M\n@ARG\nM=D\n" // restore ARG

            + &Self::get_temp_var(13, "D")
            + "@4\nA=D-A\nD=M\n@LCL\nM=D\n" // restore LCL

            + &Self::get_temp_var(14, "A")
            + "0;JMP\n" // jump to ret_address
            + comment;

        self.out_stream.write_all(result.as_bytes())
    }

    /// writes the `call` VM command to the out_stream
    pub fn write_call(&mut self, function_name: String, n_vars: i16) -> io::Result<()> {
        let ret_address = self.get_label(LabelType::FunctionRet, Some(&function_name));
        let n_vars_str = n_vars.to_string();
        let comment = format!("// call {function_name} {n_vars_str}\n");
        let result = format!("@{ret_address}\nD=A\n") + &Self::push_d()
            + &Self::push_label("LCL")
            + &Self::push_label("ARG")
            + &Self::push_label("THIS")
            + &Self::push_label("THAT")
            + "@SP\nD=M\n@5\nD=D-A\n" // D = SP-5 (SP before the previous stack frame was pushed)
            + &format!("@{n_vars_str}\nD=D-A\n") // D = SP-n_vars (SP before the args for this function got added)
            + "@ARG\nM=D\n" // ARG = D (args can now be gotten by 'pop argument i')
            + "@SP\nD=M\n@LCL\nM=D\n" // LCL = SP
            + &format!("@{function_name}\n0;JMP\n") // goto function
            + &format!("({ret_address})\n") // sets the ret_address label
            + &comment;

        self.out_stream.write_all(result.as_bytes())
    }

    pub fn write_function(&mut self, function_name: String, n_vars: i16) -> io::Result<()> {
        let mut result = format!("({function_name})\n");
        let n_vars_str = n_vars.to_string();
        let comment = format!("// function {function_name} {n_vars_str}\n");

        let mut i = 0;
        while i < n_vars {
            result.push_str(Self::push_const(0).as_str());
            i += 1;
        }
        result.push_str(comment.as_str());

        self.out_stream.write_all(result.as_bytes())
    }

    /// setup assembly for setting the stack pointer and jumps to the `Sys.init`
    pub fn write_init(&mut self) -> io::Result<()> {
        self.out_stream
            .write_all("@256\nD=A\n@SP\nM=D\n@Sys.init\n0;JMP\n".as_bytes())
    }

    /// writes a neverending loop to the out_stream
    pub fn write_end(&mut self) -> io::Result<()> {
        self.out_stream
            .write_all("(VMEND)\n@VMEND\n0;JMP\n".as_bytes())
    }
}
