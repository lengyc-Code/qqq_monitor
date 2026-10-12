//! Connects to a host the way the monitor does and prints three rounds of readings.
//!
//! ```text
//! cargo run -p qqq-remote --example probe -- ADDRESS USER
//!     [--port 22] [--password PASSWORD | --key FILE [--passphrase TEXT] | --agent] [--trust]
//! ```
//!
//! Without a trusted key the host must be in `known_hosts`; `--trust` accepts the key the
//! host offers after showing its fingerprint.

use qqq_domain::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicBool},
    thread,
    time::Duration,
};

#[derive(Default)]
struct Secrets(Mutex<HashMap<String, String>>);

impl SecretStore for Secrets {
    fn secret(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }
    fn set_secret(&self, name: &str, secret: Option<&str>) -> Result<(), Reason> {
        let mut secrets = self.0.lock().unwrap();
        match secret {
            Some(secret) => secrets.insert(name.into(), secret.into()),
            None => secrets.remove(name),
        };
        Ok(())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let (Some(address), Some(user)) = (args.first(), args.get(1)) else {
        eprintln!("usage: probe ADDRESS USER [--port N] [--password P | --key FILE | --agent]");
        std::process::exit(2);
    };
    let platform = qqq_remote::ssh(Arc::new(Secrets::default()));
    let secret = value("--password").or_else(|| value("--passphrase"));
    platform.set_secret("probe", secret.as_deref()).unwrap();
    let mut endpoint = RemoteEndpoint {
        id: "probe".into(),
        address: address.clone(),
        port: value("--port").map_or(22, |p| p.parse().expect("port number")),
        user: user.clone(),
        auth: match (value("--key"), args.iter().any(|a| a == "--agent")) {
            (Some(file), _) => RemoteAuth::Key(file),
            (None, true) => RemoteAuth::Agent,
            (None, false) => RemoteAuth::Password,
        },
        host_key: None,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let mut session = match platform.connect(&endpoint, Arc::clone(&cancel)) {
        Ok(session) => session,
        Err(Reason::HostKeyUnknown { key, fingerprint }) if args.iter().any(|a| a == "--trust") => {
            println!("trusting host key {fingerprint}");
            endpoint.host_key = Some(key);
            platform
                .connect(&endpoint, cancel)
                .unwrap_or_else(|reason| {
                    eprintln!("connection failed: {reason:?}");
                    std::process::exit(1)
                })
        }
        Err(reason) => {
            eprintln!("connection failed: {reason:?}");
            std::process::exit(1)
        }
    };
    for round in 1..=3 {
        match session.collect() {
            Ok(batch) => {
                println!("--- round {round}");
                for device in &batch.devices {
                    if device.kind != DeviceKind::Core {
                        println!(
                            "device {} {:?} {:?} {:?}",
                            device.id, device.kind, device.name, device.details
                        );
                    }
                }
                for sample in &batch.samples {
                    if !sample.descriptor.key.device.starts_with("core:") {
                        println!(
                            "{} / {}: {:?}",
                            sample.descriptor.key.device,
                            sample.descriptor.key.metric,
                            sample.value
                        );
                    }
                }
            }
            Err(reason) => eprintln!("read failed: {reason:?}"),
        }
        thread::sleep(Duration::from_secs(1));
    }
}
