"""Behavior checks for migrations and the release boundary."""
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


migration = load('init_database')
release = load('collect_release')
version = load('check_version')


class DeliveryTests(unittest.TestCase):
    def test_version_matches_current_tag(self):
        current = version.check_version()
        self.assertEqual(version.check_version(f'v{current}'), current)
        with self.assertRaises(ValueError):
            version.check_version('v999.0.0')

    def test_collect_distinguishes_windows_editions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = root / 'bundle/nsis'
            bundle.mkdir(parents=True)
            (bundle / 'setup.exe').write_bytes(b'example installer')
            online = release.collect(bundle.parent, root / 'out', 'windows', 'x64', 'online', 'v0.1.0')
            offline = release.collect(bundle.parent, root / 'out', 'windows', 'x64', 'offline', 'v0.1.0')
            self.assertNotEqual(online, offline)
            self.assertEqual(len(list((root / 'out').iterdir())), 2)

    def test_missing_or_duplicate_installers_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaises(ValueError):
                release.collect(root, root / 'out', 'linux', 'x64', 'system-webview', 'v0.1.0')
            (root / 'one.exe').touch()
            (root / 'two.exe').touch()
            with self.assertRaises(ValueError):
                release.collect(root, root / 'out', 'windows', 'x64', 'online', 'v0.1.0')

    def test_migrations_are_idempotent_and_keep_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / 'test.db'
            migration.migrate(db)
            migration.migrate(db)
            with sqlite3.connect(db) as connection:
                self.assertEqual(connection.execute('PRAGMA integrity_check').fetchone()[0], 'ok')
                self.assertEqual(connection.execute('SELECT COUNT(*) FROM schema_migrations').fetchone()[0], len(list((migration.ROOT / 'database/migrations').glob('*.sql'))))

    def test_changed_migration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            migrations = root / 'database/migrations'
            migrations.mkdir(parents=True)
            sql = migrations / '0001.sql'
            sql.write_text('CREATE TABLE example (id INTEGER);')
            with patch.object(migration, 'ROOT', root):
                migration.migrate(root / 'test.db')
                sql.write_text('CREATE TABLE altered (id INTEGER);')
                with self.assertRaisesRegex(RuntimeError, 'checksum mismatch'):
                    migration.migrate(root / 'test.db')

    def test_failed_migration_rolls_back_schema_and_ledger(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            migrations = root / 'database/migrations'
            migrations.mkdir(parents=True)
            (migrations / '0001.sql').write_text('CREATE TABLE example (id INTEGER); INVALID SQL;')
            with patch.object(migration, 'ROOT', root):
                with self.assertRaises(sqlite3.Error):
                    migration.migrate(root / 'test.db')
            with sqlite3.connect(root / 'test.db') as connection:
                self.assertIsNone(connection.execute("SELECT name FROM sqlite_master WHERE name = 'example'").fetchone())
                self.assertEqual(connection.execute('SELECT COUNT(*) FROM schema_migrations').fetchone()[0], 0)

    def test_fresh_database_has_only_current_runtime_tables(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / 'test.db'
            migration.migrate(db)
            with sqlite3.connect(db) as connection:
                names = {row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
                self.assertEqual(names, {'schema_migrations', 'timer_tasks', 'timer_runs'})
                self.assertEqual(connection.execute('SELECT COUNT(*) FROM timer_tasks').fetchone()[0], 0)


if __name__ == '__main__':
    unittest.main()
