use std::{env, ffi::OsStr, fs::{read, File}, io::{self, Cursor}, path::Path, process};

use hack_vm::{CodeWriter, CommandType, Parser};

fn main() {
    let mut args = env::args();
    let input_arg = args.nth(1).unwrap_or_else(|| {
        eprintln!("Usage: ./hack-vm [input_file.vm]");
        process::exit(0);
    });

    let input_path = Path::new(&input_arg);

    let input_file = read(input_path).unwrap_or_else( |e| {
        eprintln!("Error while reading input file: {}", e);
        process::exit(1);
    });

    let input_file = Cursor::new(input_file);

    let output_file_name = input_path.with_extension("asm");
    let output_file_path = output_file_name.file_name().unwrap_or_else(|| {
        eprintln!("Invalid path");
        process::exit(1);
    });

    let output_file = File::create(output_file_path).unwrap_or_else(|e| {
        eprintln!("Error while creating output file: {}", e);
        process::exit(1);
    });

    // fuckass function, theres gotta be a better way to do this
    let namespace = input_path.file_name().and_then(|x| Some(String::from(x.to_string_lossy()))).unwrap_or("Default".to_string());
    let namespace = namespace.strip_suffix(".vm").unwrap_or(&namespace).to_string();
    println!("{}", namespace);

    let mut parser = Parser::new(input_file);
    let mut writer = CodeWriter::new(output_file, namespace);

    parser.advance().expect("fuck you");
    while parser.has_more_lines() {
        let command_type = parser.command_type();
        
        let arg1;
        if command_type != CommandType::Return {
            arg1 = parser.arg1().unwrap_or_else(|| {
                eprintln!("Error extracting arg1 from line {}\n{:#?}", parser.line_raw, parser);
                process::exit(1);
            });
        }
        else {
            arg1 = String::new();
        }

        let grab_arg2 = || {
            parser.arg2()
            .and_then(|x| x.parse::<i16>().ok())
            .unwrap_or_else( || {
                eprintln!("Error extracting arg 2 from line {}\n{:#?}", parser.line_raw, parser);
                process::exit(1)
            })
        };

        let output_write_error = |e: io::Error| {
            eprintln!("Error writing to output file: {}", e);
            process::exit(1);
        };
        
        match &command_type {
            CommandType::Arithmetic(x) => {
                writer.write_arithmetic(x.clone()).unwrap_or_else(output_write_error);
            }
            CommandType::Push | CommandType::Pop => {
                let index = grab_arg2();
                writer.write_push_pop(command_type, arg1, index).unwrap_or_else(output_write_error);
            }
            _ => {
                todo!()
            }
        }

        parser.advance().unwrap_or_else(|_| {
            println!("Finished");
            writer.write_end().unwrap();
        });
    }
}
