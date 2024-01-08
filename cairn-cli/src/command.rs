use crate::error::AppError;
use crate::util::stream_output;
use regex::Regex;
use std::collections::{HashSet, VecDeque};
use std::fs::File;
use std::io::Write;
use std::io::{BufRead, BufReader};

pub trait MutCommand {
    fn execute(&mut self) -> Result<(), AppError>;
}

#[derive(Debug)]
pub struct Command {
    executable: String,
    args: Vec<String>,
    output_path: String,
    root_ppid: Option<u32>,
    start_time: u32,
}

#[derive(Clone)]
struct LogEntry {
    timestamp: u32,
    pid: u32,
    ppid: u32,
    op: char,
    path: String,
    order: u32,
}

impl Command {
    pub fn new(executable: &str, args: Vec<&str>, output_path: &str) -> Self {
        Self {
            executable: executable.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            output_path: output_path.to_string(),
            root_ppid: None,
            start_time: 0,
        }
    }

    fn process_log(&self) -> Result<(), AppError> {
        let mnt_dir = std::env::var("MNT_DIR").expect("ERROR: MNT_DIR not set");
        let log_file =
            File::open(format!("{}/tracer.log", mnt_dir)).expect("ERROR: Could not open log file");

        let res = self.parse_lines(
            BufReader::new(log_file)
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>(),
        );

        let root_ppid = self.root_ppid.unwrap();
        let mut ppids = HashSet::new();
        ppids.insert(root_ppid);

        let mut filtered_results: Vec<LogEntry> = Vec::new();
        let mut queue = VecDeque::from(res);
        let mut queue_size = queue.len();

        while queue_size == queue.len() {
            queue_size = queue.len();
            let mut backup_queue = VecDeque::new();

            while let Some(result) = queue.pop_front() {
                if result.timestamp < self.start_time {
                    continue;
                }

                if result.pid == root_ppid {
                    filtered_results.push(result.clone());
                } else if ppids.contains(&result.ppid) {
                    filtered_results.push(result.clone());
                    ppids.insert(result.pid);
                } else {
                    backup_queue.push_back(result);
                }
            }

            queue = backup_queue;
        }

        filtered_results.sort_by(|a, b| a.order.cmp(&b.order));
        let mut file = File::create(format!("{}", self.output_path))?;
        for result in filtered_results {
            writeln!(&mut file, "{}|{}", result.op, result.path)?;
        }

        Ok(())
    }

    fn parse_lines(&self, lines: Vec<String>) -> Vec<LogEntry> {
        let regex_str = r"^\[INFO\] -> (\d+): (\d+)\|(\d+)\|([a-z])\|(.*)$";
        let regex = Regex::new(regex_str).unwrap();

        let mut order = 0;

        lines
            .into_iter()
            .map(|line| {
                if let Some(captures) = regex.captures(line.as_str()) {
                    let timestamp = captures.get(1).unwrap().as_str().parse::<u32>().unwrap();
                    let pid = captures.get(2).unwrap().as_str().parse::<u32>().unwrap();
                    let ppid = captures.get(3).unwrap().as_str().parse::<u32>().unwrap();
                    let op = captures.get(4).unwrap().as_str().chars().next().unwrap();
                    let path = captures.get(5).unwrap().as_str().to_string();

                    order += 1;

                    Some(LogEntry {
                        timestamp,
                        pid,
                        ppid,
                        op,
                        path,
                        order,
                    })
                } else {
                    None
                }
            })
            .filter(|l| l.is_some())
            .map(|l| l.unwrap())
            .collect()
    }
}

impl MutCommand for Command {
    fn execute(&mut self) -> Result<(), AppError> {
        self.start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as u32;

        let mut child = std::process::Command::new(&self.executable)
            .args(&self.args)
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let output = stream_output(&mut child)?;

        if let Some(last_line) = output.lines().collect::<Vec<&str>>().last() {
            if let Ok(pid) = last_line.parse::<u32>() {
                self.root_ppid = Some(pid);
            } else {
                return Err(AppError::Unknown);
            }
        }

        self.process_log()?;

        Ok(())
    }
}
