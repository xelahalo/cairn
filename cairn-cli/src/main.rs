mod app;
mod command;
mod error;
mod util;

use crate::app::App;
use clap::{crate_version, Arg, Command};
use dotenv::dotenv;
use error::AppError;

const CHROOT_DIR: &str = "/usr/src/fusemount";
const CONTAINER_NAME: &str = "build-env";

fn main() -> Result<(), AppError> {
    dotenv().ok();

    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version(crate_version!())
        .about("Tracing tool for forward build systems.")
        // .arg(Arg::new("options").long("options").help(
        //     "Characters to filter which operations to dump; a combination of r, w, m ,d, q, t",
        // ).num_args(1).required(true))
        // .arg(
        //     Arg::new("output")
        //         .help("Output file to write to")
        //         .long("output")
        //         .num_args(1)
        //         .required(true),
        // )
        .arg(
            Arg::new("cmd")
                .help("Command to run in the build environment. Must be quoted.")
                .num_args(1)
                .required(true)
                .allow_hyphen_values(true),
        )
        .get_matches();

    // let mut options = String::new();
    // if let Some(opts) = matches.get_one::<String>("options") {
    //     options.push_str(opts.as_str())
    // }

    // let mut output_path = String::new();
    // if let Some(path) = matches.get_one::<String>("output") {
    //     output_path.push_str(path.as_str())
    // }

    let parsed_cmd: &str = match matches.get_one::<String>("cmd") {
        Some(cmd) => cmd,
        None => panic!("No command provided"),
    }
    .trim_matches(|c| c == '\'' || c == '"');

    let workdir = std::env::var("WORKDIR").expect("ERROR: WORKDIR not set");

    // create log file then cd into folder where we run command
    let cmd = command::Command::new(
        "docker",
        vec![
            "exec",
            CONTAINER_NAME,
            "/bin/bash",
            "-c",
            format!(
                "./command_wrapper.sh {} {} {}",
                CHROOT_DIR, workdir, parsed_cmd
            )
            .as_str(),
        ],
        "cairn.log",
    );

    let mut app = App::new(vec![Box::new(cmd)]);

    app.execute()?;

    Ok(())
}
