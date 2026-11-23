//! Protocol runners: how SSH/Telnet/Serial sessions are launched.
//!
//! A runner is an external executable plus an argument template. Built-ins cover
//! PuTTY/KiTTY conventions; users can add any executable and use the same
//! `%1RM_*`/`{name}` macros.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const DEFAULT_RUNNER_ID: &str = "internal";
pub const PUTTY_SSH: &str = "putty-ssh";
pub const KITTY_SSH: &str = "kitty-ssh";
pub const PUTTY_TELNET: &str = "putty-telnet";
pub const PUTTY_SERIAL: &str = "putty-serial";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RunnerKind {
    /// Protocol handled by H-Remote itself (RDP/VNC/file transfer).
    #[default]
    Internal,
    /// External executable (PuTTY, KiTTY, custom SSH client, …).
    External,
    /// External SSH client highlighted as such in the runner editor.
    ExternalSsh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Runner {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub kind: RunnerKind,
    pub exe_path: String,
    /// Argument template using `{host}`, `{port}`, `{user}`, `{password}`,
    /// `{privateKey}`, `{startupPath}`, `{sessionConfig}` or `%1RM_*` macros.
    pub arguments: String,
    /// Extra name → value substitutions for `{name}`.
    pub parameters: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub run_with_hosting: bool,
    pub session_config_path: Option<String>,
    pub install_state: String,
    pub macros: Vec<String>,
}

impl Default for Runner {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            protocol: "SSH".to_string(),
            kind: RunnerKind::External,
            exe_path: String::new(),
            arguments: String::new(),
            parameters: BTreeMap::new(),
            environment: BTreeMap::new(),
            run_with_hosting: true,
            session_config_path: None,
            install_state: "missing".to_string(),
            macros: default_macros(),
        }
    }
}

impl Runner {
    pub fn external(id: &str, name: &str, protocol: &str, exe_path: &str, arguments: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            protocol: protocol.to_string(),
            kind: RunnerKind::External,
            exe_path: exe_path.to_string(),
            arguments: arguments.to_string(),
            macros: default_macros(),
            ..Self::default()
        }
    }

    pub fn is_configured(&self) -> bool {
        self.kind == RunnerKind::Internal || !self.exe_path.trim().is_empty()
    }
}

pub fn default_macros() -> Vec<String> {
    [
        "{host}",
        "{port}",
        "{user}",
        "{password}",
        "{privateKey}",
        "{startupPath}",
        "{sessionConfig}",
        "{serialPort}",
        "{baud}",
        "{dataBits}",
        "{stopBits}",
        "{parity}",
        "{flow}",
        "{name}",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunnersConfig {
    pub runners: Vec<Runner>,
    /// Protocol id → default runner id.
    pub defaults: BTreeMap<String, String>,
}

impl Default for RunnersConfig {
    fn default() -> Self {
        Self {
            runners: builtin_runners(),
            defaults: BTreeMap::from([
                ("SSH".to_string(), PUTTY_SSH.to_string()),
                ("Telnet".to_string(), PUTTY_TELNET.to_string()),
                ("Serial".to_string(), PUTTY_SERIAL.to_string()),
            ]),
        }
    }
}

impl RunnersConfig {
    pub fn runner(&self, id: &str) -> Option<&Runner> {
        self.runners.iter().find(|runner| runner.id == id)
    }

    /// Resolves the requested runner, then the protocol default, then any
    /// configured external runner for the protocol.
    pub fn resolve(&self, protocol: &str, requested: Option<&str>) -> Option<&Runner> {
        requested
            .filter(|value| !value.trim().is_empty())
            .and_then(|value| {
                self.runner(value).or_else(|| {
                    self.runners.iter().find(|runner| {
                        runner.name.eq_ignore_ascii_case(value)
                            || runner.id.eq_ignore_ascii_case(value)
                    })
                })
            })
            .or_else(|| self.defaults.get(protocol).and_then(|id| self.runner(id)))
            .or_else(|| {
                self.runners
                    .iter()
                    .find(|runner| runner.protocol.eq_ignore_ascii_case(protocol))
            })
    }

    pub fn upsert(&mut self, runner: Runner) -> Result<(), String> {
        if runner.id.trim().is_empty() || runner.name.trim().is_empty() {
            return Err("runner id and name are required".to_string());
        }
        if let Some(existing) = self
            .runners
            .iter_mut()
            .find(|existing| existing.id == runner.id)
        {
            *existing = runner;
        } else {
            self.runners.push(runner);
        }
        Ok(())
    }

    pub fn remove(&mut self, id: &str) {
        self.runners
            .retain(|runner| runner.id != id || is_builtin_id(&runner.id));
        self.defaults.retain(|_, value| value != id);
    }

    pub fn set_default(&mut self, protocol: &str, runner_id: &str) {
        self.defaults
            .insert(protocol.to_string(), runner_id.to_string());
    }

    /// Compare built-in runner ids case-insensitively.
    pub fn builtin(&self, id: &str) -> Option<&Runner> {
        self.runners.iter().find(|runner| runner.id == id)
    }

    /// Fills in missing built-in runners after loading an older profile.
    pub fn sanitize(&mut self) {
        for builtin in builtin_runners() {
            if !self.runners.iter().any(|runner| runner.id == builtin.id) {
                self.runners.push(builtin);
            }
        }
        for runner in &mut self.runners {
            runner.protocol = runner.protocol.trim().to_string();
            runner.name = runner.name.trim().to_string();
            if runner.macros.is_empty() {
                runner.macros = default_macros();
            }
        }
        self.defaults
            .entry("SSH".to_string())
            .or_insert_with(|| PUTTY_SSH.to_string());
        self.defaults
            .entry("Telnet".to_string())
            .or_insert_with(|| PUTTY_TELNET.to_string());
        self.defaults
            .entry("Serial".to_string())
            .or_insert_with(|| PUTTY_SERIAL.to_string());
    }
}

pub fn is_builtin_id(id: &str) -> bool {
    matches!(id, PUTTY_SSH | KITTY_SSH | PUTTY_TELNET | PUTTY_SERIAL)
}

fn builtin_runners() -> Vec<Runner> {
    vec![
        Runner::external(
            PUTTY_SSH,
            "PuTTY (SSH)",
            "SSH",
            "",
            "-ssh {user}@{host} -P {port} -pw {password}",
        ),
        Runner::external(
            KITTY_SSH,
            "KiTTY (SSH)",
            "SSH",
            "",
            "-ssh {user}@{host} -P {port} -pw {password}",
        ),
        Runner::external(
            PUTTY_TELNET,
            "PuTTY (Telnet)",
            "Telnet",
            "",
            "-telnet {host} -P {port}",
        ),
        Runner::external(
            PUTTY_SERIAL,
            "PuTTY (Serial)",
            "Serial",
            "",
            "-serial {serialPort} -sercfg {baud},{dataBits},{parity},{stopBits},{flow}",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_include_putty_and_resolve_by_protocol() {
        let config = RunnersConfig::default();
        let runner = config.resolve("SSH", None).unwrap();
        assert_eq!(runner.id, PUTTY_SSH);
        assert!(!runner.is_configured());
        assert_eq!(
            config.resolve("Telnet", Some(KITTY_SSH)).unwrap().id,
            KITTY_SSH
        );
    }

    #[test]
    fn requested_runner_wins_over_default() {
        let config = RunnersConfig::default();
        assert_eq!(
            config.resolve("SSH", Some(KITTY_SSH)).unwrap().id,
            KITTY_SSH
        );
        assert_eq!(
            config.resolve("Telnet", Some(KITTY_SSH)).unwrap().id,
            KITTY_SSH
        );
    }

    #[test]
    fn upsert_adds_and_updates_and_keeps_builtins() {
        let mut config = RunnersConfig::default();
        let mut runner = Runner::external("custom", "Custom", "SSH", "C:\\ssh.exe", "-p {port}");
        runner.environment.insert("TERM".into(), "xterm".into());
        config.upsert(runner.clone()).unwrap();
        assert_eq!(config.runner("custom").unwrap().name, "Custom");
        runner.name = "Renamed".into();
        config.upsert(runner).unwrap();
        assert_eq!(config.runner("custom").unwrap().name, "Renamed");
        config.remove(PUTTY_SSH);
        assert!(config.runner(PUTTY_SSH).is_some());
        config.remove("custom");
        assert!(config.runner("custom").is_none());
    }

    #[test]
    fn external_ssh_kind_behaves_like_an_external_runner() {
        let mut runner = Runner::external("ssh-client", "SSH client", "SSH", "", "-p {port}");
        runner.kind = RunnerKind::ExternalSsh;
        assert!(!runner.is_configured());
        runner.exe_path = "C:\\Windows\\System32\\OpenSSH\\ssh.exe".into();
        assert!(runner.is_configured());
        let json = serde_json::to_string(&runner).unwrap();
        assert!(json.contains("externalSsh"));
        let round_trip: Runner = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip.kind, RunnerKind::ExternalSsh);
    }

    #[test]
    fn sanitize_restores_missing_builtins_and_defaults() {
        let mut config = RunnersConfig {
            runners: Vec::new(),
            defaults: BTreeMap::new(),
        };
        config.sanitize();
        assert!(config.runner(PUTTY_SSH).is_some());
        assert_eq!(
            config.defaults.get("SSH").map(String::as_str),
            Some(PUTTY_SSH)
        );
    }
}
