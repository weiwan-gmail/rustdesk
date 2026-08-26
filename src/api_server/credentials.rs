//! Credential lookup for headless OS / RustDesk passwords.
//!
//! Phase 1: CSV backend. Later backends (SQLite / Postgres) should implement
//! [`CredentialStore`] without changing API call sites.

use hbb_common::log;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};

#[derive(Clone, Debug, Default)]
pub struct OsCredential {
    pub peer_id: String,
    pub ip: String,
    pub os_username: String,
    pub os_password: String,
    pub rustdesk_password: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct CredentialPublicInfo {
    pub peer_id: String,
    pub ip: String,
    pub os_username: String,
    pub has_os_password: bool,
    pub has_rustdesk_password: bool,
}

pub trait CredentialStore: Send + Sync {
    fn lookup(&self, peer_id: &str, ip: Option<&str>) -> Option<OsCredential>;
    fn list_public(&self) -> Vec<CredentialPublicInfo>;
    fn reload(&self) -> Result<(), String>;
}

#[derive(Default)]
pub struct CsvCredentialStore {
    path: PathBuf,
    rows: RwLock<Vec<OsCredential>>,
}

impl CsvCredentialStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let store = Self {
            path,
            rows: RwLock::new(Vec::new()),
        };
        if let Err(e) = store.reload() {
            log::warn!("credentials csv load skipped: {e}");
        }
        store
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn parse_csv(text: &str) -> Result<Vec<OsCredential>, String> {
        let mut lines = text.lines().filter(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        });
        let header = lines
            .next()
            .ok_or_else(|| "csv is empty".to_string())?
            .to_lowercase();
        let cols: Vec<&str> = header.split(',').map(|s| s.trim()).collect();
        let idx = |name: &str| cols.iter().position(|c| *c == name);
        let i_peer = idx("peer_id");
        let i_ip = idx("ip");
        let i_user = idx("os_username");
        let i_osp = idx("os_password");
        let i_rdp = idx("rustdesk_password");
        if i_peer.is_none() && i_ip.is_none() {
            return Err("csv requires peer_id or ip column".to_string());
        }
        let get = |parts: &[&str], i: Option<usize>| -> String {
            i.and_then(|n| parts.get(n).copied())
                .unwrap_or("")
                .trim()
                .trim_matches('"')
                .to_string()
        };
        let mut out = Vec::new();
        for line in lines {
            let parts: Vec<&str> = line.split(',').collect();
            let row = OsCredential {
                peer_id: get(&parts, i_peer),
                ip: get(&parts, i_ip),
                os_username: get(&parts, i_user),
                os_password: get(&parts, i_osp),
                rustdesk_password: get(&parts, i_rdp),
            };
            if row.peer_id.is_empty() && row.ip.is_empty() {
                continue;
            }
            out.push(row);
        }
        Ok(out)
    }
}

impl CredentialStore for CsvCredentialStore {
    fn lookup(&self, peer_id: &str, ip: Option<&str>) -> Option<OsCredential> {
        let rows = self.rows.read().ok()?;
        let peer = peer_id.trim();
        if !peer.is_empty() {
            if let Some(r) = rows.iter().find(|r| !r.peer_id.is_empty() && r.peer_id == peer) {
                return Some(r.clone());
            }
        }
        let ip_key = ip
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                if looks_like_ip(peer) {
                    Some(peer.to_string())
                } else {
                    None
                }
            });
        if let Some(ip_key) = ip_key {
            if let Some(r) = rows.iter().find(|r| !r.ip.is_empty() && r.ip == ip_key) {
                return Some(r.clone());
            }
        }
        None
    }

    fn list_public(&self) -> Vec<CredentialPublicInfo> {
        self.rows
            .read()
            .map(|rows| {
                rows.iter()
                    .map(|r| CredentialPublicInfo {
                        peer_id: r.peer_id.clone(),
                        ip: r.ip.clone(),
                        os_username: r.os_username.clone(),
                        has_os_password: !r.os_password.is_empty(),
                        has_rustdesk_password: !r.rustdesk_password.is_empty(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn reload(&self) -> Result<(), String> {
        if !self.path.exists() {
            *self.rows.write().map_err(|e| e.to_string())? = Vec::new();
            return Ok(());
        }
        let text = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        let rows = Self::parse_csv(&text)?;
        log::info!(
            "loaded {} credential row(s) from {}",
            rows.len(),
            self.path.display()
        );
        *self.rows.write().map_err(|e| e.to_string())? = rows;
        Ok(())
    }
}

fn looks_like_ip(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u8>().is_ok())
}

/// In-memory store for tests / empty default.
#[derive(Default)]
pub struct EmptyCredentialStore;

impl CredentialStore for EmptyCredentialStore {
    fn lookup(&self, _peer_id: &str, _ip: Option<&str>) -> Option<OsCredential> {
        None
    }
    fn list_public(&self) -> Vec<CredentialPublicInfo> {
        Vec::new()
    }
    fn reload(&self) -> Result<(), String> {
        Ok(())
    }
}

pub type SharedCredentialStore = std::sync::Arc<dyn CredentialStore>;

pub fn open_csv(path: Option<String>) -> SharedCredentialStore {
    match path {
        Some(p) if !p.is_empty() => std::sync::Arc::new(CsvCredentialStore::new(p)),
        _ => std::sync::Arc::new(EmptyCredentialStore),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn csv_lookup_by_peer_and_ip() {
        let dir = std::env::temp_dir().join(format!("rd-creds-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("c.csv");
        let mut f = fs::File::create(&path).unwrap();
        writeln!(
            f,
            "peer_id,ip,os_username,os_password,rustdesk_password\n\
             111,,Admin,p1,rd1\n\
             ,10.0.0.2,user2,p2,"
        )
        .unwrap();
        let store = CsvCredentialStore::new(&path);
        let a = store.lookup("111", None).unwrap();
        assert_eq!(a.os_username, "Admin");
        assert_eq!(a.rustdesk_password, "rd1");
        let b = store.lookup("10.0.0.2", None).unwrap();
        assert_eq!(b.os_username, "user2");
        assert!(store.lookup("missing", None).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
