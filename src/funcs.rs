use std::io::Read; // Enables the use of .read_exact()
use std::io;
use std::fs::File;
use std::path::Path;
use std::collections::HashSet;
use walkdir::WalkDir;

use sysinfo::{System, SystemExt, RefreshKind};

use crate::types::{DbType,CookieDB};
use crate::config::{SEARCH_DIRS,DB_NAMES};

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
        RefreshKind::everything()
            .without_cpu()
            .without_disks()
            .without_networks()
            .without_memory()
            .without_components()
            .without_users_list()
    );
    let found = sys.processes_by_exact_name(name)
        .find_map(|_| Some(true)).is_some();
    found
}

fn is_db_with_table(conn: &rusqlite::Connection, table_name: &str) -> bool {
    return conn.query_row::<u32,_,_>(
        &format!("SELECT 1 FROM {table_name} LIMIT 1"),
        [],
        |row|row.get(0)
    ).is_ok();
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
        for entry in WalkDir::new(&search_path).follow_links(false)
           .into_iter().filter_map(|e| e.ok()) {
            // The filter is used to skip inaccessible paths
            if entry.file_type().is_file() &&
             DB_NAMES.contains(&entry.file_name().to_string_lossy().as_ref()) {
                let db_type = cookie_db_type(&(entry.path()))
                    .unwrap_or_else(|_| {
                        return DbType::Unknown;
                    });
                if ! matches!(db_type, DbType::Unknown) {
                    cookie_dbs.insert( CookieDB {
                        path: entry.into_path().to_owned(),
                        typing: db_type,
                        cookies: vec![]
                    });
                }
            }
        }
    }
}

/// Finds all SQLite databases under the given path
/// which feature a non-empty `cookies` or `moz_cookies` table
pub fn cookie_db_type(filepath:&Path) -> Result<DbType,io::Error> {
    let mut f = File::open(filepath)?;
    let mut buf = [0; 15];

    f.read_exact(&mut buf)?;

    for (i,j) in buf.iter().zip("SQLite format 3".as_bytes().iter()) {
        if i != j {
            return Ok(DbType::Unknown);
        }
    }

    let r = rusqlite::Connection::open(filepath);
    if r.is_ok() {
        let conn = r.unwrap();

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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use crate::cookie_db_type;
    use crate::types::DbType;

    #[test]
    fn test_is_cookie_db() {
        let result = cookie_db_type(Path::new("./moz_cookies.sqlite"));
        assert!(matches!(result.unwrap(), DbType::Firefox));
    }
}

