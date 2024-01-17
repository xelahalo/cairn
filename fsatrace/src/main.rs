mod app;
mod command;
mod error;
mod util;

use crate::app::App;
use clap::{crate_version, Arg, Command};
use error::AppError;

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
                //.required(true)
        )
        .arg(
            Arg::new("output")
                .help("Output file to write to")
                .num_args(1)
                // .required(true),
        )
        // .arg(
        //     Arg::new("--")
        //         .num_args(1)
        //         .required(true)
        // )
        .arg(
            Arg::new("cmd")
                .help("Command to run in the build environment.")
                .required(true)
                .allow_hyphen_values(true)
                .num_args(0..)
                .last(true)
        )
        .get_matches();

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

    // let cmd_result = std::process::Command::new("docker")
    //     .args([
    //         "inspect",
    //         "build-env",
    //         "--format",
    //         "\"{{ (index .Mounts 0).Source}}\"",
    //     ])
    //     .output()
    //     .expect("Not able to query containers mount dir.");
    // let mnt_dir = String::from_utf8_lossy(&cmd_result.stdout);
    // let mnt_dir_trimmed = mnt_dir.trim_matches(|c: char| c.is_whitespace() || c == '"');

    // create log file then cd into folder where we run command
    let mnt_dir_trimmed = "/Users/xelahalo/git/personal/cairn/host_mnt";
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
                    .map(|s| s.trim_start_matches(mnt_dir_trimmed))
                    .collect::<Vec<&str>>()
                    .join(" ")
            )
            .as_str(),
        ],
        &output_path,
        &options,
        &mnt_dir_trimmed,
    );

    let mut app = App::new(vec![Box::new(cmd)]);

    app.execute()?;

    Ok(())
}
