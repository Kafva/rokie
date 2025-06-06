use std::io;
use std::{
    collections::HashSet,
    env::consts,
    fs::{File, OpenOptions},
    io::{BufRead, Read, Write},
    path::Path,
    process::{Command, Stdio},
};

use walkdir::WalkDir;

use sysinfo::{ProcessRefreshKind, RefreshKind, System};

use crate::config::{DB_NAMES, SEARCH_DIRS, SQLITE_FILE_ID};
use crate::cookie_db::CookieDB;

/// The PartialEq trait allows us to use `matches!` to check
/// equality between enums
#[derive(Debug, PartialEq)]
pub enum DbType {
    Chrome,
    Firefox,
    Unknown,
}

/// Returns /mnt/c/Users/$USER under WSL, otherwise the value of $HOME
pub fn get_home() -> String {
    if std::fs::metadata("/mnt/c/Users").is_ok() {
        format!("/mnt/c/Users/{}", std::env::var("USER").unwrap())
    } else {
        std::env::var("HOME").unwrap()
    }
}

/// Check if a process is running using the `sysinfo` library
pub fn process_is_running(name: &str) -> bool {
    let sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(
            ProcessRefreshKind::everything()
                .without_cpu()
                .without_disk_usage()
                .without_memory()
                .without_user()
        )
    );
    let found = sys
        .processes_by_exact_name(name.as_ref())
        .find_map(|_| Some(true))
        .is_some();
    found
}

fn is_db_with_table(conn: &rusqlite::Connection, table_name: &str) -> bool {
    return conn
        .query_row::<u32, _, _>(
            &format!("SELECT 1 FROM {table_name} LIMIT 1"),
            [],
            |row| row.get(0),
        )
        .is_ok();
}

/// Search all configured `SEARCH_DIRS` for SQLite databases and
/// add each path to the provided set.
pub fn cookie_dbs_from_profiles(cookie_dbs: &mut HashSet<CookieDB>) {
    let home = get_home();
    for search_dir in SEARCH_DIRS {
        // 'home' needs to be cloned since it is referenced in each iteration
        let search_path: String = format!("{}/{}", home.to_owned(), search_dir);

        // We pass a reference of `search_path` since
        // we want to retain ownership of the variable for later use
        for entry in WalkDir::new(&search_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            // The filter is used to skip inaccessible paths
            if entry.file_type().is_file()
                && DB_NAMES
                    .contains(&entry.file_name().to_string_lossy().as_ref())
            {
                let db_type =
                    cookie_db_type(&(entry.path())).unwrap_or_else(|_| {
                        return DbType::Unknown;
                    });
                if !matches!(db_type, DbType::Unknown) {
                    cookie_dbs.insert(CookieDB {
                        path: entry.into_path().to_owned(),
                        typing: db_type,
                        cookies: vec![],
                    });
                }
            }
        }
    }
}

/// Finds all SQLite databases under the given path
/// which feature a non-empty `cookies` or `moz_cookies` table
pub fn cookie_db_type(filepath: &Path) -> Result<DbType, io::Error> {
    let mut f = File::open(filepath)?;
    let mut buf = [0; 15];
    f.read_exact(&mut buf)?;

    if let Ok(f_header) = String::from_utf8(buf.to_vec()) {
        if f_header != SQLITE_FILE_ID {
            return Ok(DbType::Unknown);
        }
    }

    if let Ok(conn) = rusqlite::Connection::open(filepath) {
        if is_db_with_table(&conn, "moz_cookies") {
            conn.close().unwrap();
            return Ok(DbType::Firefox);
        }
        if is_db_with_table(&conn, "cookies") {
            conn.close().unwrap();
            return Ok(DbType::Chrome);
        } else {
            conn.close().unwrap();
        }
    }

    return Ok(DbType::Unknown);
}

/// Parse the domains from a newline separated whitelist into a vector,
/// skipping lines that start with '#'. Each entry will have explicit
/// quotes surrounding it.
pub fn parse_whitelist(filepath: &Path) -> Result<Vec<String>, io::Error> {
    let f = OpenOptions::new()
        .read(true)
        .open(filepath)
        .expect("Failed to open whitelist");
    let mut reader = io::BufReader::new(f);

    let mut whitelist = vec![];
    let mut line: String = "".to_string();
    while reader.read_line(&mut line)? > 0 {
        // Skip comments
        let trimmed_line = line.trim();
        if !trimmed_line.starts_with("#") && trimmed_line.len() > 0 {
            // Insert explicit qoutes
            whitelist.push(format!("\"{trimmed_line}\""));
        }
        line = "".to_string();
    }
    Ok(whitelist)
}

/// Only applies if `SSH_CONNECTION` is unset.
/// Utilises `xsel` on Linux/BSD.
pub fn copy_to_clipboard(content: String) -> Result<(), io::Error> {
    if std::env::var("SSH_CONNECTION").is_ok() {
        return Ok(());
    }
    match consts::OS {
        "macos" => {
            let mut p = Command::new("/usr/bin/pbcopy")
                .stdin(Stdio::piped())
                .spawn()?;

            p.stdin.as_mut().unwrap().write_all(content.as_bytes())
        }
        "linux" | "freebsd" => {
            if std::env::var("DISPLAY").is_ok() {
                let mut p = Command::new("xsel")
                    .args(["-i", "-b"])
                    .stdin(Stdio::piped())
                    .spawn()?;

                p.stdin.as_mut().unwrap().write_all(content.as_bytes())
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use crate::util::{cookie_db_type, DbType};
    use std::path::Path;

    #[test]
    fn test_is_cookie_db() {
        if Path::new("moz_cookies.sqlite").exists() {
            let result = cookie_db_type(Path::new("moz_cookies.sqlite"));
            assert!(matches!(result.unwrap(), DbType::Firefox));
        }
    }
}
