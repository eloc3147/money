const FROM_V0: &str = "
BEGIN TRANSACTION;

CREATE TABLE metadata (
    id INTEGER PRIMARY KEY,
    version INTEGER NOT NULL
);

INSERT INTO metadata (version) VALUES (1);

COMMIT;
";

const FROM_V1: &str = "
BEGIN TRANSACTION;

CREATE TABLE accounts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

UPDATE metadata SET version = 2;

COMMIT;
";

/// Migrations FROM a version.
/// The version number in the database will be one above these migration numbers if the migration has completed
pub const MIGRATIONS: &[&str] = &[FROM_V0, FROM_V1];
