"""Exercise the diagnostic sync script using isolated fake git/zip/svn only."""
from pathlib import Path
import os
import shlex
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'development/examples/code-sync/sync_code-merge-debug.sh'


class SyncDiagnosticsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='handykit sync ')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.work = self.root / 'script folder'
        self.work.mkdir()
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.svn = self.root / 'svn target'
        self.svn.mkdir()
        self.other = self.root / 'other cwd'
        self.other.mkdir()
        self.script = self.work / 'sync_code-merge-debug.sh'
        shutil.copy2(SCRIPT, self.script)
        self.secret = 'TEST-password[x]/$secret-123'
        self.trace = self.root / 'trace'
        self.env = {**os.environ, 'PATH': f'{self.bin}:/opt/homebrew/bin:/usr/bin:/bin',
                    'HK_TEST_TRACE': str(self.trace), 'HK_TEST_SECRET': self.secret}
        self.env.pop('BASH_ENV', None)
        self.env.pop('ENV', None)
        self.bash = shutil.which('bash')
        self.assertIsNotNone(self.bash)
        # These are the only git/zip/svn processes the tested script can invoke.
        self.mock('git', '''
case "$1" in
  --version) printf 'git test-version\\n'; exit 0 ;;
esac
printf 'git %s\\n' "$*" >> "$HK_TEST_TRACE"
printf 'git stdout\\n'
printf 'git stderr progress\\n' >&2
printf 'remote https://user:URL-CREDENTIAL@demo.invalid/repo?token=QUERY-CREDENTIAL\\n'
if [[ "${HK_FAIL_STAGE:-}" == "$1" ]]; then exit 42; fi
if [[ "$1" == clone ]]; then
  target=${!#}
  mkdir -p -- "$target"
  printf 'sample' > "$target/source.txt"
fi
if [[ "$1" == rev-parse ]]; then printf 'abc123\\n'; fi
''')
        self.mock('zip', '''
if [[ "$1" == -v ]]; then printf 'zip test-version\\n'; exit 0; fi
printf 'zip %s\\n' "$*" >> "$HK_TEST_TRACE"
while [[ "$1" == -* ]]; do shift; done
printf 'archive' > "$1"
''')
        self.mock('svn', '''
if [[ "$1" == --version ]]; then printf 'svn test-version\\n'; exit 0; fi
printf 'svn %s\\n' "$*" >> "$HK_TEST_TRACE"
printf 'svn diagnostic password=%s\\n' "$HK_TEST_SECRET"
if [[ "${HK_FAIL_STAGE:-}" == svn ]]; then exit 43; fi
''')
        self.config()
        # directory intentionally precedes mergebranch; original parser ran too early.
        (self.work / 'repository.ini').write_bytes(
            b'\xef\xbb\xbf[repository-one]\r\nbranch=main\r\n'
            b'url=https://user:URL-CREDENTIAL@demo.invalid/repo?token=QUERY-CREDENTIAL\r\n'
            b'directory=repo with spaces\r\nmergebranch=integration\r\n')

    def mock(self, name, body):
        p = self.bin / name
        p.write_text('#!/usr/bin/env bash\n' + body)
        p.chmod(0o755)

    def config(self, suffix=''):
        (self.work / 'config.sh').write_text(
            f'svn_dir={shlex.quote(str(self.svn))}\n'
            'svn_username=test-user\n'
            f'svn_password={shlex.quote(self.secret)}\n'
            'svn_timeout=30\n'
            'printf "config output password=%s\\n" "$svn_password"\n'
            f'cd -- {shlex.quote(str(self.other))}\n' + suffix)

    def run_script(self, *args):
        p = subprocess.run([self.bash, str(self.script), *args], cwd=self.other, env=self.env,
                           stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=20)
        logs = list(self.work.glob('sync_code-debug-*.log'))
        self.assertEqual(len(logs), 1, p.stdout + p.stderr)
        log = logs[0].read_text()
        for data in [p.stdout, p.stderr, log]:
            for secret in [self.secret, 'URL-CREDENTIAL', 'QUERY-CREDENTIAL']:
                self.assertNotIn(secret, data, 'Credential leaked into diagnostics')
        return p, log

    def test_success_logs_all_stages_handles_spaces_and_defers_section(self):
        (self.work / 'unrelated.zip').write_text('must stay')
        p, log = self.run_script()
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        trace = self.trace.read_text()
        self.assertIn('fetch origin integration:refs/remotes/origin/integration', trace)
        self.assertIn('merge -s recursive -X theirs origin/integration', trace)
        self.assertIn('svn commit --non-interactive', trace)
        self.assertIn('git stderr progress', log)
        self.assertIn('rc=0', log)
        self.assertIn('sections=1 archived=1 svn_committed=1 failed=0', log)
        self.assertIn('cwd_after_source=', log)
        self.assertIn('[REDACTED]', log)
        self.assertTrue((self.svn / 'repo with spaces.zip').exists())
        self.assertFalse((self.work / 'repo with spaces').exists())
        self.assertFalse((self.work / 'repo with spaces.zip').exists())
        self.assertEqual((self.work / 'unrelated.zip').read_text(), 'must stay')

    def test_failed_merge_preserves_clone_and_stops_push_zip_svn(self):
        self.env['HK_FAIL_STAGE'] = 'merge'
        p, log = self.run_script()
        self.assertEqual(p.returncode, 1, p.stdout + p.stderr)
        self.assertIn('git merge rc=42', log)
        trace = self.trace.read_text()
        self.assertNotIn('git push', trace)
        self.assertNotIn('zip ', trace)
        self.assertNotIn('svn commit', trace)
        self.assertTrue((self.work / 'repo with spaces').is_dir())

    def test_svn_failure_preserves_archive_and_records_original_status(self):
        self.env['HK_FAIL_STAGE'] = 'svn'
        p, log = self.run_script()
        self.assertEqual(p.returncode, 1, p.stdout + p.stderr)
        self.assertIn('svn commit rc=43', log)
        self.assertTrue((self.work / 'repo with spaces.zip').is_file())

    def test_check_mode_has_no_sync_and_no_stdin_pause(self):
        p, log = self.run_script('--check')
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertFalse(self.trace.exists())
        self.assertIn('只读配置/工具检查完成', log)

    def test_source_error_gets_line_and_exit_log_before_any_git(self):
        self.config('false\n')
        p, log = self.run_script()
        self.assertEqual(p.returncode, 2, p.stdout + p.stderr)
        self.assertIn('config.sh line=', log)
        self.assertIn('rc=1', log)
        self.assertIn('运行结束：rc=2', log)
        self.assertFalse(self.trace.exists())

    def test_bom_config_is_diagnosed_before_any_source_or_sync(self):
        p = self.work / 'config.sh'
        p.write_bytes(b'\xef\xbb\xbf' + p.read_bytes())
        result, log = self.run_script()
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
        self.assertIn('UTF-8 BOM', log)
        self.assertNotIn('config output', log)
        self.assertFalse(self.trace.exists())

    def test_config_enabling_xtrace_does_not_leak_known_password(self):
        self.config('set -x\nprintf "trace secret %s\\n" "$svn_password"\n')
        p, log = self.run_script('--check')
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn('[REDACTED]', log)


if __name__ == '__main__':
    unittest.main()
