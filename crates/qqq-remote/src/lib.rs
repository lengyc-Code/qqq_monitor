//! Linux hosts monitored over SSH. A session signs in with a password, a private key or
//! the SSH agent, checks the host key against the trusted key or the user's
//! `known_hosts`, then keeps one `sh` running on the host that answers each read with
//! the counters of `/proc` and the local file systems.

#![forbid(unsafe_code)]

mod linux;
mod presets;
mod session;

use qqq_domain::*;
use std::sync::{Arc, atomic::AtomicBool};

pub use session::secret_name;

struct Ssh {
    secrets: Arc<dyn SecretStore>,
}

impl RemotePlatform for Ssh {
    fn connect(
        &self,
        endpoint: &RemoteEndpoint,
        cancel: Arc<AtomicBool>,
    ) -> Result<Box<dyn RemoteSession>, Reason> {
        Ok(Box::new(session::Session::connect(
            endpoint,
            &*self.secrets,
            cancel,
        )?))
    }
    fn set_secret(&self, id: &str, secret: Option<&str>) -> Result<(), Reason> {
        self.secrets.set_secret(&secret_name(id), secret)
    }
    fn presets(&self) -> Vec<RemotePreset> {
        presets::read()
    }
}

/// Remote access over SSH; passwords and passphrases come from `secrets`.
pub fn ssh(secrets: Arc<dyn SecretStore>) -> Arc<dyn RemotePlatform> {
    Arc::new(Ssh { secrets })
}
