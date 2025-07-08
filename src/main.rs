use std::{
    env,
    fmt::Debug,
    fs::{read_dir, File},
    io::{self, BufRead, BufReader, BufWriter, Seek},
    path::{Path, PathBuf},
    process,
};

use hack_vm::{CodeWriter, CommandType, Parser};

struct FileInfo {
    path: PathBuf,
    file: File,
    name: String,
}
impl FileInfo {
    fn new(path: PathBuf) -> FileInfo {
        let file = File::open(&path).unwrap_or_else(|e| {
            eprintln!("Error while reading input file: {}", e);
            process::exit(1);
        });

        let name = path
            .file_name()
            .map(|x| String::from(x.to_string_lossy()))
            .unwrap_or("Default".to_string());

        FileInfo { path, file, name }
    }

    fn gen_namespace_raw(path: &mut PathBuf) -> String {
        path.set_extension("");
        String::from(
            path.file_name()
                .expect("There should be a final component")
                .to_string_lossy(),
        )
    }

    fn gen_namespace(&self) -> String {
        Self::gen_namespace_raw(&mut self.path.clone())
    }

    fn create_output_file(input_path: PathBuf) -> FileInfo {
        let name = input_path.with_extension("asm");
        let path = PathBuf::from(name.file_name().unwrap_or_else(|| {
            eprintln!("Invalid path");
            process::exit(1);
        }));

        let file = File::create(&path).unwrap_or_else(|e| {
            eprintln!("Error while creating output file: {}", e);
            process::exit(1);
        });

        let name = String::from(name.as_os_str().to_string_lossy());

        FileInfo { path, file, name }
    }
}

fn translate_file<W: BufRead + Seek + Debug>(
    writer: &mut CodeWriter<BufWriter<File>>,
    parser: &mut Parser<W>,
) {
    parser.advance().expect("the parser should be able to advance the first line if everything is functioning as expected");
    while parser.has_more_lines() {
        let command_type = parser.command_type();

        let arg1 = if command_type != CommandType::Return {
            parser.arg1().unwrap_or_else(|| {
                eprintln!(
                    "Error extracting arg1 from line {}\n{:#?}",
                    parser.line_raw, parser
                );
                process::exit(1);
            })
        } else {
            String::new()
        };

        let grab_arg2 = || {
            parser
                .arg2()
                .and_then(|x| x.parse::<i16>().ok())
                .unwrap_or_else(|| {
                    eprintln!(
                        "Error extracting arg 2 from line {}\n{:#?}",
                        parser.line_raw, parser
                    );
                    process::exit(1)
                })
        };

        let output_write_error = |e: io::Error| {
            eprintln!("Error writing to output file: {}", e);
            process::exit(1);
        };

        match &command_type {
            CommandType::Arithmetic(x) => {
                writer
                    .write_arithmetic(x.clone())
                    .unwrap_or_else(output_write_error);
            }
            CommandType::Push | CommandType::Pop => {
                let index = grab_arg2();
                writer
                    .write_push_pop(command_type, arg1, index)
                    .unwrap_or_else(output_write_error);
            }
            CommandType::Label => {
                writer.write_label(arg1).unwrap_or_else(output_write_error);
            }
            CommandType::Goto => {
                writer.write_goto(arg1).unwrap_or_else(output_write_error);
            }
            CommandType::If => {
                writer.write_if(arg1).unwrap_or_else(output_write_error);
            }
            CommandType::Function => {
                let n_vars = grab_arg2();
                writer
                    .write_function(arg1, n_vars)
                    .unwrap_or_else(output_write_error);
            }
            CommandType::Call => {
                let n_vars = grab_arg2();
                writer
                    .write_call(arg1, n_vars)
                    .unwrap_or_else(output_write_error);
            }
            CommandType::Return => {
                writer.write_return().unwrap_or_else(output_write_error);
            }
            _ => {
                println!();
                todo!()
            }
        }

        parser.advance().unwrap_or_else(|_| {
            let namespace = writer.get_namespace();
            println!("Finished {namespace}");
            return;
        });
    }
}

fn main() {
    let mut args = env::args();
    let input_arg = args.nth(1).unwrap_or_else(|| {
        println!("Usage: ./hack-vm [input_file.vm | input_dir/]");
        process::exit(0);
    });
    let input_path = Path::new(&input_arg).to_path_buf();
    let output_file =
        FileInfo::create_output_file(PathBuf::from(input_path.file_name().unwrap_or_default()));
    let mut writer = CodeWriter::new(BufWriter::new(output_file.file));

    writer.write_init().unwrap_or_else(|e| {
        eprintln!("ERROR: {e}");
        process::exit(2);
    });
    let mut parser: Parser<BufReader<&File>>;

    if input_path.is_dir() {
        for entry in read_dir(&input_path)
            .unwrap_or_else(|e| {
                eprintln!("ERROR: {e}");
                process::exit(2);
            })
            .flatten()
        {
            if entry.path().is_dir() || entry.path().extension().is_some_and(|x| x != "vm") {
                continue;
            }

            let file = FileInfo::new(entry.path());
            parser = Parser::new(BufReader::new(&file.file));

            writer.set_namespace(file.gen_namespace());
            println!("Translating new file: {}", &file.name);
            translate_file(&mut writer, &mut parser);
        }
    } else {
        let input_file = FileInfo::new(input_path.clone());
        let namespace = input_file.gen_namespace();
        let input_file = BufReader::new(&input_file.file);

        parser = Parser::new(input_file);
        writer.set_namespace(namespace);

        translate_file(&mut writer, &mut parser);
    }

    writer.write_end().unwrap();
}
