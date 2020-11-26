// EndBASIC
// Copyright 2020 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Integration tests that use golden input and output files.

// TODO(jmmv): All of the supporting code here is duplicated in the other EndBASIC crates.
// Should probably add a separate testutils crate, but I'm hesitant for now.

// Keep these in sync with other top-level files.
#![warn(anonymous_parameters, bad_style, missing_docs)]
#![warn(unused, unused_extern_crates, unused_import_braces, unused_qualifications)]
#![warn(unsafe_code)]

use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process;

/// Matches a formatted date.
const DATE_RE: &str = "[0-9]{4}-[0-9]{2}-[0-9]{2} [0-2][0-9]:[0-5][0-9]";

/// Matches a version number.
const VERSION_RE: &str = "[0-9]+\\.[0-9]+\\.[0-9]+";

/// Computes the path to the directory where this test's binary lives.
fn self_dir() -> PathBuf {
    let self_exe = env::current_exe().expect("Cannot get self's executable path");
    let dir = self_exe.parent().expect("Cannot get self's directory");
    assert!(dir.ends_with("target/debug/deps") || dir.ends_with("target/release/deps"));
    dir.to_owned()
}

/// Computes the path to the built binary `name`.
fn bin_path<P: AsRef<Path>>(name: P) -> PathBuf {
    let test_dir = self_dir();
    let debug_or_release_dir = test_dir.parent().expect("Failed to get parent directory");
    debug_or_release_dir.join(name).with_extension(env::consts::EXE_EXTENSION)
}

/// Computes the path to the source file `name`.
fn src_path(name: &str) -> PathBuf {
    let test_dir = self_dir();
    let debug_or_release_dir = test_dir.parent().expect("Failed to get parent directory");
    let target_dir = debug_or_release_dir.parent().expect("Failed to get parent directory");
    let dir = target_dir.parent().expect("Failed to get parent directory");

    // Sanity-check that we landed in the right location.
    assert!(dir.join("Cargo.toml").exists());

    dir.join(name)
}

/// Same as `src_path` but returns a string reference for the few places where we need this.
fn src_str(p: &str) -> String {
    src_path(p).to_str().expect("Need paths to be valid strings").to_owned()
}

/// Describes the behavior for one of the three streams (stdin, stdout, stderr) connected to a
/// program.
enum Behavior {
    /// Ensure the stream is silent.
    Null,

    /// If stdin, feed the given path as the program's input.  If stdout/stderr, expect the contents
    /// of the stream to match this file.
    File(PathBuf),

    /// If stdin, this is not supported.  If stdout/stderr, expect the contents of the stream to
    /// match this literal string.
    Literal(String),
}

/// Reads the contents of a golden data file.
fn read_golden(path: &Path) -> String {
    let mut f = File::open(path).expect("Failed to open golden data file");
    let mut golden = vec![];
    f.read_to_end(&mut golden).expect("Failed to read golden data file");
    let raw = String::from_utf8(golden).expect("Golden data file is not valid UTF-8");
    let golden = if cfg!(target_os = "windows") { raw.replace("\r\n", "\n") } else { raw };

    // This is the opposite of apply_mocks and ensures we don't leak actual values into the golden
    // files by mistake.
    let version_re = regex::Regex::new(VERSION_RE).unwrap();
    assert!(
        !version_re.is_match(&golden),
        "Golden file {} contains a version number",
        path.display()
    );
    let date_re = regex::Regex::new(DATE_RE).unwrap();
    assert!(!date_re.is_match(&golden), "Golden file {} contains a date", path.display());

    golden
}

/// Replaces the parts of the output that can change due to the environment with placeholders.
fn apply_mocks(input: String) -> String {
    let version_re = regex::Regex::new(VERSION_RE).unwrap();
    let input = version_re.replace_all(&input, "X.Y.Z").to_owned();

    let date_re = regex::Regex::new(DATE_RE).unwrap();
    date_re.replace_all(&input, "YYYY-MM-DD HH:MM").into()
}

/// Runs `bin` with arguments `args` and checks its behavior against expectations.
///
/// `exp_code` is the expected error code from the program.  `stdin_behavior` indicates what to feed
/// to the program's stdin.  `stdout_behavior` and `stderr_behavior` indicate what to expect from
/// the program's textual output.
fn check<P: AsRef<Path>>(
    bin: P,
    args: &[&str],
    exp_code: i32,
    stdin_behavior: Behavior,
    stdout_behavior: Behavior,
    stderr_behavior: Behavior,
) {
    let golden_stdin = match stdin_behavior {
        Behavior::Null => process::Stdio::null(),
        Behavior::File(path) => File::open(path).unwrap().into(),
        Behavior::Literal(_) => panic!("Literals not supported for stdin"),
    };

    let exp_stdout = match stdout_behavior {
        Behavior::Null => "".to_owned(),
        Behavior::File(path) => read_golden(&path),
        Behavior::Literal(text) => text,
    };

    let exp_stderr = match stderr_behavior {
        Behavior::Null => "".to_owned(),
        Behavior::File(path) => read_golden(&path),
        Behavior::Literal(text) => text,
    };

    let result = process::Command::new(bin.as_ref())
        .args(args)
        .stdin(golden_stdin)
        .env("LINES", "24")
        .env("COLUMNS", "80")
        .output()
        .expect("Failed to execute subprocess");
    let code = result.status.code().expect("Subprocess didn't exit cleanly");
    let stdout =
        apply_mocks(String::from_utf8(result.stdout).expect("Stdout not is not valid UTF-8"));
    let stderr =
        apply_mocks(String::from_utf8(result.stderr).expect("Stderr not is not valid UTF-8"));

    if exp_code != code || exp_stdout != stdout || exp_stderr != stderr {
        eprintln!("Exit code: {}", code);
        eprintln!("stdout:\n{}", stdout);
        eprintln!("stderr:\n{}", stderr);
        assert_eq!(exp_code, code);
        assert_eq!(exp_stdout, stdout);
        assert_eq!(exp_stderr, stderr);
    }
}

#[test]
fn test_cli_help() {
    fn check_with_args(args: &[&str]) {
        check(
            &bin_path("endbasic"),
            args,
            0,
            Behavior::Null,
            Behavior::File(src_path("cli/tests/cli/help.out")),
            Behavior::Null,
        );
    }
    check_with_args(&["-h"]);
    check_with_args(&["--help"]);
    check_with_args(&["--version", "--help"]);
    check_with_args(&["the", "--help", "flag always wins"]);
}

// TODO(jmmv): This test fails almost always on Linux CI builds with `Text file busy` when
// attempting to run the copied binary.  I've also gotten it to occasionally fail on a local Linux
// installation in the same way, but that's much harder to trigger.  Investigate what's going on.
#[cfg(not(target_os = "linux"))]
#[test]
fn test_cli_program_name_uses_arg0() {
    struct DeleteOnDrop<'a> {
        path: &'a Path,
    }

    impl<'a> Drop for DeleteOnDrop<'a> {
        fn drop(&mut self) {
            let _best_effort_removal = fs::remove_file(self.path);
        }
    }

    let original = bin_path("endbasic");
    let custom = self_dir().join("custom-name").with_extension(env::consts::EXE_EXTENSION);
    let _delete_custom = DeleteOnDrop { path: &custom };
    fs::copy(&original, &custom).unwrap();
    check(
        &custom,
        &["one", "two", "three"],
        2,
        Behavior::Null,
        Behavior::Null,
        Behavior::Literal(
            "Usage error: Too many arguments\nType custom-name --help for more information\n"
                .to_owned(),
        ),
    );
}

#[test]
fn test_cli_too_many_args() {
    check(
        &bin_path("endbasic"),
        &["foo", "bar"],
        2,
        Behavior::Null,
        Behavior::Null,
        Behavior::Literal(
            "Usage error: Too many arguments\nType endbasic --help for more information\n"
                .to_owned(),
        ),
    );
}

#[test]
fn test_cli_unknown_option() {
    check(
        &bin_path("endbasic"),
        &["-Z", "some-file"],
        2,
        Behavior::Null,
        Behavior::Null,
        Behavior::Literal(
            "Usage error: Unrecognized option: 'Z'\nType endbasic --help for more information\n"
                .to_owned(),
        ),
    );
}

#[test]
fn test_cli_version() {
    fn check_with_args(args: &[&str]) {
        check(
            &bin_path("endbasic"),
            args,
            0,
            Behavior::Null,
            Behavior::File(src_path("cli/tests/cli/version.out")),
            Behavior::Null,
        );
    }
    check_with_args(&["--version"]);
    check_with_args(&["the", "--version", "flag wins over arguments"]);
}

#[test]
fn test_repl_console() {
    check(
        bin_path("endbasic"),
        &[&src_str("cli/tests/repl/console.bas")],
        0,
        Behavior::Null,
        Behavior::File(src_path("cli/tests/repl/console.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_dir() {
    let dir = tempfile::tempdir().unwrap();
    let subdir = dir.path().join("subdir"); // Start with a non-existent directory.
    check(
        bin_path("endbasic"),
        &["--programs-dir", subdir.to_str().unwrap()],
        0,
        Behavior::File(src_path("cli/tests/repl/dir.in")),
        Behavior::File(src_path("cli/tests/repl/dir.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_editor() {
    check(
        bin_path("endbasic"),
        &[],
        0,
        Behavior::File(src_path("cli/tests/repl/editor.in")),
        Behavior::File(src_path("cli/tests/repl/editor.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_exit() {
    check(
        bin_path("endbasic"),
        &[&src_str("cli/tests/repl/exit.bas")],
        78,
        Behavior::Null,
        Behavior::Null,
        Behavior::Null,
    );

    check(
        bin_path("endbasic"),
        &[],
        78,
        Behavior::File(src_path("cli/tests/repl/exit.bas")),
        Behavior::File(src_path("cli/tests/repl/exit.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_help() {
    check(
        bin_path("endbasic"),
        &[],
        0,
        Behavior::File(src_path("cli/tests/repl/help.in")),
        Behavior::File(src_path("cli/tests/repl/help.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_interactive() {
    check(
        bin_path("endbasic"),
        &[],
        0,
        Behavior::File(src_path("cli/tests/repl/interactive.in")),
        Behavior::File(src_path("cli/tests/repl/interactive.out")),
        Behavior::Null,
    );
}

#[test]
fn test_repl_load_save() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(&src_path("cli/tests/repl/hello.bas"), &dir.path().join("hello.bas")).unwrap();
    assert!(!dir.path().join("hello2.bas").exists());
    check(
        bin_path("endbasic"),
        &["--programs-dir", dir.path().to_str().unwrap()],
        0,
        Behavior::File(src_path("cli/tests/repl/load-save.in")),
        Behavior::File(src_path("cli/tests/repl/load-save.out")),
        Behavior::Null,
    );
    assert!(dir.path().join("hello2.bas").exists());
}

#[test]
fn test_repl_state_sharing() {
    check(
        bin_path("endbasic"),
        &[],
        0,
        Behavior::File(src_path("cli/tests/repl/state-sharing.in")),
        Behavior::File(src_path("cli/tests/repl/state-sharing.out")),
        Behavior::Null,
    );
}
