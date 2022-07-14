use crate::types::{DbType,CookieDB,Cookie,CookieField};

impl CookieDB {
    /// Fetch the name of the cookies table depending on
    /// the browser type.
    fn table_name(&self) -> &'static str {
        if self.typing == DbType::Firefox {
            "moz_cookies"
        } else {
            "cookies"
        }
    }

    /// Resolve the given Cookie field name to the
    /// corresponding key in the database for the browser type.
    fn field_name(&self, field_name: &CookieField) -> &'static str {
        match (field_name, &self.typing) {
            (CookieField::Host, DbType::Firefox) => "host",
            (CookieField::Host, DbType::Chrome) => "host_key",

            (CookieField::Name, _) => "name",
            (CookieField::Path, _) => "path",
            // The value field in chrome is usually empty with content
            // instead being present in `encrypted_value`
            (CookieField::Value, _) => "value",

            (CookieField::Creation, DbType::Firefox) => "creationTime",
            (CookieField::Creation, DbType::Chrome) => "creation_utc",

            (CookieField::Expiry, DbType::Firefox) => "expiry",
            (CookieField::Expiry, DbType::Chrome) => "expires_utc",

            (CookieField::HttpOnly, DbType::Firefox) => "isHttpOnly",
            (CookieField::HttpOnly, DbType::Chrome) => "is_http_only",

            (CookieField::LastAccess, DbType::Firefox) => "lastAccessed",
            (CookieField::LastAccess, DbType::Chrome) => "last_access_utc",

            _ => panic!("Unknown `CookieField` parameter")
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

        let query = format!(
            "SELECT {},{},{},{},{},{},{},{} FROM {};",
             self.field_name(&CookieField::Host),
             self.field_name(&CookieField::Name),
             self.field_name(&CookieField::Value),
             self.field_name(&CookieField::Path),
             self.field_name(&CookieField::Creation),
             self.field_name(&CookieField::Expiry),
             self.field_name(&CookieField::LastAccess),
             self.field_name(&CookieField::HttpOnly),
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
                    http_only: row.get::<_,bool>(7)?
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

