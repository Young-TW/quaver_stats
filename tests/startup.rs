//! Regression test for GitHub issue #6:
//! "Panic: missing background image asset crashes the request handler".
//!
//! The issue's third acceptance criterion asks for a startup test that
//! confirms the binary exits non-zero with a clear error when `assets/` is
//! absent — required assets must be validated at startup, not at request time.
//!
//! This test runs the compiled binary in a working directory that has no
//! `assets/` directory and asserts that it fails fast (exits non-zero) instead
//! of starting up successfully and only panicking later, per request.
//!
//! On the current (buggy) code the binary binds its port and serves forever
//! without ever validating the background image, so it never exits — the poll
//! below times out and the test fails, as intended for a regression test.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for the binary to exit on its own before concluding that
/// it failed to validate assets at startup.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(8);

#[test]
fn binary_exits_non_zero_when_assets_missing() {
    // A fresh empty directory has no `assets/image/quaver.jpg`. Running the
    // binary here (without disturbing the repo's real assets) reproduces the
    // "assets absent" scenario from the issue.
    let workdir = std::env::temp_dir().join(format!(
        "quaver_stats_issue6_{}_{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&workdir).expect("create temp workdir");

    let mut child = Command::new(env!("CARGO_BIN_EXE_quaver_stats"))
        .current_dir(&workdir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn quaver_stats binary");

    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let status = loop {
        match child.try_wait().expect("poll child status") {
            Some(status) => break Some(status),
            None if Instant::now() >= deadline => break None,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };

    // Clean up: if it is still running it never validated its assets.
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_dir_all(&workdir);

    let status = status.unwrap_or_else(|| {
        panic!(
            "binary did not exit within {:?} with `assets/` missing; required \
             assets are not validated at startup and the process would instead \
             panic per request (GitHub issue #6)",
            STARTUP_TIMEOUT
        )
    });

    assert!(
        !status.success(),
        "binary exited successfully with `assets/` missing; it must fail fast \
         with a clear error at startup instead (GitHub issue #6)"
    );
}
