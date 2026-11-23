//! Link-time registration of the built-in protocols.

use hr_core::{
    ProtocolCapabilities, ProtocolCategory, ProtocolDescriptor, SessionKind, register_protocol,
};

use crate::protocol::ProtocolKind;

#[allow(clippy::too_many_arguments)]
fn descriptor(
    kind: ProtocolKind,
    subtitle_template: &str,
    category: ProtocolCategory,
    list_order: i32,
    session_kind: SessionKind,
    embedded: bool,
    multiple: bool,
    file_transfer: bool,
    private_key: bool,
    help_url: Option<&str>,
) -> ProtocolDescriptor {
    ProtocolDescriptor {
        id: kind.id().to_string(),
        display_name: kind.display_name().to_string(),
        subtitle_template: subtitle_template.to_string(),
        category,
        list_order,
        capabilities: ProtocolCapabilities {
            session_kind,
            supports_embedded_session: embedded,
            supports_multiple_instances: multiple,
            supports_file_transfer: file_transfer,
            supports_private_key: private_key,
            requires_os: None,
        },
        help_url: help_url.map(str::to_string),
    }
}

fn rdp() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Rdp,
        "{address}:{port} ({username})",
        ProtocolCategory::RemoteDesktop,
        10,
        SessionKind::RemoteDesktop,
        true,
        true,
        false,
        false,
        Some("https://learn.microsoft.com/windows-server/remote/remote-desktop-services/"),
    )
}

fn remote_app() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::RemoteApp,
        "{address}:{port} · {application}",
        ProtocolCategory::RemoteDesktop,
        20,
        SessionKind::RemoteDesktop,
        true,
        true,
        false,
        false,
        Some(
            "https://learn.microsoft.com/windows-server/remote/remote-desktop-services/rds-remoteapp",
        ),
    )
}

fn vnc() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Vnc,
        "{address}:{port}",
        ProtocolCategory::RemoteDesktop,
        30,
        SessionKind::Vnc,
        true,
        true,
        false,
        false,
        Some("https://datatracker.ietf.org/doc/html/rfc6143"),
    )
}

fn ssh() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Ssh,
        "{address}:{port} ({username})",
        ProtocolCategory::Terminal,
        40,
        SessionKind::Terminal,
        false,
        true,
        false,
        true,
        Some("https://www.rfc-editor.org/rfc/rfc4251"),
    )
}

fn telnet() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Telnet,
        "{address}:{port}",
        ProtocolCategory::Terminal,
        50,
        SessionKind::Terminal,
        false,
        true,
        false,
        false,
        None,
    )
}

fn serial() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Serial,
        "{serialPort} ({bitRate})",
        ProtocolCategory::Terminal,
        60,
        SessionKind::Terminal,
        false,
        true,
        false,
        false,
        None,
    )
}

fn sftp() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Sftp,
        "{address}:{port} ({username})",
        ProtocolCategory::FileTransfer,
        70,
        SessionKind::FileTransfer,
        false,
        true,
        true,
        true,
        Some("https://datatracker.ietf.org/doc/html/draft-ietf-secsh-filexfer-13"),
    )
}

fn ftp() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Ftp,
        "{address}:{port} ({username})",
        ProtocolCategory::FileTransfer,
        80,
        SessionKind::FileTransfer,
        false,
        true,
        true,
        false,
        Some("https://www.rfc-editor.org/rfc/rfc959"),
    )
}

fn local_app() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::LocalApp,
        "{exePath}",
        ProtocolCategory::Local,
        90,
        SessionKind::LocalProcess,
        false,
        true,
        false,
        false,
        None,
    )
}

fn custom() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Custom,
        "{address}:{port} ({customProtocolName})",
        ProtocolCategory::Utility,
        85,
        SessionKind::Terminal,
        false,
        true,
        false,
        true,
        None,
    )
}

fn dummy() -> ProtocolDescriptor {
    descriptor(
        ProtocolKind::Dummy,
        "",
        ProtocolCategory::Utility,
        i32::MAX,
        SessionKind::Placeholder,
        false,
        true,
        false,
        false,
        None,
    )
}

register_protocol!(rdp);
register_protocol!(remote_app);
register_protocol!(vnc);
register_protocol!(ssh);
register_protocol!(telnet);
register_protocol!(serial);
register_protocol!(sftp);
register_protocol!(ftp);
register_protocol!(local_app);
register_protocol!(custom);
register_protocol!(dummy);

/// Descriptors that should always be present after linking this crate.
pub fn builtin_ids() -> Vec<&'static str> {
    ProtocolKind::builtin_ids().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hr_core::{all_protocols, validate_registry};

    #[test]
    fn all_builtins_are_registered_once() {
        let protocols = all_protocols();
        for id in builtin_ids() {
            assert!(
                protocols.iter().any(|descriptor| descriptor.id == id),
                "missing {id}"
            );
        }
        assert_eq!(validate_registry(), Ok(()));
    }

    #[test]
    fn registry_is_ordered_for_pickers() {
        let ids: Vec<String> = all_protocols()
            .into_iter()
            .map(|descriptor| descriptor.id)
            .collect();
        assert!(ids.iter().position(|id| id == "RDP") < ids.iter().position(|id| id == "VNC"));
        assert!(ids.iter().position(|id| id == "VNC") < ids.iter().position(|id| id == "SSH"));
    }

    #[test]
    fn capabilities_match_protocol_families() {
        let protocols = all_protocols();
        let rdp = protocols.iter().find(|p| p.id == "RDP").unwrap();
        assert!(rdp.capabilities.supports_embedded_session);
        let sftp = protocols.iter().find(|p| p.id == "SFTP").unwrap();
        assert!(sftp.capabilities.supports_file_transfer);
        assert!(sftp.capabilities.supports_private_key);
    }
}
