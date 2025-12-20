//! SQLite serialization for typed enums
//!
//! Implements ToSql and FromSql for Status, Priority, and LinkType
//! to enable typed storage and retrieval from SQLite.

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

use crate::core::entity::{Priority, Status};

use super::types::LinkType;

// =========================================================================
// Status - ToSql/FromSql
// =========================================================================

impl ToSql for Status {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for Status {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse()
            .map_err(|e: String| FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            ))))
    }
}

// =========================================================================
// Priority - ToSql/FromSql
// =========================================================================

impl ToSql for Priority {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for Priority {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse()
            .map_err(|e: String| FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            ))))
    }
}

// =========================================================================
// LinkType - ToSql/FromSql
// =========================================================================

impl std::str::FromStr for LinkType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "traces_to" => Ok(LinkType::TracesTo),
            "traces_from" => Ok(LinkType::TracesFrom),
            "verifies" => Ok(LinkType::Verifies),
            "verified_by" => Ok(LinkType::VerifiedBy),
            "mitigates" => Ok(LinkType::Mitigates),
            "mitigated_by" => Ok(LinkType::MitigatedBy),
            "references" => Ok(LinkType::References),
            "referenced_by" => Ok(LinkType::ReferencedBy),
            "contains" => Ok(LinkType::Contains),
            "contained_in" => Ok(LinkType::ContainedIn),
            "quotes_for" => Ok(LinkType::QuotesFor),
            "quoted_by" => Ok(LinkType::QuotedBy),
            _ => Err(format!("Unknown link type: {}", s)),
        }
    }
}

impl std::fmt::Display for LinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl ToSql for LinkType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for LinkType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse()
            .map_err(|e: String| FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e,
            ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_status_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test (status TEXT)", []).unwrap();

        // Insert each status
        for status in [
            Status::Draft,
            Status::Review,
            Status::Approved,
            Status::Released,
            Status::Obsolete,
        ] {
            conn.execute("DELETE FROM test", []).unwrap();
            conn.execute("INSERT INTO test VALUES (?1)", [&status])
                .unwrap();

            let retrieved: Status = conn
                .query_row("SELECT status FROM test", [], |row| row.get(0))
                .unwrap();

            assert_eq!(status, retrieved);
        }
    }

    #[test]
    fn test_priority_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test (priority TEXT)", [])
            .unwrap();

        for priority in [
            Priority::Low,
            Priority::Medium,
            Priority::High,
            Priority::Critical,
        ] {
            conn.execute("DELETE FROM test", []).unwrap();
            conn.execute("INSERT INTO test VALUES (?1)", [&priority])
                .unwrap();

            let retrieved: Priority = conn
                .query_row("SELECT priority FROM test", [], |row| row.get(0))
                .unwrap();

            assert_eq!(priority, retrieved);
        }
    }

    #[test]
    fn test_link_type_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test (link_type TEXT)", [])
            .unwrap();

        for link_type in [
            LinkType::TracesTo,
            LinkType::Verifies,
            LinkType::Mitigates,
            LinkType::Contains,
        ] {
            conn.execute("DELETE FROM test", []).unwrap();
            conn.execute("INSERT INTO test VALUES (?1)", [&link_type])
                .unwrap();

            let retrieved: LinkType = conn
                .query_row("SELECT link_type FROM test", [], |row| row.get(0))
                .unwrap();

            assert_eq!(link_type, retrieved);
        }
    }
}
