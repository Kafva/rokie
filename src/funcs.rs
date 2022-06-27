use std::io::Read; // Enables the use of .read_exact()
use std::io;
use std::fs::File;
use std::path::Path;
use crate::config::DbType;

fn is_db_with_table(conn: &rusqlite::Connection, table_name: &str) -> bool {
    return conn.query_row::<u32,_,_>(
        &format!("SELECT 1 FROM {table_name} LIMIT 1"),
        [],
        |row|row.get(0)
    ).is_ok();
}

/// Finds all SQLite databases under the given path
/// which feature either a `cookies` or `moz_cookies` table
pub fn is_cookie_db(filepath:&Path) -> Result<DbType,io::Error> {
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
    use crate::is_cookie_db;
    use crate::config::DbType;

    #[test]
    fn test_is_cookie_db() {
        let result = is_cookie_db(Path::new("./moz_cookies.sqlite"));
        assert!( matches!(result.unwrap(), DbType::Firefox) );
    }

}

