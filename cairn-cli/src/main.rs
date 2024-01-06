mod app;
mod command;
mod error;
mod util;

use crate::app::App;
use clap::{crate_version, Arg, Command};
use error::AppError;

const CHROOT_DIR: &str = "/usr/src/fusemount";
const WORK_DIR: &str = "workdir";
const CONTAINER_NAME: &str = "build-env";
const LOG_DIR: &str = "/usr/src/app/tmp";

fn main() -> Result<(), AppError> {
    let matches = Command::new("Cairn")
        .author("xelahalo <xelahalo@gmail.com>")
        .version(crate_version!())
        .about("Tracing tool for forward build systems.")
        .arg(
            Arg::new("cmd")
                .help("Command to run in the build environment. Must be quoted.")
                .num_args(1)
                .required(true),
        )
        .get_matches();

    let parsed_cmd: &str = match matches.get_one::<String>("cmd") {
        Some(cmd) => cmd,
        None => panic!("No command provided"),
    }
    .trim_matches(|c| c == '\'' || c == '"');

    // create log file then cd into folder where we run command
    let cmd = command::Command::new(
        "docker",
        vec![
            "exec",
            CONTAINER_NAME,
            "/bin/bash",
            "-c",
            format!(
                "mkdir -p {} && touch {} && chroot {} /bin/bash -c 'cd {} && {}'",
                LOG_DIR, log_name, CHROOT_DIR, WORK_DIR, parsed_cmd
            )
            .as_str(),
        ],
    );

    let app = App::new(vec![&cmd]);

    app.execute()?;

    Ok(())
}
