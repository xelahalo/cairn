mod app;
mod command;
mod error;
mod util;

use crate::app::App;
use clap::{crate_version, Arg, Command};
use env_logger::Builder;
use error::AppError;
use log::{LevelFilter};
use std::io;
use std::env;

const CHROOT_DIR: &str = "/usr/src/fusemount";
const CONTAINER_NAME: &str = "build-env";

fn main() -> Result<(), AppError> {
    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version(crate_version!())
        .about("Tracing tool for forward build systems.")
        .arg(
            Arg::new("options")
                .help("Characters to filter which operations to dump; a combination of r, w, m ,d, q, t", )
                .num_args(1)
        )
        .arg(
            Arg::new("output")
                .help("Output file to write to")
                .num_args(1)
        )
        .arg(
            Arg::new("cmd")
                .help("Command to run in the build environment.")
                .required(true)
                .allow_hyphen_values(true)
                .num_args(0..)
                .last(true)
        )
        .get_matches();

    Builder::from_default_env()
        .format_timestamp_millis()
        .filter_level(LevelFilter::Info)
        .init();

    // println!(
    //     "Executing command: {:?}",
    //     std::env::args().collect::<Vec<_>>()
    // );

    let mut options = String::new();
    if let Some(opts) = matches.get_one::<String>("options") {
        options.push_str(opts.as_str())
    }

    let mut output_path = String::new();
    if let Some(path) = matches.get_one::<String>("output") {
        output_path.push_str(path.as_str())
    }

    let parsed_cmd: Vec<&str> = match matches.get_many::<String>("cmd") {
        Some(cmd_args) => cmd_args.map(|s| s.as_str()).collect(),
        None => panic!("No command provided"),
    };

    let mnt_dir_var = env::var("CAIRN_MNT_DIR");
    if mnt_dir_var.is_err() {
        return Err(AppError::EnvVarError(io::Error::new(
            io::ErrorKind::NotFound,
            "CAIRN_MNT_DIR not set",
        )));
    }

    let mnt_dir = mnt_dir_var.unwrap();

    let cmd = command::Command::new(
        "docker",
        vec![
            "exec",
            CONTAINER_NAME,
            "/bin/bash",
            "-c",
            format!(
                "./command_wrapper.sh {} {}",
                CHROOT_DIR,
                parsed_cmd
                    .iter()
                    .map(|s| s.trim_start_matches(&mnt_dir))
                    .collect::<Vec<&str>>()
                    .join(" ")
            )
            .as_str(),
        ],
        &output_path,
        &options,
        &mnt_dir,
    );

    // println!("Executing command: {:?}", cmd);

    let mut app = App::new(vec![Box::new(cmd)]);

    app.execute()?;

    Ok(())
}
