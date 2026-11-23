//! Pre/post-connect script execution (P2-52/P9-05/P9-06).
//!
//! Scripts run through the platform shell (`cmd /C` or `sh -c`) and receive the
//! session environment as ordinary environment variables so they never have to
//! parse arguments.

use std::collections::BTreeMap;
use std::process::Command;

use hr_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptResult {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Variables exported to pre/post-connect scripts (P9-06).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptEnvironment {
    pub session_id: String,
    pub server_id: String,
    pub server_name: String,
    pub server_host: String,
    pub server_tags: Vec<String>,
}

impl ScriptEnvironment {
    pub fn variables(&self) -> BTreeMap<String, String> {
        let mut variables = BTreeMap::new();
        variables.insert("SESSION_ID".to_string(), self.session_id.clone());
        variables.insert("SERVER_ID".to_string(), self.server_id.clone());
        variables.insert("SERVER_NAME".to_string(), self.server_name.clone());
        variables.insert("SERVER_HOST".to_string(), self.server_host.clone());
        variables.insert("SERVER_TAGS".to_string(), self.server_tags.join(","));
        variables
    }
}

pub fn run_script(script: &str, hide_window: bool) -> AppResult<ScriptResult> {
    run_script_with_env(script, hide_window, &BTreeMap::new())
}

pub fn run_script_with_env(
    script: &str,
    hide_window: bool,
    environment: &BTreeMap<String, String>,
) -> AppResult<ScriptResult> {
    if script.trim().is_empty() {
        return Ok(ScriptResult {
            success: true,
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        });
    }

    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut command = Command::new("cmd");
        // `raw_arg` keeps user quotes/backslashes intact; the default escaping
        // breaks quoted paths in `cmd /C` scripts.
        command.arg("/C").raw_arg(script);
        if hide_window {
            command.creation_flags(CREATE_NO_WINDOW);
        }
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let _ = hide_window;
        let mut command = Command::new("sh");
        command.args(["-c", script]);
        command
    };

    command.envs(environment);
    let output = command
        .output()
        .map_err(|error| AppError::platform(format!("cannot run script: {error}")))?;
    Ok(ScriptResult {
        success: output.status.success(),
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_script_succeeds() {
        let result = run_script("  ", true).unwrap();
        assert!(result.success);
        assert_eq!(result.code, Some(0));
    }

    #[test]
    fn captures_output_and_exit_code() {
        #[cfg(windows)]
        let script = "echo hello";
        #[cfg(not(windows))]
        let script = "printf hello";
        let result = run_script(script, true).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));

        #[cfg(windows)]
        let failing = "exit /b 3";
        #[cfg(not(windows))]
        let failing = "exit 3";
        let result = run_script(failing, true).unwrap();
        assert!(!result.success);
        assert_eq!(result.code, Some(3));
    }

    #[test]
    fn environment_variables_reach_the_script() {
        let environment = BTreeMap::from([
            ("SESSION_ID".to_string(), "session-1".to_string()),
            ("SERVER_TAGS".to_string(), "prod,linux".to_string()),
        ]);
        #[cfg(windows)]
        let script = "echo %SESSION_ID% %SERVER_TAGS%";
        #[cfg(not(windows))]
        let script = "printf '%s %s' \"$SESSION_ID\" \"$SERVER_TAGS\"";
        let result = run_script_with_env(script, true, &environment).unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("session-1"));
        assert!(result.stdout.contains("prod,linux"));
    }

    #[test]
    fn quoted_paths_with_spaces_are_executed() {
        let path = std::env::temp_dir().join(format!("hremote script {}.txt", std::process::id()));
        let script = format!("echo ok > \"{}\"", path.display());
        let result = run_script(&script, true).unwrap();
        assert!(result.success, "{:?}", result.stderr);
        assert!(path.is_file());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn script_environment_exports_the_documented_variables() {
        let environment = ScriptEnvironment {
            session_id: "session".into(),
            server_id: "server".into(),
            server_name: "Box".into(),
            server_host: "10.0.0.5".into(),
            server_tags: vec!["prod".into(), "linux".into()],
        };
        let variables = environment.variables();
        assert_eq!(variables["SESSION_ID"], "session");
        assert_eq!(variables["SERVER_ID"], "server");
        assert_eq!(variables["SERVER_NAME"], "Box");
        assert_eq!(variables["SERVER_HOST"], "10.0.0.5");
        assert_eq!(variables["SERVER_TAGS"], "prod,linux");
    }
}
