//! Backend-neutral session plans.

use hr_core::{AppError, AppResult};
use hr_domain::credential::SecretString;
use hr_domain::{Server, VncWindowResizeMode};
use serde::{Deserialize, Serialize};

use crate::plan::RdpPlan;
use crate::process::ProcessPlan;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VncPlan {
    pub server_id: String,
    pub server_name: String,
    pub address: String,
    pub port: u16,
    pub password: SecretString,
    pub resize_mode: VncWindowResizeMode,
    pub shared: bool,
}

impl VncPlan {
    pub fn from_server(server: &Server) -> AppResult<Self> {
        let Server::Vnc(vnc) = server else {
            return Err(AppError::validation(
                "only VNC servers can start a VNC session",
            ));
        };
        if vnc.address.address.trim().is_empty() {
            return Err(AppError::validation("the VNC address is required"));
        }
        if vnc.address.port == 0 {
            return Err(AppError::validation("the VNC port is required"));
        }
        Ok(Self {
            server_id: vnc.base.id.clone(),
            server_name: vnc.base.display_name.clone(),
            address: vnc.address.address.clone(),
            port: vnc.address.port,
            password: vnc.credentials.password.clone(),
            resize_mode: vnc.vnc_window_resize_mode,
            shared: true,
        })
    }
}

#[derive(Clone)]
pub enum SessionPlan {
    Rdp(RdpPlan),
    Vnc(VncPlan),
    Process(ProcessPlan),
}

impl std::fmt::Debug for SessionPlan {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rdp(plan) => formatter.debug_tuple("Rdp").field(plan).finish(),
            Self::Vnc(plan) => formatter.debug_tuple("Vnc").field(plan).finish(),
            Self::Process(plan) => formatter
                .debug_tuple("Process")
                .field(&plan.server_name)
                .field(&plan.kind)
                .finish_non_exhaustive(),
        }
    }
}

impl SessionPlan {
    pub fn server_id(&self) -> &str {
        match self {
            Self::Rdp(plan) => &plan.server_id,
            Self::Vnc(plan) => &plan.server_id,
            Self::Process(plan) => &plan.server_id,
        }
    }

    pub fn server_name(&self) -> &str {
        match self {
            Self::Rdp(plan) => &plan.server_name,
            Self::Vnc(plan) => &plan.server_name,
            Self::Process(plan) => &plan.server_name,
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            Self::Rdp(plan) => plan.display.width,
            Self::Vnc(_) => 0,
            Self::Process(_) => 0,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Self::Rdp(plan) => plan.display.height,
            Self::Vnc(_) => 0,
            Self::Process(_) => 0,
        }
    }

    pub fn max_attempts(&self) -> u32 {
        self.reconnect_policy().max_attempts
    }

    pub fn reconnect_policy(&self) -> crate::plan::ReconnectPolicy {
        match self {
            Self::Rdp(plan) => plan.reconnect,
            Self::Vnc(_) => crate::plan::ReconnectPolicy::default(),
            Self::Process(plan) => crate::plan::ReconnectPolicy {
                max_attempts: plan.max_attempts,
                ..crate::plan::ReconnectPolicy::default()
            },
        }
    }

    pub fn backend_hint(&self) -> crate::state::SessionBackend {
        match self {
            Self::Rdp(plan) => match plan.engine {
                crate::plan::EngineChoice::Embedded => crate::state::SessionBackend::IronRdp,
                crate::plan::EngineChoice::ExternalMstsc => crate::state::SessionBackend::Mstsc,
            },
            Self::Vnc(_) => crate::state::SessionBackend::Vnc,
            Self::Process(plan) => plan.backend(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vnc_plan_requires_address_and_port() {
        let mut server = Server::new_vnc("Lab", "10.0.0.4", 5900);
        let plan = VncPlan::from_server(&server).unwrap();
        assert_eq!(plan.address, "10.0.0.4");
        assert_eq!(plan.port, 5900);
        let Server::Vnc(vnc) = &mut server else {
            unreachable!()
        };
        vnc.address.address.clear();
        assert!(VncPlan::from_server(&server).is_err());
    }

    #[test]
    fn non_vnc_servers_are_rejected() {
        let server = Server::new_ssh("Box", "host", "root");
        assert!(VncPlan::from_server(&server).is_err());
    }
}
