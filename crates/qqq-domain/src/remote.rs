//! Linux hosts monitored over SSH: where to connect, how to sign in, and the session that
//! reads a host's devices and readings.

use crate::{Reason, SampleBatch};
use std::sync::{Arc, atomic::AtomicBool};

/// How the user signs in to a remote host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteAuth {
    /// A password from the secret store.
    Password,
    /// A private key file; an encrypted key's passphrase comes from the secret store.
    Key(String),
    /// The keys of the running SSH agent.
    Agent,
}

/// Where and as whom to connect to a remote host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEndpoint {
    /// Stable identity of the host entry; it names the host's secret and devices.
    pub id: String,
    pub address: String,
    pub port: u16,
    pub user: String,
    pub auth: RemoteAuth,
    /// Host key the user trusted, as an OpenSSH public key line such as
    /// `ssh-ed25519 AAAA…`. Without one, the user's `known_hosts` vouches for the host.
    pub host_key: Option<String>,
}

/// A host of the user's SSH client configuration, offered when adding a remote host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemotePreset {
    /// The `Host` alias.
    pub alias: String,
    pub address: String,
    pub port: u16,
    /// Empty when the configuration names no user.
    pub user: String,
    pub key_file: Option<String>,
}

/// Connection to one host; created, used and dropped on that host's thread.
pub trait RemoteSession {
    /// Reads the host's devices and readings once.
    fn collect(&mut self) -> Result<SampleBatch, Reason>;
}

/// Remote access of the platform.
pub trait RemotePlatform: Send + Sync {
    /// Connects and signs in on the calling thread. Setting `cancel` makes this call and
    /// every later call of the session give up promptly.
    fn connect(
        &self,
        endpoint: &RemoteEndpoint,
        cancel: Arc<AtomicBool>,
    ) -> Result<Box<dyn RemoteSession>, Reason>;
    /// Saves, or with `None` removes, the password or key passphrase of host `id`.
    fn set_secret(&self, id: &str, secret: Option<&str>) -> Result<(), Reason>;
    /// Hosts of the user's SSH client configuration.
    fn presets(&self) -> Vec<RemotePreset>;
}

/// Passwords and passphrases kept by the operating system.
pub trait SecretStore: Send + Sync {
    fn secret(&self, name: &str) -> Option<String>;
    /// Saves, or with `None` removes, the secret called `name`.
    fn set_secret(&self, name: &str, secret: Option<&str>) -> Result<(), Reason>;
}

/// ID of a remote host's device: the host's prefix followed by the ID the host's session
/// reports, such as `ssh:web/cpu` or `ssh:web/net:eth0`.
pub fn remote_device(host: &str, device: &str) -> String {
    format!("{}{device}", remote_prefix(host))
}

/// Prefix shared by the IDs of every device of a remote host.
pub fn remote_prefix(host: &str) -> String {
    format!("ssh:{host}/")
}
