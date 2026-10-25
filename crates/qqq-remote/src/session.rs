//! One SSH connection to a host. A single-threaded tokio runtime, owned by the session and
//! driven only while the host's thread waits on it, carries the connection; between reads
//! the connection rests. The session signs in, checks the host key, starts `sh -s` with
//! the program of [`linux`](crate::linux) and asks it for readings over that one channel.

use crate::linux::{self, END, READY, SCRIPT, Sampler};
use qqq_domain::*;
use russh::{
    ChannelMsg, Disconnect, MethodKind,
    client::{self, AuthResult, KeyboardInteractiveAuthResponse},
    keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate},
};
use std::{
    future::Future,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Longest wait to connect, sign in and start the shell.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Longest wait for one answer of the shell.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Largest answer the session accepts.
const ANSWER_LIMIT: usize = 8 * 1024 * 1024;

/// Name of the password or passphrase of host `id` in the secret store.
pub fn secret_name(id: &str) -> String {
    format!("qqq_monitor/ssh/{id}")
}

/// Runs `work` until it finishes, `limit` passes or `cancel` is set.
async fn bounded<T>(
    cancel: &AtomicBool,
    limit: Duration,
    work: impl Future<Output = Result<T, Reason>>,
) -> Result<T, Reason> {
    let cancelled = async {
        while !cancel.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    tokio::select! {
        result = tokio::time::timeout(limit, work) => result.unwrap_or(Err(Reason::TimedOut)),
        () = cancelled => Err(Reason::ConnectionLost),
    }
}

/// Reason for an I/O failure while connecting.
fn io_reason(error: &std::io::Error) -> Reason {
    use std::io::ErrorKind::*;
    // WSAHOST_NOT_FOUND, WSATRY_AGAIN, WSANO_RECOVERY and WSANO_DATA from name lookup.
    if error
        .raw_os_error()
        .is_some_and(|code| (11001..=11004).contains(&code))
    {
        return Reason::AddressNotFound;
    }
    match error.kind() {
        ConnectionRefused => Reason::ConnectionRefused,
        TimedOut => Reason::TimedOut,
        ConnectionReset | ConnectionAborted | UnexpectedEof | BrokenPipe | NotConnected => {
            Reason::ConnectionLost
        }
        HostUnreachable | NetworkUnreachable | AddrNotAvailable => Reason::TimedOut,
        _ => Reason::CallFailed {
            api: "connect",
            code: error.raw_os_error().unwrap_or(0) as u32,
        },
    }
}

fn ssh_reason(error: russh::Error) -> Reason {
    use russh::Error::*;
    match error {
        IO(error) => io_reason(&error),
        Elapsed(_) | ConnectionTimeout => Reason::TimedOut,
        HUP
        | Disconnect
        | RecvError
        | SendError
        | KeepaliveTimeout
        | InactivityTimeout
        | WrongChannel
        | ChannelOpenFailure(_) => Reason::ConnectionLost,
        _ => Reason::Handshake,
    }
}

/// Reason for a failure while signing in. A host that closes the connection during sign-in
/// has usually refused too many attempts, such as the keys of an agent one after another,
/// so the failure waits for the user like a rejected sign-in instead of trying again.
fn sign_in_reason(error: russh::Error) -> Reason {
    match ssh_reason(error) {
        Reason::TimedOut => Reason::TimedOut,
        _ => Reason::AuthFailed,
    }
}

/// Checks the host key: against the trusted key when the user trusted one, otherwise
/// against the user's `known_hosts`. A rejected key's reason stays in `verdict`.
struct Client {
    address: String,
    port: u16,
    trusted: Option<String>,
    verdict: Arc<Mutex<Option<Reason>>>,
}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        offered: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let key = match offered {
            PublicKeyOrCertificate::PublicKey { key, .. } => key.clone(),
            PublicKeyOrCertificate::Certificate(certificate) => {
                PublicKey::from(certificate.public_key().clone())
            }
        };
        let line = key.to_openssh().unwrap_or_default();
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let verdict = match &self.trusted {
            Some(trusted) => match PublicKey::from_openssh(trusted) {
                Ok(trusted) if trusted.key_data() == key.key_data() => return Ok(true),
                _ => Reason::HostKeyChanged {
                    key: line,
                    fingerprint,
                },
            },
            None => match keys::check_known_hosts(&self.address, self.port, &key) {
                Ok(true) => return Ok(true),
                Err(keys::Error::KeyChanged { .. }) => Reason::HostKeyChanged {
                    key: line,
                    fingerprint,
                },
                _ => Reason::HostKeyUnknown {
                    key: line,
                    fingerprint,
                },
            },
        };
        *self.verdict.lock().unwrap() = Some(verdict);
        Ok(false)
    }
}

/// What the session signs in with, read before connecting so that a missing key or
/// password costs no connection.
enum Credentials {
    Password(String),
    Key(Box<keys::PrivateKey>),
    Agent,
}

fn credentials(
    endpoint: &RemoteEndpoint,
    secrets: &dyn SecretStore,
) -> Result<Credentials, Reason> {
    let secret = || secrets.secret(&secret_name(&endpoint.id));
    Ok(match &endpoint.auth {
        RemoteAuth::Password => Credentials::Password(secret().ok_or(Reason::NoPassword)?),
        RemoteAuth::Key(path) => {
            let path = key_path(path);
            let passphrase = secret();
            match keys::load_secret_key(&path, passphrase.as_deref()) {
                Ok(key) => Credentials::Key(Box::new(key)),
                Err(keys::Error::IO(_)) => return Err(Reason::KeyUnreadable),
                Err(keys::Error::KeyIsEncrypted) => return Err(Reason::KeyPassphrase),
                Err(_) if passphrase.is_some() => return Err(Reason::KeyPassphrase),
                Err(_) => return Err(Reason::KeyUnreadable),
            }
        }
        RemoteAuth::Agent => Credentials::Agent,
    })
}

/// A key file path with a leading `~` standing for the home directory.
fn key_path(path: &str) -> PathBuf {
    let path = path.trim();
    for prefix in ["~/", "~\\"] {
        if let Some(rest) = path.strip_prefix(prefix)
            && let Some(dirs) = directories::BaseDirs::new()
        {
            return dirs.home_dir().join(rest);
        }
    }
    PathBuf::from(path)
}

async fn authenticate(
    handle: &mut client::Handle<Client>,
    user: &str,
    credentials: Credentials,
) -> Result<(), Reason> {
    let rsa_hash = || async {
        handle
            .best_supported_rsa_hash()
            .await
            .ok()
            .flatten()
            .flatten()
    };
    let accepted = match credentials {
        Credentials::Password(password) => {
            match handle
                .authenticate_password(user, password.clone())
                .await
                .map_err(sign_in_reason)?
            {
                AuthResult::Success => true,
                // Hosts that sign in through PAM often ask for the password this way.
                AuthResult::Failure {
                    remaining_methods, ..
                } if remaining_methods.contains(&MethodKind::KeyboardInteractive) => {
                    keyboard_interactive(handle, user, &password).await?
                }
                AuthResult::Failure { .. } => false,
            }
        }
        Credentials::Key(key) => {
            let hash = rsa_hash().await;
            handle
                .authenticate_publickey(user, PrivateKeyWithHashAlg::new(Arc::new(*key), hash))
                .await
                .map_err(sign_in_reason)?
                .success()
        }
        Credentials::Agent => agent(handle, user).await?,
    };
    if accepted {
        Ok(())
    } else {
        Err(Reason::AuthFailed)
    }
}

/// Answers every hidden prompt with the password.
async fn keyboard_interactive(
    handle: &mut client::Handle<Client>,
    user: &str,
    password: &str,
) -> Result<bool, Reason> {
    let mut response = handle
        .authenticate_keyboard_interactive_start(user, None::<String>)
        .await
        .map_err(sign_in_reason)?;
    for _ in 0..4 {
        match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let answers = prompts
                    .iter()
                    .map(|p| {
                        if p.echo {
                            String::new()
                        } else {
                            password.into()
                        }
                    })
                    .collect();
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await
                    .map_err(sign_in_reason)?;
            }
        }
    }
    Ok(false)
}

/// Tries each key of the running agent: the OpenSSH agent service, then Pageant.
#[cfg(windows)]
async fn agent(handle: &mut client::Handle<Client>, user: &str) -> Result<bool, Reason> {
    match keys::agent::client::AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await
    {
        Ok(mut agent) => sign_with_agent(handle, user, &mut agent).await,
        Err(_) => match keys::agent::client::AgentClient::connect_pageant().await {
            Ok(mut agent) => sign_with_agent(handle, user, &mut agent).await,
            Err(_) => Err(Reason::NoAgent),
        },
    }
}

#[cfg(not(windows))]
async fn agent(handle: &mut client::Handle<Client>, user: &str) -> Result<bool, Reason> {
    let mut agent = keys::agent::client::AgentClient::connect_env()
        .await
        .map_err(|_| Reason::NoAgent)?;
    sign_with_agent(handle, user, &mut agent).await
}

async fn sign_with_agent<S>(
    handle: &mut client::Handle<Client>,
    user: &str,
    agent: &mut keys::agent::client::AgentClient<S>,
) -> Result<bool, Reason>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
{
    let identities = agent
        .request_identities()
        .await
        .map_err(|_| Reason::NoAgent)?;
    let hash = handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten();
    let mut tried = false;
    for identity in identities {
        let keys::agent::AgentIdentity::PublicKey { key, .. } = identity else {
            continue;
        };
        tried = true;
        // The host closing the connection after too many keys, and the agent declining to
        // sign, both wait for the user.
        if handle
            .authenticate_publickey_with(user, key, hash, agent)
            .await
            .map_err(|_| Reason::AuthFailed)?
            .success()
        {
            return Ok(true);
        }
    }
    if tried {
        Ok(false)
    } else {
        Err(Reason::NoAgent)
    }
}

/// The `sh -s` that answers the session's requests.
struct Shell {
    channel: russh::Channel<client::Msg>,
    pending: Vec<u8>,
}

impl Shell {
    async fn start(handle: &client::Handle<Client>) -> Result<Self, Reason> {
        let channel = handle.channel_open_session().await.map_err(ssh_reason)?;
        channel.exec(true, "sh -s").await.map_err(ssh_reason)?;
        channel
            .data_bytes(SCRIPT.as_bytes())
            .await
            .map_err(ssh_reason)?;
        let mut shell = Self {
            channel,
            pending: Vec::new(),
        };
        // A host without a POSIX shell, such as one that only allows file transfers,
        // closes the channel instead.
        shell
            .read_through(READY)
            .await
            .map_err(|reason| match reason {
                Reason::ConnectionLost => Reason::NotLinux,
                reason => reason,
            })?;
        Ok(shell)
    }

    async fn request(&mut self, command: &str) -> Result<String, Reason> {
        self.channel
            .data_bytes(format!("{command}\n").into_bytes())
            .await
            .map_err(ssh_reason)?;
        self.read_through(END).await
    }

    /// Output up to the line `marker`, which is consumed.
    async fn read_through(&mut self, marker: &str) -> Result<String, Reason> {
        loop {
            if let Some(text) = take_through(&mut self.pending, marker) {
                return Ok(text);
            }
            match self.channel.wait().await {
                Some(ChannelMsg::Data { data }) => self.pending.extend_from_slice(&data),
                Some(ChannelMsg::Failure | ChannelMsg::Eof | ChannelMsg::Close) | None => {
                    return Err(Reason::ConnectionLost);
                }
                Some(_) => {}
            }
            if self.pending.len() > ANSWER_LIMIT {
                return Err(Reason::BadData { api: "sh" });
            }
        }
    }
}

/// Removes the output through the line `marker` from `pending` and returns the output
/// before it.
fn take_through(pending: &mut Vec<u8>, marker: &str) -> Option<String> {
    let line = format!("{marker}\n");
    let at = if pending.starts_with(line.as_bytes()) {
        0
    } else {
        pending
            .windows(line.len() + 1)
            .position(|w| w[0] == b'\n' && &w[1..] == line.as_bytes())?
            + 1
    };
    let text = String::from_utf8_lossy(&pending[..at]).into_owned();
    pending.drain(..at + line.len());
    Some(text)
}

pub(crate) struct Session {
    cancel: Arc<AtomicBool>,
    handle: client::Handle<Client>,
    shell: Shell,
    sampler: Sampler,
    /// Dropped last, after the connection it carries.
    runtime: tokio::runtime::Runtime,
}

impl Session {
    pub(crate) fn connect(
        endpoint: &RemoteEndpoint,
        secrets: &dyn SecretStore,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, Reason> {
        let credentials = credentials(endpoint, secrets)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Reason::CallFailed {
                api: "tokio",
                code: error.raw_os_error().unwrap_or(0) as u32,
            })?;
        let (handle, shell, sampler) =
            runtime.block_on(bounded(&cancel, CONNECT_TIMEOUT, async {
                let verdict = Arc::new(Mutex::new(None));
                let client = Client {
                    address: endpoint.address.clone(),
                    port: endpoint.port,
                    trusted: endpoint.host_key.clone(),
                    verdict: Arc::clone(&verdict),
                };
                let config = Arc::new(client::Config {
                    nodelay: true,
                    ..Default::default()
                });
                let address = endpoint.address.trim_matches(['[', ']']);
                let mut handle = client::connect(config, (address, endpoint.port), client)
                    .await
                    .map_err(|error| {
                        verdict
                            .lock()
                            .unwrap()
                            .take()
                            .unwrap_or_else(|| ssh_reason(error))
                    })?;
                authenticate(&mut handle, &endpoint.user, credentials).await?;
                let mut shell = Shell::start(&handle).await?;
                let info = linux::parse_info(&shell.request("i").await?);
                Ok((handle, shell, Sampler::new(info)))
            }))?;
        Ok(Self {
            cancel,
            handle,
            shell,
            sampler,
            runtime,
        })
    }
}

impl RemoteSession for Session {
    fn collect(&mut self) -> Result<SampleBatch, Reason> {
        let started_at = Instant::now();
        let text = self.runtime.block_on(bounded(
            &self.cancel,
            REQUEST_TIMEOUT,
            self.shell.request("s"),
        ))?;
        let raw = linux::parse_sample(&text)?;
        Ok(self.sampler.batch(raw, started_at, Instant::now()))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Says goodbye so the host logs a normal disconnect; a broken connection gives up
        // at once. The timer needs the runtime, so it starts inside it.
        let handle = &self.handle;
        self.runtime.block_on(async {
            let goodbye = handle.disconnect(Disconnect::ByApplication, "", "en");
            let _ = tokio::time::timeout(Duration::from_millis(300), goodbye).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_end_at_their_marker_line() {
        let mut pending = b"@@ ready\n@@ info\nhostname a\n@@ end\n@@ uptime\n1.0".to_vec();
        assert_eq!(take_through(&mut pending, READY).unwrap(), "");
        assert_eq!(
            take_through(&mut pending, END).unwrap(),
            "@@ info\nhostname a\n"
        );
        assert!(take_through(&mut pending, END).is_none());
        pending.extend_from_slice(b" 2.0\n@@ endless\n@@ end\n");
        assert_eq!(
            take_through(&mut pending, END).unwrap(),
            "@@ uptime\n1.0 2.0\n@@ endless\n"
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn a_trusted_key_must_match_and_an_untrusted_one_names_its_fingerprint() {
        use client::Handler;
        const OFFERED: &str =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJSUjfvtse2Qgnp4qd97Vvp9XhQLu1crFaCLkGv5hXDt";
        const OTHER: &str =
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC+bsggLr0XiU9V3l+efLXjwl10l5kOi/SlrNTyohouR";
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let offered = PublicKeyOrCertificate::PublicKey {
            key: PublicKey::from_openssh(OFFERED).unwrap(),
            hash_alg: None,
        };
        let check = |trusted: Option<&str>| {
            let verdict = Arc::new(Mutex::new(None));
            let mut client = Client {
                // Reserved for documentation, so no known_hosts names it.
                address: "192.0.2.1".into(),
                port: 2222,
                trusted: trusted.map(Into::into),
                verdict: Arc::clone(&verdict),
            };
            let accepted = runtime.block_on(client.check_server_key(&offered)).unwrap();
            (accepted, verdict.lock().unwrap().take())
        };
        assert_eq!(check(Some(&format!("{OFFERED} comment"))), (true, None));
        let fingerprint = "SHA256:X3tcp1w2wbEki1ZddXSK4JGse91J/QkmlnRKBTtUY9A".to_string();
        assert_eq!(
            check(Some(OTHER)),
            (
                false,
                Some(Reason::HostKeyChanged {
                    key: OFFERED.into(),
                    fingerprint: fingerprint.clone()
                })
            )
        );
        assert_eq!(
            check(None),
            (
                false,
                Some(Reason::HostKeyUnknown {
                    key: OFFERED.into(),
                    fingerprint
                })
            )
        );
    }

    #[test]
    fn a_connection_closed_during_sign_in_waits_for_the_user() {
        assert_eq!(sign_in_reason(russh::Error::Disconnect), Reason::AuthFailed);
        assert_eq!(sign_in_reason(russh::Error::SendError), Reason::AuthFailed);
        assert_eq!(sign_in_reason(russh::Error::HUP), Reason::AuthFailed);
        assert_eq!(
            sign_in_reason(russh::Error::ConnectionTimeout),
            Reason::TimedOut
        );
    }

    #[test]
    fn name_lookup_and_refusal_have_their_own_reasons() {
        let lookup = std::io::Error::from_raw_os_error(11001);
        assert_eq!(io_reason(&lookup), Reason::AddressNotFound);
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert_eq!(io_reason(&refused), Reason::ConnectionRefused);
    }
}
