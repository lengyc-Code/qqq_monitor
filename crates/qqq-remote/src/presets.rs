//! Hosts of the user's OpenSSH client configuration, `~/.ssh/config`, offered when adding
//! a remote host. Only the settings a host entry uses are read: `HostName`, `Port`, `User`
//! and the first `IdentityFile`. As in OpenSSH, the first value that applies to a host
//! wins, so a `Host *` block at the end supplies defaults.

use qqq_domain::RemotePreset;
use std::path::Path;

pub(crate) fn read() -> Vec<RemotePreset> {
    let Some(dirs) = directories::BaseDirs::new() else {
        return Vec::new();
    };
    let home = dirs.home_dir();
    std::fs::read_to_string(home.join(".ssh").join("config"))
        .map(|text| parse(&text, home))
        .unwrap_or_default()
}

struct Block {
    patterns: Vec<String>,
    settings: Vec<(String, String)>,
}

impl Block {
    fn applies(&self, alias: &str) -> bool {
        let mut matched = false;
        for pattern in &self.patterns {
            if let Some(negated) = pattern.strip_prefix('!') {
                if glob(negated, alias) {
                    return false;
                }
            } else if glob(pattern, alias) {
                matched = true;
            }
        }
        matched
    }
}

/// OpenSSH pattern with `*` and `?`, ignoring case.
fn glob(pattern: &str, text: &str) -> bool {
    fn matches(pattern: &[char], text: &[char]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => (0..=text.len()).any(|skip| matches(rest, &text[skip..])),
            Some(('?', rest)) => !text.is_empty() && matches(rest, &text[1..]),
            Some((c, rest)) => text
                .first()
                .is_some_and(|t| t.eq_ignore_ascii_case(c) && matches(rest, &text[1..])),
        }
    }
    let pattern: Vec<_> = pattern.chars().collect();
    let text: Vec<_> = text.chars().collect();
    matches(&pattern, &text)
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

/// A key file path with `~` and `%d` standing for the home directory.
fn expand(path: &str, home: &Path) -> String {
    for prefix in ["~/", "~\\", "%d/", "%d\\"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    path.into()
}

pub(crate) fn parse(text: &str, home: &Path) -> Vec<RemotePreset> {
    // Settings before the first `Host` line apply to every host.
    let mut blocks = vec![Block {
        patterns: vec!["*".into()],
        settings: Vec::new(),
    }];
    let mut aliases: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let split = line
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(line.len());
        let keyword = line[..split].to_ascii_lowercase();
        let value = line[split..].trim_start().trim_start_matches('=').trim();
        match keyword.as_str() {
            "host" => {
                let patterns: Vec<String> = value
                    .split_whitespace()
                    .map(|p| unquote(p).into())
                    .collect();
                for pattern in &patterns {
                    if !pattern.contains(['*', '?', '!']) && !aliases.contains(pattern) {
                        aliases.push(pattern.clone());
                    }
                }
                blocks.push(Block {
                    patterns,
                    settings: Vec::new(),
                });
            }
            // Conditions of `Match` blocks are not evaluated; their settings never apply.
            "match" => blocks.push(Block {
                patterns: Vec::new(),
                settings: Vec::new(),
            }),
            _ => {
                if let Some(block) = blocks.last_mut() {
                    block.settings.push((keyword, unquote(value).into()));
                }
            }
        }
    }
    aliases
        .into_iter()
        .map(|alias| {
            let value = |key: &str| {
                blocks
                    .iter()
                    .filter(|block| block.applies(&alias))
                    .flat_map(|block| &block.settings)
                    .find(|(keyword, _)| keyword == key)
                    .map(|(_, value)| value.clone())
            };
            RemotePreset {
                address: value("hostname")
                    .map(|name| name.replace("%h", &alias))
                    .unwrap_or_else(|| alias.clone()),
                port: value("port").and_then(|p| p.parse().ok()).unwrap_or(22),
                user: value("user").unwrap_or_default(),
                key_file: value("identityfile").map(|file| expand(&file, home)),
                alias,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_value_that_applies_wins_and_patterns_are_not_hosts() {
        let home = Path::new("C:/Users/me");
        let presets = parse(
            "# comment
Host web web-alias
  HostName 10.0.0.5
  Port=2222
  IdentityFile ~/.ssh/web_key
  IdentityFile ~/.ssh/other
Host db
  Hostname \"%h.internal\"
Match user root
  User nobody
Host *.example !skip *
  User ops
  IdentityFile %d/.ssh/id_ed25519
",
            home,
        );
        let names: Vec<_> = presets.iter().map(|p| p.alias.as_str()).collect();
        assert_eq!(names, ["web", "web-alias", "db"]);
        let web = &presets[0];
        assert_eq!(web.address, "10.0.0.5");
        assert_eq!(web.port, 2222);
        assert_eq!(web.user, "ops");
        assert_eq!(
            web.key_file.as_deref(),
            Some(home.join(".ssh/web_key").to_string_lossy().as_ref())
        );
        let db = &presets[2];
        assert_eq!(db.address, "db.internal");
        assert_eq!(db.port, 22);
        assert_eq!(db.user, "ops");
        assert_eq!(
            db.key_file.as_deref(),
            Some(home.join(".ssh/id_ed25519").to_string_lossy().as_ref())
        );
        assert!(glob("*.EXAMPLE", "a.example") && !glob("a?", "a"));
    }
}
