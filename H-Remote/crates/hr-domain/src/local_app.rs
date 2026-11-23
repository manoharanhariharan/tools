//! Local application arguments, macros and templates.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::credential::SecretString;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AppArgumentType {
    #[default]
    Normal,
    Int,
    Float,
    File,
    Secret,
    Flag,
    Selection,
    Const,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppArgument {
    #[serde(rename = "type")]
    pub argument_type: AppArgumentType,
    pub name: String,
    pub key: String,
    pub is_nullable: bool,
    pub add_blank_after_key: bool,
    pub add_blank_after_value: bool,
    pub value: String,
    pub description: String,
    pub selections: BTreeMap<String, String>,
}

impl Default for AppArgument {
    fn default() -> Self {
        Self {
            argument_type: AppArgumentType::Normal,
            name: String::new(),
            key: String::new(),
            is_nullable: false,
            add_blank_after_key: true,
            add_blank_after_value: false,
            value: String::new(),
            description: String::new(),
            selections: BTreeMap::new(),
        }
    }
}

impl fmt::Debug for AppArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self.argument_type {
            AppArgumentType::Secret if !self.value.is_empty() => "***",
            _ => self.value.as_str(),
        };
        formatter
            .debug_struct("AppArgument")
            .field("argument_type", &self.argument_type)
            .field("name", &self.name)
            .field("key", &self.key)
            .field("value", &value)
            .finish()
    }
}

impl AppArgument {
    pub fn normal(
        name: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            value: value.into(),
            ..Self::default()
        }
    }

    pub fn flag(name: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            argument_type: AppArgumentType::Flag,
            ..Self::default()
        }
    }

    pub fn normalize(&mut self) {
        self.name = self.name.trim().to_string();
        self.key = self.key.trim().to_string();
        self.description = self.description.trim().to_string();
        self.selections
            .retain(|key, value| !key.trim().is_empty() && !value.trim().is_empty());
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.name.trim().is_empty() {
            issues.push("argument name is required".to_string());
        }
        if self.argument_type != AppArgumentType::Const && self.key.trim().is_empty() {
            issues.push(format!("argument '{}' needs a key", self.name.trim()));
        }
        if self.argument_type == AppArgumentType::Selection && self.selections.is_empty() {
            issues.push(format!(
                "selection argument '{}' needs at least one selection",
                self.name.trim()
            ));
        }
        if !self.is_nullable
            && self.value.trim().is_empty()
            && self.argument_type != AppArgumentType::Flag
        {
            issues.push(format!(
                "argument '{}' is required but has no value",
                self.name.trim()
            ));
        }
        issues
    }

    pub fn demonstrative(&self, context: &MacroContext, reveal_secret: bool) -> String {
        let rendered = self.render(context);
        if self.argument_type == AppArgumentType::Secret && !reveal_secret {
            return if self.key.is_empty() {
                "***".to_string()
            } else {
                format!("{} ***", self.key)
            };
        }
        rendered.join(" ")
    }

    /// Produces command-line tokens for this argument.
    pub fn render(&self, context: &MacroContext) -> Vec<String> {
        match self.argument_type {
            AppArgumentType::Flag => {
                if is_truthy(&expand_macros(&self.value, context)) {
                    vec![self.key.clone()]
                } else {
                    Vec::new()
                }
            }
            AppArgumentType::Selection => {
                let selected = expand_macros(&self.value, context);
                let mapped = self
                    .selections
                    .get(&selected)
                    .cloned()
                    .unwrap_or(selected.clone());
                self.join_key_value(&mapped)
            }
            AppArgumentType::Const => {
                vec![expand_macros(&self.value, context)]
            }
            AppArgumentType::Normal
            | AppArgumentType::Int
            | AppArgumentType::Float
            | AppArgumentType::File
            | AppArgumentType::Secret => {
                let value = expand_macros(&self.value, context);
                if value.is_empty() && self.is_nullable {
                    Vec::new()
                } else {
                    self.join_key_value(&value)
                }
            }
        }
    }

    fn join_key_value(&self, value: &str) -> Vec<String> {
        if self.key.is_empty() {
            return vec![value.to_string()];
        }
        if self.add_blank_after_key {
            vec![self.key.clone(), value.to_string()]
        } else {
            vec![format!("{}{value}", self.key)]
        }
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroContext {
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub password: SecretString,
    pub private_key_path: String,
    pub startup_path: String,
    pub ssh_version: String,
    pub startup_auto_command: String,
    pub environment: BTreeMap<String, String>,
}

impl MacroContext {
    pub fn with_host(mut self, hostname: impl Into<String>, port: u16) -> Self {
        self.hostname = hostname.into();
        self.port = port;
        self
    }

    pub fn with_environment(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(key.into(), value.into());
        self
    }
}

/// Expands `%1RM_*%` macros and environment variables. Unknown macros are left
/// untouched so a typo is visible instead of silently becoming empty.
pub fn expand_macros(input: &str, context: &MacroContext) -> String {
    let mut output = input.to_string();
    let known = [
        ("%1RM_HOSTNAME%", context.hostname.as_str()),
        ("%1RM_USERNAME%", context.username.as_str()),
        ("%1RM_PASSWORD%", context.password.expose()),
        ("%1RM_PRIVATE_KEY_PATH%", context.private_key_path.as_str()),
        ("%STARTUP_PATH%", context.startup_path.as_str()),
        ("%SSH_VERSION%", context.ssh_version.as_str()),
        (
            "%STARTUP_AUTO_COMMAND%",
            context.startup_auto_command.as_str(),
        ),
    ];
    for (macro_name, value) in known {
        output = output.replace(macro_name, value);
    }
    output = output.replace("%1RM_PORT%", &context.port.to_string());
    for (key, value) in &context.environment {
        output = output.replace(&format!("%{key}%"), value);
    }
    output
}

pub fn render_arguments(arguments: &[AppArgument], context: &MacroContext) -> Vec<String> {
    arguments
        .iter()
        .flat_map(|argument| argument.render(context))
        .filter(|value| !value.is_empty())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentTemplate {
    pub name: &'static str,
    pub example_executable: &'static str,
    pub arguments: Vec<AppArgument>,
}

impl ArgumentTemplate {
    /// Creates a LocalApp server pre-populated with this template.
    pub fn instantiate(
        &self,
        display_name: impl Into<String>,
        exe_path: impl Into<String>,
    ) -> crate::server::LocalAppServer {
        let mut server = crate::server::LocalAppServer {
            exe_path: exe_path.into(),
            argument_list: self.arguments.clone(),
            ..crate::server::LocalAppServer::default()
        };
        server.base = crate::protocol::ProtocolBase::with_display_name(
            crate::protocol::ProtocolKind::LocalApp,
            display_name,
        );
        server
            .argument_list
            .iter_mut()
            .for_each(AppArgument::normalize);
        server
    }
}

/// Templates applied when creating a LocalApp for a known application.
pub fn templates() -> Vec<ArgumentTemplate> {
    vec![
        ArgumentTemplate {
            name: "Chrome / Edge",
            example_executable: "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            arguments: vec![
                AppArgument {
                    argument_type: AppArgumentType::Flag,
                    name: "New window".into(),
                    key: "--new-window".into(),
                    value: "true".into(),
                    ..AppArgument::default()
                },
                AppArgument {
                    argument_type: AppArgumentType::File,
                    name: "Startup path".into(),
                    key: "--app".into(),
                    value: "%STARTUP_PATH%".into(),
                    is_nullable: true,
                    ..AppArgument::default()
                },
            ],
        },
        ArgumentTemplate {
            name: "NoMachine",
            example_executable: "C:\\Program Files\\NoMachine\\bin\\nxplayer.exe",
            arguments: vec![
                AppArgument::normal("Protocol", "--protocol", "NX"),
                AppArgument::normal("Host", "--host", "%1RM_HOSTNAME%"),
                AppArgument::normal("Port", "--port", "%1RM_PORT%"),
                AppArgument::normal("User", "--user", "%1RM_USERNAME%"),
                AppArgument {
                    argument_type: AppArgumentType::Secret,
                    name: "Password".into(),
                    key: "--password".into(),
                    value: "%1RM_PASSWORD%".into(),
                    is_nullable: true,
                    ..AppArgument::default()
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> MacroContext {
        MacroContext {
            hostname: "host.example".into(),
            port: 2222,
            username: "root".into(),
            password: "secret".into(),
            private_key_path: "C:\\keys\\id".into(),
            startup_path: "/srv/app".into(),
            ssh_version: "SSH-2".into(),
            startup_auto_command: "uptime".into(),
            environment: BTreeMap::from([("APP_ENV".to_string(), "prod".to_string())]),
        }
    }

    #[test]
    fn macros_expand_known_and_environment_values() {
        let input = "%1RM_HOSTNAME%:%1RM_PORT% %1RM_USERNAME% %STARTUP_PATH% %SSH_VERSION% %APP_ENV% %UNKNOWN%";
        assert_eq!(
            expand_macros(input, &context()),
            "host.example:2222 root /srv/app SSH-2 prod %UNKNOWN%"
        );
    }

    #[test]
    fn password_macros_expand_only_when_requested() {
        assert_eq!(expand_macros("%1RM_PASSWORD%", &context()), "secret");
        let argument = AppArgument {
            argument_type: AppArgumentType::Secret,
            name: "Password".into(),
            key: "--password".into(),
            value: "%1RM_PASSWORD%".into(),
            ..AppArgument::default()
        };
        assert_eq!(argument.demonstrative(&context(), false), "--password ***");
        assert_eq!(
            argument.demonstrative(&context(), true),
            "--password secret"
        );
    }

    #[test]
    fn arguments_respect_blank_and_flag_rules() {
        let spaced = AppArgument::normal("Host", "--host", "%1RM_HOSTNAME%");
        assert_eq!(spaced.render(&context()), vec!["--host", "host.example"]);

        let joined = AppArgument {
            add_blank_after_key: false,
            ..AppArgument::normal("Port", "--port=", "%1RM_PORT%")
        };
        assert_eq!(joined.render(&context()), vec!["--port=2222"]);

        let flag = AppArgument::flag("Verbose", "--verbose");
        assert_eq!(flag.render(&context()), Vec::<String>::new());
        let enabled = AppArgument {
            value: "true".into(),
            ..flag
        };
        assert_eq!(enabled.render(&context()), vec!["--verbose"]);
    }

    #[test]
    fn selection_arguments_map_values() {
        let mut argument = AppArgument {
            argument_type: AppArgumentType::Selection,
            name: "Profile".into(),
            key: "--profile".into(),
            value: "prod".into(),
            ..AppArgument::default()
        };
        argument
            .selections
            .insert("prod".into(), "production".into());
        assert_eq!(argument.render(&context()), vec!["--profile", "production"]);
    }

    #[test]
    fn templates_are_valid() {
        let templates = templates();
        assert_eq!(templates.len(), 2);
        for template in &templates {
            assert!(!template.arguments.is_empty());
            for argument in &template.arguments {
                assert!(argument.validate().is_empty(), "{:?}", argument.validate());
            }
        }
        assert_eq!(
            render_arguments(&templates[1].arguments, &context()).len(),
            10
        );

        let local = templates[0].instantiate("Browser", templates[0].example_executable);
        assert_eq!(local.base.display_name, "Browser");
        assert_eq!(local.argument_list.len(), 2);
    }

    #[test]
    fn argument_validation_catches_missing_fields() {
        let argument = AppArgument {
            name: String::new(),
            key: String::new(),
            value: String::new(),
            ..AppArgument::default()
        };
        assert!(argument.validate().len() >= 2);

        let selection = AppArgument {
            argument_type: AppArgumentType::Selection,
            name: "x".into(),
            key: "--x".into(),
            value: "a".into(),
            ..AppArgument::default()
        };
        assert!(
            selection
                .validate()
                .iter()
                .any(|issue| issue.contains("selection"))
        );
    }
}
