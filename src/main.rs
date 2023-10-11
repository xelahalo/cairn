use clap::Parser;
use std::process::{Command, Stdio};
use std::path::Path;
use std::io::{BufReader, BufRead};

fn main() {
    // env::set_var("RUST_BACKTRACE", "full");
    let args = Cli::parse();

    exec_stream("/bin/bash", vec!("cairn.sh", args.cmd.as_str()))
}

// Execute command and stream its output to stdout
pub fn exec_stream<P: AsRef<Path>>(binary: P, args: Vec<&str> ) {
    let mut cmd = Command::new(binary.as_ref())
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdout = cmd.stdout.as_mut().unwrap();
        let stdout_reader = BufReader::new(stdout);
        let stdout_lines = stdout_reader.lines();

        for line in stdout_lines {
            println!("Read: {:?}", line);
        }
    }

    cmd.wait().unwrap();
}

#[derive(Parser)]
struct Cli {
    cmd: String,
}


