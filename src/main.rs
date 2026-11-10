//! `dulook`: backend process of the Dulook Omarchy Shell plugin.
//! Reads JSON commands on stdin and writes `state`/`tick` JSON lines on stdout.

use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use dulook::protocol::{Command, Event, parse_command};
use dulook::service::{Paths, Service};

const USAGE: &str =
    "usage: dulook --plugin-dir <dir> --omarchy-path <dir> --cache-file <path> --state-file <path>";
const CLOCK_INTERVAL: Duration = Duration::from_secs(1);
const HADITH_INTERVAL: Duration = Duration::from_secs(10 * 60);

fn parse_args() -> Option<Paths> {
    let (mut plugin_dir, mut omarchy_path, mut cache_file, mut state_file) = (None, None, None, None);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next()?;
        match flag.as_str() {
            "--plugin-dir" => plugin_dir = Some(PathBuf::from(value)),
            "--omarchy-path" => omarchy_path = Some(value),
            "--cache-file" => cache_file = Some(PathBuf::from(value)),
            "--state-file" => state_file = Some(PathBuf::from(value)),
            _ => return None,
        }
    }
    Some(Paths {
        plugin_dir: plugin_dir?,
        omarchy_path: omarchy_path?,
        cache_file: cache_file?,
        state_file: state_file?,
        home: std::env::var("HOME").unwrap_or_default(),
    })
}

/// Writes a message line when it differs from the previous one of the same type.
fn emit(last: &mut String, payload: &serde_json::Value) -> std::io::Result<()> {
    let text = payload.to_string();
    if *last == text {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{text}")?;
    stdout.flush()?;
    *last = text;
    Ok(())
}

fn main() -> ExitCode {
    let Some(paths) = parse_args() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    let (sender, receiver) = mpsc::channel();
    let stdin_sender = sender.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match parse_command(&line) {
                Ok(command) => {
                    if stdin_sender.send(Event::Command(command)).is_err() {
                        return;
                    }
                }
                Err(error) => eprintln!("Dulook: ignoring command: {error}"),
            }
        }
        let _ = stdin_sender.send(Event::StdinClosed);
    });

    let mut service = Service::new(paths, sender);
    let mut next_tick: Option<Instant> = None;
    let mut next_hadith: Option<Instant> = None;
    let (mut last_state, mut last_tick) = (String::new(), String::new());

    loop {
        let deadline = [next_tick, next_hadith, service.hadith_retry_at].into_iter().flatten().min();
        let event = match deadline {
            Some(deadline) => receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())),
            None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match event {
            Ok(Event::Command(command)) if !service.initialized() => match command {
                Command::Settings { settings } => {
                    service.init(&settings);
                    let now = Instant::now();
                    next_tick = Some(now + CLOCK_INTERVAL);
                    next_hadith = Some(now + HADITH_INTERVAL);
                }
                Command::Dnd { value } => service.dnd = value,
                _ => {}
            },
            Ok(Event::Command(command)) => service.handle(command),
            Ok(Event::HadithFinished(result)) => service.finish_hadith(result),
            Ok(Event::StdinClosed) | Err(RecvTimeoutError::Disconnected) => {
                service.stop_sound();
                return ExitCode::SUCCESS;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        let now = Instant::now();
        if let Some(tick) = next_tick.filter(|tick| now >= *tick) {
            service.tick();
            let following = tick + CLOCK_INTERVAL;
            next_tick = Some(if following <= now { now + CLOCK_INTERVAL } else { following });
        }
        if let Some(due) = next_hadith.filter(|due| now >= *due) {
            service.request_hadith();
            next_hadith = Some(due + HADITH_INTERVAL);
        }
        if service.hadith_retry_at.is_some_and(|retry| now >= retry) {
            service.hadith_retry_at = None;
            service.request_hadith();
        }

        if service.initialized()
            && (emit(&mut last_state, &service.state_payload()).is_err()
                || emit(&mut last_tick, &service.tick_payload()).is_err())
        {
            service.stop_sound();
            return ExitCode::SUCCESS;
        }
    }
}
