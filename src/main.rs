mod app;
mod command;
mod error;
mod util;

use crate::app::App;
use clap::{Arg, Command};
use error::AppError;

const MNT_DIR: &str = "/mnt";
const WORKDIR: &str = "/usr/src/fusemount";
const CONTAINER_NAME: &str = "build-env";

fn main() -> Result<(), AppError> {
    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version("0.1.0")
        .about("Tracing tool for forward build systems.")
        .arg(
            Arg::new("container")
                .help("Specify the container to use. If not specified will start the container process and tear it down after use.")
                .num_args(1)
                .short('c')
                .long("container")
                .index(1),
        )
        .arg(
            Arg::new("cmd")
                .help("Command to run in the build environment")
                .num_args(1)
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("args")
                .help("Arguments to pass to the command")
                .num_args(0..)
                .index(3),
        )
        .get_matches();

    let container_given: bool; 
    let container: &str = match matches.get_one::<String>("container") {
        Some(container) => {
            container_given = true;
            container
        },
        None => {
            container_given = false;
            CONTAINER_NAME
        },
    };
    let parsed_cmd: &str = match matches.get_one::<String>("cmd") {
        Some(cmd) => cmd,
        None => panic!("No command provided"),
    };
    let parsed_args: Vec<&str> = match matches.get_many::<String>("args") {
        Some(args) => args.map(|s| s.as_str()).collect(),
        None => vec![],
    }; 

    let init_cmd = command::Command::new("bash", vec!["init.sh", MNT_DIR]);
    let build_cmd = command::Command::new("docker", vec!["exec", "-it", container, "chroot", WORKDIR, &parsed_cmd, &parsed_args.join(" ")]);
    let teardown_cmd = command::Command::new("bash", vec!["teardown.sh"]);

    let app = if container_given {
        App::new(vec![&build_cmd])
    } else {
        App::new(vec![&init_cmd, &build_cmd, &teardown_cmd])
    };

    app.execute()?;

    Ok(())
}
