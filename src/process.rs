use std::{
    ffi::OsStr,
    io,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn run(command: &OsStr, args: &[&OsStr]) -> Result<Output, String> {
    let timeout = std::env::var("CONVERSION_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300);
    run_with_timeout(command, args, Duration::from_secs(timeout), None)
}

pub fn run_limited(
    command: &OsStr,
    args: &[&OsStr],
    output_path: &std::path::Path,
    max_output_size: u64,
) -> Result<Output, String> {
    let timeout = std::env::var("CONVERSION_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300);
    run_with_timeout(
        command,
        args,
        Duration::from_secs(timeout),
        Some((output_path, max_output_size)),
    )
}

fn run_with_timeout(
    command: &OsStr,
    args: &[&OsStr],
    timeout: Duration,
    output_limit: Option<(&std::path::Path, u64)>,
) -> Result<Output, String> {
    let mut command_builder = Command::new(command);
    command_builder
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command_builder.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command_builder
        .spawn()
        .map_err(|error| format!("failed to start conversion tool: {error}"))?;
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("failed to collect conversion result: {error}"));
            }
            Ok(None) => {
                if let Some((output_path, max_output_size)) = output_limit
                    && output_path
                        .metadata()
                        .map(|metadata| metadata.len())
                        .unwrap_or(0)
                        > max_output_size
                {
                    terminate(&mut child);
                    return Err("conversion output exceeded the configured limit".to_string());
                }
                if Instant::now() >= deadline {
                    terminate(&mut child);
                    return Err("conversion timed out".to_string());
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                terminate(&mut child);
                return Err(format!("failed to monitor conversion: {error}"));
            }
        }
    }
}

fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run_args(command: &str, args: &[&OsStr]) -> Result<Output, String> {
    run(OsStr::new(command), args)
}

pub fn output_error(output: &Output, name: &str) -> String {
    let _ = output;
    format!("{name} failed")
}

pub fn output_exists(output: &Output, path: &std::path::Path, name: &str) -> Result<(), String> {
    if !output.status.success() {
        return Err(output_error(output, name));
    }
    if !path.exists() {
        return Err(format!("{name} did not create the output file"));
    }
    Ok(())
}

pub fn io_error(error: io::Error, name: &str) -> String {
    format!("failed to start {name}: {error}")
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::ffi::OsStr;

    #[test]
    fn reports_missing_process_without_panicking() {
        let result = run(OsStr::new("euoni-command-that-does-not-exist"), &[]);
        assert!(
            result
                .unwrap_err()
                .contains("failed to start conversion tool")
        );
    }

    #[test]
    fn terminates_a_process_that_exceeds_the_timeout() {
        let result = super::run_with_timeout(
            OsStr::new("sleep"),
            &[OsStr::new("1")],
            std::time::Duration::from_millis(1),
            None,
        );
        assert_eq!(result.unwrap_err(), "conversion timed out");
    }

    #[test]
    fn terminates_a_process_when_output_exceeds_the_limit() {
        let path = std::env::temp_dir().join(format!("euoni-process-limit-{}", std::process::id()));
        let result = super::run_limited(
            OsStr::new("sh"),
            &[
                OsStr::new("-c"),
                OsStr::new("dd if=/dev/zero of=\"$1\" bs=1024 count=2; sleep 1"),
                OsStr::new("sh"),
                path.as_os_str(),
            ],
            &path,
            1024,
        );
        assert_eq!(
            result.unwrap_err(),
            "conversion output exceeded the configured limit"
        );
        let _ = std::fs::remove_file(path);
    }
}
