use crate::config::COOKIE_FIELDS;
use crate::types::{DbType,CookieDB,Cookie};
use crate::funcs::get_home;

impl CookieDB {
    /// Return the parent of the current path and replaces $HOME with "~".
    /// Returns `path` as is if it is not an absolute path.
    pub fn path_short(&self) -> String {
        if self.path.has_root() {
            self.path.parent().unwrap().to_string_lossy()
                .replace(&get_home(),"~")
        } else {
            self.path.to_string_lossy().to_string()
        }
    }

    /// Fetch the name of the cookies table depending on
    /// the browser type.
    fn table_name(&self) -> &'static str {
        if self.typing == DbType::Firefox {
            "moz_cookies"
        } else {
            "cookies"
        }
    }

    /// Timestamps are stored internally as UNIX epoch microseconds
    /// for Firefox and as microseconds since Jan 01 1601 in Chrome
    ///
    /// Cookies with a Session-only lifetime will have 0 as their
    /// expiry date in Chrome
    fn get_unix_epoch(self: &Self, timestamp:i64) -> i64 {
        if timestamp == 0 {
            0
        } else if self.typing == DbType::Firefox {
            timestamp/1_000_000
        } else {
            (timestamp/1_000_000) - 11_644_473_600
        }
    }

    /// Load all cookies from the current `path` into the `cookies` vector
    pub fn load_cookies(&mut self) -> Result<(), rusqlite::Error> {
        let conn = rusqlite::Connection::open(&self.path)?;
        let field_idx = if self.typing==DbType::Chrome {0} else {1};

        let query = format!(
            "SELECT {},{},{},{},{},{},{},{},{},{} FROM {};",
            COOKIE_FIELDS["Host"][field_idx],
            COOKIE_FIELDS["Name"][field_idx],
            COOKIE_FIELDS["Value"][field_idx],
            COOKIE_FIELDS["Path"][field_idx],
            COOKIE_FIELDS["Creation"][field_idx],
            COOKIE_FIELDS["Expiry"][field_idx],
            COOKIE_FIELDS["LastAccess"][field_idx],
            COOKIE_FIELDS["HttpOnly"][field_idx],
            COOKIE_FIELDS["Secure"][field_idx],
            COOKIE_FIELDS["SameSite"][field_idx],
            self.table_name()
        );
        let mut stmt = conn.prepare(&query)?;
        let results_iter = stmt.query_map([], |row| {
            // The second parameter to get() denotes
            // the underlying type that the fetched field is expected to have
            Ok(
                Cookie {
                    host: row.get::<_,String>(0)?,
                    name: row.get::<_,String>(1)?,
                    value: row.get::<_,String>(2)?,
                    path: row.get::<_,String>(3)?,
                    creation: self.get_unix_epoch(
                        row.get::<_,i64>(4)?
                    ),
                    expiry: self.get_unix_epoch(
                        row.get::<_,i64>(5)?
                    ),
                    last_access: self.get_unix_epoch(
                        row.get::<_,i64>(6)?
                    ),
                    http_only: row.get::<_,bool>(7)?,
                    secure: row.get::<_,bool>(8)?,
                    samesite: row.get::<_,i32>(9)?
                }
            )
        })?;

        // The query_map() call returns an iterator
        // of results, Ok(), which we need to unwrap
        // before calling collect
        self.cookies = results_iter.filter_map(|r| r.ok() ).collect();

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use crate::path::PathBuf;
    use crate::types::{DbType,CookieDB};
    use crate::funcs::get_home;

    #[test]
    fn test_path_short() {
        let mut cdb = CookieDB { 
            path: PathBuf::from("./cookies.sqlite"), 
            typing: DbType::Chrome, 
            cookies: vec![] 
        };
        assert_eq!(cdb.path_short(), "./cookies.sqlite");

        cdb.path = PathBuf::from("../../var/Cookies");
        assert_eq!(cdb.path_short(), "../../var/Cookies");

        cdb.path = PathBuf::from(
            format!("{}/.config/chromium/Default/Cookies", get_home())
        );
        assert_eq!(cdb.path_short(), "~/.config/chromium/Default");
    }
}


