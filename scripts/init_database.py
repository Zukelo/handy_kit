"""Initialize a development SQLite database with checksum-verified migrations."""
import hashlib
from pathlib import Path
import sqlite3

ROOT = Path(__file__).resolve().parents[1]

def migrate(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(path) as conn:
        conn.execute("PRAGMA foreign_keys = ON")
        conn.execute("CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY, checksum TEXT NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)")
        for source in sorted((ROOT / "database/migrations").glob("*.sql")):
            sql = source.read_text(encoding="utf-8")
            checksum = hashlib.sha256(source.read_bytes()).hexdigest()
            row = conn.execute("SELECT checksum FROM schema_migrations WHERE version = ?", (source.name,)).fetchone()
            if row:
                if row[0] != checksum:
                    raise RuntimeError(f"Migration checksum mismatch: {source.name}")
                continue
            # Migration files are controlled repository SQL. Transaction covers schema and ledger.
            version = source.name.replace("'", "''")
            try:
                conn.executescript("BEGIN IMMEDIATE;\n" + sql + f"\nINSERT INTO schema_migrations(version, checksum) VALUES ('{version}', '{checksum}');\nCOMMIT;")
            except Exception:
                if conn.in_transaction:
                    conn.rollback()
                raise
    return path

if __name__ == "__main__":
    print(migrate(ROOT / "database/data/handykit.sqlite3"))
