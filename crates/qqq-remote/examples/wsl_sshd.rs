//! An SSH server for trying remote monitoring on this computer. Each command a client
//! executes runs in a WSL distribution through `wsl.exe`, so the client reads a real
//! Linux `/proc`. For development only: it listens on 127.0.0.1.
//!
//! ```text
//! cargo run -p qqq-remote --example wsl_sshd -- --host-key KEY --password PASSWORD
//!     [--authorized KEY.pub] [--port 2222] [--distro Ubuntu]
//! ```
//!
//! `--host-key` names an OpenSSH private key file, such as one from
//! `ssh-keygen -t ed25519 -N "" -f KEY`; `--authorized` names a file of public keys, one
//! per line, that may sign in.

use russh::{
    Channel, ChannelId,
    keys::{self, PublicKey},
    server::{self, Auth, Msg, Server as _, Session},
};
use std::{collections::HashMap, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{ChildStdin, Command},
};

struct Options {
    password: String,
    authorized: Vec<PublicKey>,
    distro: String,
}

#[derive(Clone)]
struct Server {
    options: Arc<Options>,
}

impl server::Server for Server {
    type Handler = Client;
    fn new_client(&mut self, peer: Option<std::net::SocketAddr>) -> Client {
        println!("connection from {peer:?}");
        Client {
            options: Arc::clone(&self.options),
            inputs: HashMap::new(),
        }
    }
}

struct Client {
    options: Arc<Options>,
    inputs: HashMap<ChannelId, ChildStdin>,
}

impl server::Handler for Client {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        let accepted = password == self.options.password;
        println!(
            "password for {user}: {}",
            if accepted { "accepted" } else { "rejected" }
        );
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn auth_publickey(&mut self, user: &str, key: &PublicKey) -> Result<Auth, Self::Error> {
        let accepted = self
            .options
            .authorized
            .iter()
            .any(|authorized| authorized.key_data() == key.key_data());
        println!(
            "key for {user}: {}",
            if accepted { "accepted" } else { "rejected" }
        );
        Ok(if accepted {
            Auth::Accept
        } else {
            Auth::reject()
        })
    }

    async fn channel_open_session(
        &mut self,
        _: Channel<Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let command = String::from_utf8_lossy(data).into_owned();
        println!("exec {command:?}");
        let child = Command::new("wsl.exe")
            .args([
                "-d",
                &self.options.distro,
                "--cd",
                "/",
                "-e",
                "sh",
                "-c",
                &command,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn();
        let Ok(mut child) = child else {
            session.channel_failure(channel)?;
            return Ok(());
        };
        session.channel_success(channel)?;
        if let Some(stdin) = child.stdin.take() {
            self.inputs.insert(channel, stdin);
        }
        let mut stdout = child.stdout.take().expect("piped output");
        let handle = session.handle();
        tokio::spawn(async move {
            let mut buffer = vec![0; 32 * 1024];
            while let Ok(read) = stdout.read(&mut buffer).await
                && read > 0
            {
                if handle.data(channel, buffer[..read].to_vec()).await.is_err() {
                    break;
                }
            }
            let code = child.wait().await.ok().and_then(|s| s.code()).unwrap_or(1);
            let _ = handle.exit_status_request(channel, code as u32).await;
            let _ = handle.eof(channel).await;
            let _ = handle.close(channel).await;
            println!("exec ended with {code}");
        });
        Ok(())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(input) = self.inputs.get_mut(&channel) {
            let _ = input.write_all(data).await;
            let _ = input.flush().await;
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.inputs.remove(&channel);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let host_key = value("--host-key").expect("--host-key KEY");
    let options = Options {
        password: value("--password").expect("--password PASSWORD"),
        authorized: value("--authorized")
            .map(|path| std::fs::read_to_string(path).expect("readable public keys"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| PublicKey::from_openssh(line).ok())
            .collect(),
        distro: value("--distro").unwrap_or_else(|| "Ubuntu".into()),
    };
    let port: u16 = value("--port").map_or(2222, |p| p.parse().expect("port number"));
    let config = Arc::new(server::Config {
        keys: vec![keys::load_secret_key(host_key, None).expect("readable host key")],
        auth_rejection_time: std::time::Duration::from_millis(200),
        auth_rejection_time_initial: Some(std::time::Duration::ZERO),
        ..Default::default()
    });
    println!(
        "listening on 127.0.0.1:{port}, commands run in WSL {}",
        options.distro
    );
    let mut server = Server {
        options: Arc::new(options),
    };
    server
        .run_on_address(config, ("127.0.0.1", port))
        .await
        .expect("server runs");
}
