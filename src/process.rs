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
    run_with_timeout(command, args, Duration::from_secs(timeout))
}

fn run_with_timeout(command: &OsStr, args: &[&OsStr], timeout: Duration) -> Result<Output, String> {
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
            Ok(None) if Instant::now() >= deadline => {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err("conversion timed out".to_string());
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("failed to monitor conversion: {error}"));
            }
        }
    }
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
        );
        assert_eq!(result.unwrap_err(), "conversion timed out");
    }
}
