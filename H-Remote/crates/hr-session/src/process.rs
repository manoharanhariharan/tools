//! Plans for external-process protocols: SSH/Telnet/Serial runners, RemoteApp
//! and local applications.

use std::collections::BTreeMap;

use hr_config::{Runner, RunnersConfig};
use hr_core::{AppError, AppResult};
use hr_domain::{MacroContext, SerialFlowControl, SerialParity, Server, render_arguments};
use serde::{Deserialize, Serialize};

use crate::state::SessionBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProcessKind {
    Ssh,
    Telnet,
    Serial,
    LocalApp,
    RemoteApp,
    Custom,
}

impl ProcessKind {
    pub fn protocol(self) -> &'static str {
        match self {
            Self::Ssh => "SSH",
            Self::Telnet => "Telnet",
            Self::Serial => "Serial",
            Self::LocalApp => "LocalApp",
            Self::RemoteApp => "RemoteApp",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Clone)]
pub struct ProcessPlan {
    pub server_id: String,
    pub server_name: String,
    pub kind: ProcessKind,
    pub program: String,
    pub arguments: Vec<String>,
    pub working_dir: Option<String>,
    pub environment: BTreeMap<String, String>,
    /// `.rdp`/script content written to a temporary file by the backend.
    pub temporary_content: Option<(String, String)>,
    /// Private key material written to a temp file before launch and deleted
    /// afterwards (P9-07): `(path, content)`.
    pub private_key_file: Option<(String, String)>,
    pub run_with_hosting: bool,
    pub max_attempts: u32,
    pub warnings: Vec<String>,
}

impl std::fmt::Debug for ProcessPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProcessPlan")
            .field("server_id", &self.server_id)
            .field("server_name", &self.server_name)
            .field("kind", &self.kind)
            .field("program", &self.program)
            .field("argument_count", &self.arguments.len())
            .field("has_private_key_file", &self.private_key_file.is_some())
            .field("run_with_hosting", &self.run_with_hosting)
            .finish_non_exhaustive()
    }
}

impl ProcessPlan {
    pub fn backend(&self) -> SessionBackend {
        match self.kind {
            ProcessKind::Ssh | ProcessKind::Telnet | ProcessKind::Serial | ProcessKind::Custom => {
                SessionBackend::Terminal
            }
            ProcessKind::LocalApp => SessionBackend::LocalApp,
            ProcessKind::RemoteApp => SessionBackend::RemoteApp,
        }
    }

    pub fn from_server(server: &Server, runners: &RunnersConfig) -> AppResult<Self> {
        match server {
            Server::Ssh(ssh) => {
                let runner =
                    resolve_runner(runners, "SSH", ssh.base.selected_runner_name.as_deref())?;
                // Inline PEM material is written to a short-lived file so the
                // runner never receives the key on its command line (P9-07).
                let (private_key_path, private_key_file) =
                    split_private_key(ssh.credentials.private_key.as_deref());
                let context = MacroContext {
                    hostname: ssh.address.address.clone(),
                    port: ssh.address.port,
                    username: ssh.credentials.username.clone(),
                    password: ssh.credentials.password.clone(),
                    private_key_path,
                    startup_auto_command: ssh.startup_auto_command.clone(),
                    ssh_version: ssh.ssh_version.clone(),
                    ..MacroContext::default()
                };
                let session_config = ssh
                    .external_session_config_path
                    .clone()
                    .or_else(|| runner.session_config_path.clone());
                let mut plan = runner_plan(
                    server,
                    ProcessKind::Ssh,
                    runner,
                    &context,
                    session_config,
                    ssh.startup_auto_command.clone(),
                    private_key_file,
                )?;
                if ssh.open_sftp_on_connected {
                    plan.warnings.push(
                        "Open SFTP on connect requires the file-transfer client (Phase 6b)."
                            .to_string(),
                    );
                }
                Ok(plan)
            }
            Server::Telnet(telnet) => {
                let runner = resolve_runner(
                    runners,
                    "Telnet",
                    telnet.base.selected_runner_name.as_deref(),
                )?;
                let context = MacroContext {
                    hostname: telnet.address.address.clone(),
                    port: telnet.address.port,
                    username: telnet.credentials.username.clone(),
                    password: telnet.credentials.password.clone(),
                    startup_auto_command: telnet.startup_auto_command.clone(),
                    ..MacroContext::default()
                };
                let session_config = telnet
                    .external_session_config_path
                    .clone()
                    .or_else(|| runner.session_config_path.clone());
                runner_plan(
                    server,
                    ProcessKind::Telnet,
                    runner,
                    &context,
                    session_config,
                    telnet.startup_auto_command.clone(),
                    None,
                )
            }
            Server::Serial(serial) => {
                let runner = resolve_runner(
                    runners,
                    "Serial",
                    serial.base.selected_runner_name.as_deref(),
                )?;
                let mut context = MacroContext::default();
                context
                    .environment
                    .insert("SERIAL_PORT".to_string(), serial.serial_port.clone());
                context
                    .environment
                    .insert("BAUD".to_string(), serial.bit_rate.to_string());
                context
                    .environment
                    .insert("DATA_BITS".to_string(), serial.data_bits.to_string());
                context
                    .environment
                    .insert("STOP_BITS".to_string(), serial.stop_bits.to_string());
                context
                    .environment
                    .insert("PARITY".to_string(), parity_code(serial.parity).to_string());
                context.environment.insert(
                    "FLOW".to_string(),
                    flow_code(serial.flow_control).to_string(),
                );
                context.startup_auto_command = serial.startup_auto_command.clone();
                let session_config = serial
                    .external_session_config_path
                    .clone()
                    .or_else(|| runner.session_config_path.clone());
                runner_plan(
                    server,
                    ProcessKind::Serial,
                    runner,
                    &context,
                    session_config,
                    serial.startup_auto_command.clone(),
                    None,
                )
            }
            Server::LocalApp(local) => {
                let context = MacroContext::default();
                let program = hr_domain::expand_macros(&local.exe_path, &context);
                if program.trim().is_empty() {
                    return Err(AppError::validation(
                        "the local application executable path is required",
                    ));
                }
                let arguments = if !local.argument_list.is_empty() {
                    render_arguments(&local.argument_list, &context)
                } else {
                    split_arguments(&local.arguments)
                };
                let warnings = if local.run_with_hosting {
                    vec![
                        "Native window hosting is not available yet; the app opens in its own window."
                            .to_string(),
                    ]
                } else {
                    Vec::new()
                };
                Ok(Self {
                    server_id: local.base.id.clone(),
                    server_name: local.base.display_name.clone(),
                    kind: ProcessKind::LocalApp,
                    program,
                    arguments,
                    working_dir: None,
                    environment: BTreeMap::new(),
                    temporary_content: None,
                    private_key_file: None,
                    run_with_hosting: local.run_with_hosting,
                    max_attempts: 1,
                    warnings,
                })
            }
            Server::RemoteApp(app) => {
                let content = remote_app_rdp(app)?;
                Ok(Self {
                    server_id: app.base.id.clone(),
                    server_name: app.base.display_name.clone(),
                    kind: ProcessKind::RemoteApp,
                    program: "mstsc.exe".to_string(),
                    arguments: Vec::new(),
                    working_dir: None,
                    environment: BTreeMap::new(),
                    temporary_content: Some(("rdp".to_string(), content)),
                    private_key_file: None,
                    run_with_hosting: false,
                    max_attempts: 1,
                    warnings: vec![
                        "RemoteApp launches through the external mstsc engine.".to_string(),
                    ],
                })
            }
            Server::Custom(custom) => custom_plan(server, custom, runners),
            _ => Err(AppError::unsupported(format!(
                "{} sessions are handled by another backend",
                server.protocol().id()
            ))),
        }
    }
}

/// Custom protocols reuse the external-runner model: the runner is resolved by
/// the protocol name, and `extraFields` string values become both `{field}`
/// template variables and environment variables (P10-01).
fn custom_plan(
    server: &Server,
    custom: &hr_domain::CustomServer,
    runners: &RunnersConfig,
) -> AppResult<ProcessPlan> {
    let protocol = custom.custom_protocol_name.trim();
    if protocol.is_empty() {
        return Err(AppError::validation("the custom protocol name is required"));
    }
    if hr_domain::ProtocolKind::is_builtin_name(protocol) {
        return Err(AppError::validation(
            "built-in protocol names cannot be redefined",
        ));
    }
    let runner = resolve_runner(
        runners,
        protocol,
        custom.base.selected_runner_name.as_deref(),
    )?;
    let (private_key_path, private_key_file) =
        split_private_key(custom.credentials.private_key.as_deref());
    let mut context = MacroContext {
        hostname: custom.address.address.clone(),
        port: custom.address.port,
        username: custom.credentials.username.clone(),
        password: custom.credentials.password.clone(),
        private_key_path,
        ..MacroContext::default()
    };
    if let Some(fields) = custom.extra_fields.as_object() {
        for (key, value) in fields {
            let Some(value) = value.as_str() else {
                continue;
            };
            context.environment.insert(key.clone(), value.to_string());
            context
                .environment
                .insert(format!("CUSTOM_{}", key.to_uppercase()), value.to_string());
        }
    }
    let session_config = runner.session_config_path.clone();
    let mut plan = runner_plan(
        server,
        ProcessKind::Custom,
        runner,
        &context,
        session_config,
        String::new(),
        private_key_file,
    )?;
    plan.environment.extend(context.environment.clone());
    Ok(plan)
}

fn resolve_runner<'a>(
    runners: &'a RunnersConfig,
    protocol: &str,
    requested: Option<&str>,
) -> AppResult<&'a Runner> {
    let runner = runners.resolve(protocol, requested).ok_or_else(|| {
        AppError::validation(format!(
            "no {protocol} runner is configured. Add PuTTY/KiTTY or an external SSH client in Settings → Runners."
        ))
    })?;
    if let Some(requested) = requested.filter(|value| !value.trim().is_empty())
        && runner.name != requested
        && runner.id != requested
    {
        return Err(AppError::validation(format!(
            "runner '{requested}' is not available for {protocol}"
        )));
    }
    Ok(runner)
}

fn runner_plan(
    server: &Server,
    kind: ProcessKind,
    runner: &Runner,
    context: &MacroContext,
    session_config: Option<String>,
    startup_command: String,
    private_key_file: Option<(String, String)>,
) -> AppResult<ProcessPlan> {
    if runner.kind == hr_config::RunnerKind::Internal {
        return Err(AppError::unsupported(
            "internal terminal hosting is not available; choose an external runner",
        ));
    }
    if !runner.is_configured() {
        return Err(AppError::validation(format!(
            "runner '{}' has no executable path. Configure it in Settings → Runners.",
            runner.name
        )));
    }
    let mut args = split_arguments(&expand_runner_template(
        &runner.arguments,
        context,
        session_config.as_deref(),
    )?);
    if let Some(config) = session_config
        .clone()
        .filter(|value| !value.trim().is_empty())
        && !args.iter().any(|arg| arg == "-load")
    {
        args.push("-load".to_string());
        args.push(config);
    }
    let temporary_content = None;
    if !startup_command.trim().is_empty() {
        let path = std::env::temp_dir().join(format!("hremote-startup-{}.txt", hr_core::new_id()));
        if let Err(error) = std::fs::write(&path, &startup_command) {
            return Err(AppError::platform(format!(
                "cannot write the startup command file: {error}"
            )));
        }
        args.push("-m".to_string());
        args.push(path.display().to_string());
    }
    Ok(ProcessPlan {
        server_id: server.base().id.clone(),
        server_name: server.base().display_name.clone(),
        kind,
        program: expand_runner_template(&runner.exe_path, context, session_config.as_deref())?,
        arguments: args,
        working_dir: None,
        environment: runner.environment.clone(),
        temporary_content,
        private_key_file,
        run_with_hosting: runner.run_with_hosting,
        max_attempts: 1,
        warnings: if server.base().always_open_in_new_tab_window {
            vec!["The runner opens in an external window.".to_string()]
        } else {
            Vec::new()
        },
    })
}

/// Expands `{name}`/`{host}` placeholders and `%1RM_*` macros. Password is
/// expanded last so a password containing `{...}` is not re-expanded.
pub fn expand_runner_template(
    template: &str,
    context: &MacroContext,
    session_config: Option<&str>,
) -> AppResult<String> {
    let mut values = BTreeMap::new();
    values.insert("host".to_string(), context.hostname.clone());
    values.insert("port".to_string(), context.port.to_string());
    values.insert("user".to_string(), context.username.clone());
    values.insert("privateKey".to_string(), context.private_key_path.clone());
    values.insert("startupPath".to_string(), context.startup_path.clone());
    values.insert(
        "sessionConfig".to_string(),
        session_config.unwrap_or("").to_string(),
    );
    values.insert(
        "serialPort".to_string(),
        context
            .environment
            .get("SERIAL_PORT")
            .cloned()
            .unwrap_or_default(),
    );
    values.insert(
        "baud".to_string(),
        context.environment.get("BAUD").cloned().unwrap_or_default(),
    );
    values.insert(
        "dataBits".to_string(),
        context
            .environment
            .get("DATA_BITS")
            .cloned()
            .unwrap_or_default(),
    );
    values.insert(
        "stopBits".to_string(),
        context
            .environment
            .get("STOP_BITS")
            .cloned()
            .unwrap_or_default(),
    );
    values.insert(
        "parity".to_string(),
        context
            .environment
            .get("PARITY")
            .cloned()
            .unwrap_or_default(),
    );
    values.insert(
        "flow".to_string(),
        context.environment.get("FLOW").cloned().unwrap_or_default(),
    );
    values.insert("name".to_string(), context.username.clone());
    // Custom protocol fields (and serial settings) are addressable as macros.
    for (key, value) in &context.environment {
        values.insert(key.clone(), value.clone());
        values.insert(key.to_lowercase(), value.clone());
    }
    let mut output = template.to_string();
    for (name, value) in &values {
        output = output.replace(&format!("{{{name}}}"), value);
    }
    output = hr_domain::expand_macros(&output, context);
    // Password last: it may contain braces.
    output = output.replace("{password}", context.password.expose());
    if output.contains('{') && output.contains('}') {
        return Err(AppError::validation(
            "the runner template contains an unknown macro",
        ));
    }
    Ok(output)
}

/// Detects inline PEM material and reserves a temp path for it. Paths are left
/// untouched so existing path-based configurations keep working.
fn split_private_key(value: Option<&str>) -> (String, Option<(String, String)>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return (String::new(), None);
    };
    let looks_inline = value.contains("-----BEGIN") && value.contains("PRIVATE KEY");
    if !looks_inline {
        return (value.to_string(), None);
    }
    let path = std::env::temp_dir().join(format!("hremote-key-{}.pem", hr_core::new_id()));
    let path = path.display().to_string();
    (path.clone(), Some((path, value.to_string())))
}

fn parity_code(parity: SerialParity) -> char {
    match parity {
        SerialParity::None => 'n',
        SerialParity::Odd => 'o',
        SerialParity::Even => 'e',
        SerialParity::Mark => 'm',
        SerialParity::Space => 's',
    }
}

fn flow_code(flow: SerialFlowControl) -> char {
    match flow {
        SerialFlowControl::None => 'N',
        SerialFlowControl::XOnXOff => 'X',
        SerialFlowControl::RequestToSend => 'R',
        SerialFlowControl::RequestToSendXOnXOff => 'R',
    }
}

/// Splits an argument template, honouring double quotes and backslash escapes.
/// A backslash only escapes whitespace, quotes or another backslash so Windows
/// paths (`C:\keys\id_ed25519`) survive intact.
pub fn split_arguments(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' => match characters.peek() {
                Some(' ' | '\t' | '"' | '\\') => escaped = true,
                _ => current.push('\\'),
            },
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !current.is_empty() {
                    result.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn remote_app_rdp(app: &hr_domain::RemoteAppServer) -> AppResult<String> {
    if app.address.address.trim().is_empty() {
        return Err(AppError::validation("the RemoteApp host is required"));
    }
    if app.remote_application_program.trim().is_empty() {
        return Err(AppError::validation("the RemoteApp program is required"));
    }
    let mut file = hr_domain::RdpFile::default();
    file.set(
        "full address",
        "s",
        format!("{}:{}", app.address.address, app.address.port),
    );
    if !app.credentials.username.is_empty() {
        file.set("username", "s", app.credentials.username.clone());
    }
    file.set("remoteapplicationmode", "i", "1");
    file.set(
        "remoteapplicationname",
        "s",
        app.remote_application_name.clone(),
    );
    file.set(
        "remoteapplicationprogram",
        "s",
        app.remote_application_program.clone(),
    );
    file.set("alternate shell", "s", "rdpinit.exe");
    file.set("disableremoteappcapscheck", "i", "1");
    file.set("authentication level", "i", "2");
    file.set("enablecredsspsupport", "i", "1");
    file.set("redirectclipboard", "i", "1");
    file.set("redirectports", "i", "1");
    file.set("redirectsmartcards", "i", "1");
    file.set("autoreconnection enabled", "i", "1");
    file.set(
        "audiomode",
        "i",
        match app.audio_redirection_mode {
            hr_domain::AudioRedirectionMode::RedirectToLocal => "0",
            hr_domain::AudioRedirectionMode::LeaveOnRemote => "1",
            hr_domain::AudioRedirectionMode::Disabled => "2",
        },
    );
    file.merge_freeform(&app.rdp_file_additional_settings);
    Ok(file.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_config::{PUTTY_SERIAL, PUTTY_SSH};
    use hr_domain::{AppArgument, Server};

    #[test]
    fn split_arguments_handles_quotes_and_escapes() {
        assert_eq!(
            split_arguments(r#"-ssh "user name@host" -P 22"#),
            vec!["-ssh", "user name@host", "-P", "22"]
        );
        assert_eq!(split_arguments(r#"a\ b c"#), vec!["a b", "c"]);
    }

    #[test]
    fn ssh_runner_expands_macros_and_password() {
        let mut runners = RunnersConfig::default();
        let mut runner = runners.runner(PUTTY_SSH).unwrap().clone();
        runner.exe_path = "C:\\Program Files\\PuTTY\\putty.exe".into();
        runners.upsert(runner).unwrap();
        let mut server = Server::new_ssh("Box", "10.0.0.5", "root");
        let Server::Ssh(ssh) = &mut server else {
            unreachable!()
        };
        ssh.credentials.password = "hunter2".into();
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        assert_eq!(plan.kind, ProcessKind::Ssh);
        assert_eq!(plan.backend(), SessionBackend::Terminal);
        assert!(plan.arguments.iter().any(|arg| arg == "root@10.0.0.5"));
        assert!(plan.arguments.iter().any(|arg| arg == "hunter2"));
    }

    #[test]
    fn missing_runner_executable_is_reported() {
        let runners = RunnersConfig::default();
        let server = Server::new_ssh("Box", "host", "root");
        let error = ProcessPlan::from_server(&server, &runners).unwrap_err();
        assert!(error.to_string().contains("executable"));
    }

    #[test]
    fn serial_runner_uses_serial_settings() {
        let mut runners = RunnersConfig::default();
        let mut runner = runners.runner(PUTTY_SERIAL).unwrap().clone();
        runner.exe_path = "putty.exe".into();
        runners.upsert(runner).unwrap();
        let mut server = Server::new_serial("Console", "COM3");
        let Server::Serial(serial) = &mut server else {
            unreachable!()
        };
        serial.bit_rate = 115200;
        serial.parity = SerialParity::Even;
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        let joined = plan.arguments.join(" ");
        assert!(joined.contains("COM3"));
        assert!(joined.contains("115200,8,e,1,N"));
    }

    #[test]
    fn local_app_renders_typed_arguments() {
        let runners = RunnersConfig::default();
        let mut server = Server::new_local_app("Editor", "notepad.exe");
        let Server::LocalApp(local) = &mut server else {
            unreachable!()
        };
        local.argument_list = vec![AppArgument::normal("file", "", "{hostname}")];
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        assert_eq!(plan.program, "notepad.exe");
        assert_eq!(plan.kind, ProcessKind::LocalApp);
        assert!(!plan.warnings.is_empty());
    }

    #[test]
    fn inline_private_keys_become_temporary_files() {
        let mut runners = RunnersConfig::default();
        let mut runner = runners.runner(PUTTY_SSH).unwrap().clone();
        runner.exe_path = "putty.exe".into();
        runner.arguments = "-i {privateKey} {user}@{host}".into();
        runners.upsert(runner).unwrap();
        let mut server = Server::new_ssh("Box", "10.0.0.5", "root");
        {
            let Server::Ssh(ssh) = &mut server else {
                unreachable!()
            };
            ssh.credentials.private_key =
                Some("-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----".into());
        }
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        let (path, content) = plan.private_key_file.clone().expect("temp key");
        assert!(path.ends_with(".pem"));
        assert!(content.contains("BEGIN PRIVATE KEY"));
        assert!(plan.arguments.iter().any(|argument| argument == &path));
        assert!(
            !plan
                .arguments
                .iter()
                .any(|argument| argument.contains("BEGIN"))
        );
        // Path-based keys are passed through untouched.
        {
            let Server::Ssh(ssh) = &mut server else {
                unreachable!()
            };
            ssh.credentials.private_key = Some("C:\\keys\\id_ed25519".into());
        }
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        assert!(plan.private_key_file.is_none());
        assert!(
            plan.arguments
                .iter()
                .any(|argument| argument == "C:\\keys\\id_ed25519")
        );
    }

    #[test]
    fn custom_protocols_use_a_named_runner_and_extra_fields() {
        let mut runners = RunnersConfig::default();
        runners
            .upsert(hr_config::Runner {
                id: "duo".into(),
                name: "Duo client".into(),
                protocol: "DUO".into(),
                exe_path: "duo.exe".into(),
                arguments: "-h {host} -p {port} --mode {mode}".into(),
                ..hr_config::Runner::default()
            })
            .unwrap();
        let mut server = Server::new_custom("MFA", "duo");
        let Server::Custom(custom) = &mut server else {
            unreachable!()
        };
        custom.address.address = "duo.example.com".into();
        custom.address.port = 443;
        custom.extra_fields = serde_json::json!({ "mode": "push" });

        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        assert_eq!(plan.kind, ProcessKind::Custom);
        assert_eq!(plan.backend(), SessionBackend::Terminal);
        assert!(plan.arguments.iter().any(|argument| argument == "push"));
        assert_eq!(
            plan.environment.get("CUSTOM_MODE").map(String::as_str),
            Some("push")
        );

        if let Server::Custom(custom) = &mut server {
            custom.custom_protocol_name = "rdp".into();
        }
        let error = ProcessPlan::from_server(&server, &runners).unwrap_err();
        assert!(error.to_string().contains("built-in"));
    }

    #[test]
    fn remote_app_generates_an_rdp_payload() {
        let runners = RunnersConfig::default();
        let mut server = Server::new_remote_app("Published", "10.0.0.8", "calc.exe");
        let Server::RemoteApp(app) = &mut server else {
            unreachable!()
        };
        app.remote_application_name = "Calculator".into();
        let plan = ProcessPlan::from_server(&server, &runners).unwrap();
        assert_eq!(plan.program, "mstsc.exe");
        let (_, content) = plan.temporary_content.unwrap();
        assert!(content.contains("remoteapplicationmode:i:1"));
        assert!(content.contains("remoteapplicationprogram:s:calc.exe"));
        assert!(content.contains("remoteapplicationname:s:Calculator"));
    }
}
