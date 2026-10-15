"""packaging/windows/issue121-test.bat, the script the Windows zip ships for the issue #121 test
build: ASCII with CRLF line endings for cmd.exe, only variables the game knows, and (on Windows)
unattended runs against a stand-in game that check each run's variables, copies and the zip."""
from paths import ROOT
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
import zipfile
from pathlib import Path

BAT = ROOT / 'packaging' / 'windows' / 'issue121-test.bat'
# Each run: its folder and the variable it sets, with the value (run A sets none).
RUNS = {'A': ('A-diagnostics', None, ''),
        'B': ('B-viewport', 'GOW3_STATE_PROBE', 'viewport'),
        'C': ('C-depthstruct', 'GOW3_STATE_PROBE', 'depthstruct'),
        'D': ('D-depthreemit', 'GOW3_STATE_PROBE', 'depthreemit'),
        'E': ('E-full-barriers', 'GOW3_FULL_BARRIERS', '1')}
# Moving these moves the saves and the shader caches (path_util.cpp reads GOW3_GPU_USER_DIR at start).
SAVES_VARIABLES = {'GOW3_USER_DIR', 'GOW3_DATA_DIR', 'GOW3_GPU_USER_DIR'}


def names_set_by_the_script():
    """The GOW3_* variables the script sets or clears, directly or through `call :run`."""
    text = BAT.read_text(encoding='ascii')
    direct = re.findall(r'(?im)\bset\s+"?(GOW3_[A-Z0-9_]+)=', text)
    per_run = re.findall(r'(?im)\bcall\s+:run\s+\S+\s+(GOW3_[A-Z0-9_]+)\b', text)
    return set(direct) | set(per_run)


def names_the_game_knows():
    """GOW3_* string literals in the game's C and C++ sources (gpu/ without third_party, src/) and
    in run.py and the launcher."""
    known = set()
    for top in (ROOT / 'gpu', ROOT / 'src'):
        for folder, dirs, files in os.walk(top):
            dirs[:] = [name for name in dirs if name != 'third_party']
            for name in files:
                if name.endswith(('.c', '.cpp', '.h', '.hpp', '.inc')):
                    with open(os.path.join(folder, name), encoding='utf-8', errors='replace') as source:
                        known.update(re.findall(r'"(GOW3_[A-Z0-9_]+)"', source.read()))
    for path in [ROOT / 'run.py', *(ROOT / 'launcher').glob('*.py')]:
        known.update(re.findall(r'[\'"](GOW3_[A-Z0-9_]+)[\'"]', path.read_text(encoding='utf-8')))
    return known


class ScriptFileTest(unittest.TestCase):
    def test_the_windows_package_ships_the_script(self):
        script = (ROOT / 'packaging' / 'windows' / 'package.sh').read_text(encoding='utf-8')
        copies = [line for line in script.splitlines()
                  if 'packaging/windows/issue121-test.bat' in line and '"$dest/"' in line]
        self.assertTrue(copies, 'package.sh must copy packaging/windows/issue121-test.bat into "$dest/"')

    def test_ascii_with_crlf_line_endings(self):
        # cmd.exe reads a .bat in the OEM code page and misreads labels in LF-only files.
        data = BAT.read_bytes()
        self.assertTrue(data.isascii(), 'issue121-test.bat must be ASCII only')
        self.assertNotRegex(data, rb'(?<!\r)\n')
        self.assertTrue(data.endswith(b'\r\n'))

    @unittest.skipUnless(shutil.which('git'), 'needs git')
    def test_git_checks_it_out_with_crlf(self):
        # The Windows CI checks out with core.autocrlf=false: only the attribute gives CRLF there.
        path = 'packaging/windows/issue121-test.bat'
        result = subprocess.run(['git', 'check-attr', 'eol', '--', path], cwd=ROOT,
                                capture_output=True, text=True, check=True)
        self.assertEqual(result.stdout.strip(), f'{path}: eol: crlf')


class VariablesTest(unittest.TestCase):
    def test_it_sets_the_variables_of_every_run(self):
        names = names_set_by_the_script()
        expected = {'GOW3_GPU_LOG', 'GOW3_FRAME_CAPTURE_KEY', 'GOW3_CAPTURE_DIR'}
        expected.update(variable for _, variable, _ in RUNS.values() if variable)
        self.assertLessEqual(expected, names)

    def test_every_variable_it_sets_is_read_by_the_game(self):
        unknown = names_set_by_the_script() - names_the_game_knows()
        self.assertFalse(unknown, f'nothing in gpu/, src/, run.py or launcher/ reads {sorted(unknown)}')

    def test_it_leaves_the_saves_folders_alone(self):
        self.assertFalse(names_set_by_the_script() & SAVES_VARIABLES)


# Stands in for "God of War III.exe --play": prints what the game prints and writes into the saves
# folder (FAKE_SAVES) the way the launcher and the probe do.
FAKE_GAME = r'''@echo off
for %%I in ("%GOW3_CAPTURE_DIR%\..") do set "RUN_NAME=%%~nxI"
if defined FAKE_QUIET (
    >"%FAKE_SAVES%\last_run.log" echo GPU: driver quiet, last_run.log of %RUN_NAME%
    exit /b 0
)
echo Settings: fake game %*
echo vars probe=[%GOW3_STATE_PROBE%] barriers=[%GOW3_FULL_BARRIERS%] log=[%GOW3_GPU_LOG%] key=[%GOW3_FRAME_CAPTURE_KEY%]
echo vars captures=[%GOW3_CAPTURE_DIR%] user=[%GOW3_USER_DIR%] data=[%GOW3_DATA_DIR%] gpuuser=[%GOW3_GPU_USER_DIR%]
echo GPU: driver AMD proprietary, info 24.5.1
echo GPU: area check on, issue 121 test build
if defined GOW3_STATE_PROBE echo GPU: state probe on: %GOW3_STATE_PROBE%
if defined GOW3_FULL_BARRIERS echo GPU: full barriers on, GOW3_FULL_BARRIERS=1
echo GPU: overlap copy 1/32 copy, depth-overlap
echo GPU [Render_Vulkan] ^<Info^> vk_instance.cpp:1 noise 1>&2
echo GPU [Render_Vulkan] ^<Error^> vk_x.cpp:2 bad thing 1>&2
if not exist "%FAKE_SAVES%\screenshots" mkdir "%FAKE_SAVES%\screenshots"
>"%FAKE_SAVES%\last_run.log" echo last_run.log of %RUN_NAME%
>"%FAKE_SAVES%\screenshots\shot_%RUN_NAME%.png" echo png
>"%GOW3_CAPTURE_DIR%\frame_%RUN_NAME%.txt" echo presented buffer 0x1
echo Frame capture: 3 passes written to %GOW3_CAPTURE_DIR%\frame_%RUN_NAME%.txt
echo Screenshot: %FAKE_SAVES%\screenshots\shot_%RUN_NAME%.png
exit /b 3
'''


class Install:
    """A copy of the script in <root>/Juan's games (x86)/gow3-windows (an apostrophe and
    parentheses, as real folders have), the stand-in game and a desktop, all in temp folders.
    `saves` set: a saves folder chosen in the launcher (settings.json); else <port>/user. The
    script looks for `probe` as the running game, so a real game (gow3-probe.exe) does not count."""

    def __init__(self, root, saves=None):
        self.probe = f't121probe{os.getpid()}.exe'  # tasklist cuts image names at 25 characters
        self.path_prefix = None
        self.port = root / "Juan's games (x86)" / 'gow3-windows'
        self.port.mkdir(parents=True)
        (self.port / BAT.name).write_bytes(BAT.read_bytes())
        (self.port / 'gow3.ini').write_text('[gow3]\nresolution = 1080p\n', encoding='utf-8')
        self.appdata = root / 'appdata'
        (self.appdata / 'gow3-launcher').mkdir(parents=True)
        if saves:
            (self.appdata / 'gow3-launcher' / 'settings.json').write_text(
                json.dumps({'user_dir': str(saves)}), encoding='utf-8')
        self.saves = saves or self.port / 'user'
        (self.saves / 'screenshots').mkdir(parents=True)
        old = self.saves / 'screenshots' / 'old.png'
        old.write_bytes(b'png')
        hour_ago = time.time() - 3600
        os.utime(old, (hour_ago, hour_ago))
        self.game = root / 'fake game.cmd'
        self.game.write_bytes(FAKE_GAME.replace('\n', '\r\n').encode('ascii'))
        self.desktop = root / 'desktop'
        self.desktop.mkdir()

    def run(self, *args, keys=None, extra=None):
        """cmd /c issue121-test.bat <args> with no visible window; `keys` go to stdin (the menu),
        `extra` to the environment (FAKE_QUIET: the game prints nothing, as from the launcher)."""
        env = {key: value for key, value in os.environ.items()
               if not key.upper().startswith(('GOW3_', 'T121_'))}
        # Left over from an earlier test build: a run that does not set them must not see them.
        env.update(GOW3_STATE_PROBE='all', GOW3_FULL_BARRIERS='1', GOW3_GPU_LOG='debug')
        env.update(T121_GAME=str(self.game), T121_DESKTOP=str(self.desktop), T121_PROBE=self.probe,
                   APPDATA=str(self.appdata), FAKE_SAVES=str(self.saves))
        env.update(extra or {})
        if self.path_prefix:
            env['PATH'] = f'{self.path_prefix}{os.pathsep}{env["PATH"]}'
        command = 'cmd /d /s /c ""{}" {}"'.format(self.port / BAT.name, ' '.join(args))
        result = subprocess.run(command, cwd=self.port.parent, env=env, input=keys,
                                stdin=None if keys else subprocess.DEVNULL, capture_output=True,
                                timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
        self.output = result.stdout.decode('oem', errors='replace') + result.stderr.decode('oem', errors='replace')
        sessions = list((self.port / 'results').iterdir()) if (self.port / 'results').is_dir() else []
        self.session = sessions[0] if len(sessions) == 1 else None
        self.zips = list(self.desktop.glob('gow3-issue121-*.zip'))
        return result.returncode

    def read(self, run, name):
        return (self.session / RUNS[run][0] / name).read_text(encoding='utf-8-sig', errors='replace')


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class UnattendedRunTest(unittest.TestCase):
    """issue121-test.bat A E: two runs, then the zip."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.install = Install(Path(cls.tmp.name))
        cls.install.run('A', 'E')

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def setUp(self):
        self.assertIsNotNone(self.install.session, self.install.output)

    def test_only_the_runs_asked_for(self):
        self.assertEqual(sorted(path.name for path in self.install.session.iterdir() if path.is_dir()),
                         ['A-diagnostics', 'E-full-barriers'])

    def test_each_run_sets_only_its_variable(self):
        # Only run A logs at info level: it adds tens of MB per run, and the variations only need
        # their own GPU: lines.
        self.assertIn('vars probe=[] barriers=[] log=[info] key=[1]', self.install.read('A', 'console.log'))
        self.assertIn('vars probe=[] barriers=[1] log=[] key=[1]', self.install.read('E', 'console.log'))
        self.assertIn('Settings: fake game --play', self.install.read('A', 'console.log'))
        # environment.txt: the GOW3_* variables the game started with.
        self.assertNotIn('GOW3_FULL_BARRIERS', self.install.read('A', 'environment.txt'))
        self.assertIn('GOW3_GPU_LOG=info', self.install.read('A', 'environment.txt'))
        self.assertIn('GOW3_FULL_BARRIERS=1', self.install.read('E', 'environment.txt'))
        self.assertNotIn('GOW3_GPU_LOG', self.install.read('E', 'environment.txt'))

    def test_captures_go_to_the_run_folder(self):
        for run in 'AE':
            folder = self.install.session / RUNS[run][0]
            self.assertIn(f'captures=[{folder / "captures"}]', self.install.read(run, 'console.log'))
            self.assertTrue((folder / 'captures' / f'frame_{folder.name}.txt').is_file())

    def test_the_saves_folders_are_left_alone(self):
        self.assertIn('user=[] data=[] gpuuser=[]', self.install.read('A', 'console.log'))

    def test_copies_the_settings_and_only_new_screenshots(self):
        for run in 'AE':
            folder = self.install.session / RUNS[run][0]
            # console.log already holds what last_run.log holds: one copy is enough.
            self.assertFalse((folder / 'last_run.log').exists())
            self.assertIn('resolution = 1080p', self.install.read(run, 'gow3.ini'))
            self.assertEqual(sorted(path.name for path in (folder / 'screenshots').iterdir()),
                             [f'shot_{folder.name}.png'])
            self.assertEqual(self.install.read(run, 'exit_code.txt').strip(), '3')

    def test_summary_keeps_the_marker_lines(self):
        a, e = self.install.read('A', 'summary.txt'), self.install.read('E', 'summary.txt')
        for line in ('GPU: driver ', 'GPU: area check ', 'GPU: overlap copy ', 'Frame capture: ',
                     'Screenshot: ', 'Settings: ', '<Error>'):
            self.assertIn(line, a)
            self.assertIn(line, e)
        self.assertNotIn('<Info>', a)
        self.assertNotIn('vars ', a)
        self.assertNotIn('GPU: full barriers', a)
        self.assertNotIn('GPU: state probe', a)
        self.assertIn('GPU: full barriers on', e)

    def test_zips_the_session_to_the_desktop(self):
        self.assertEqual(len(self.install.zips), 1, self.install.output)
        stamp = self.install.session.name
        self.assertEqual(self.install.zips[0].name, f'gow3-issue121-{stamp}.zip')
        with zipfile.ZipFile(self.install.zips[0]) as archive:
            names = set(archive.namelist())
        for name in ('A-diagnostics/console.log', 'A-diagnostics/summary.txt',
                     'E-full-barriers/console.log', 'system.txt'):
            self.assertIn(f'{stamp}/{name}', names)


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class QuietConsoleTest(unittest.TestCase):
    """The game's output did not reach console.log: last_run.log is copied and summarized."""

    def test_falls_back_to_last_run_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(Path(tmp))
            install.run('B', extra={'FAKE_QUIET': '1'})
            self.assertIsNotNone(install.session, install.output)
            self.assertEqual(install.read('B', 'last_run.log').strip(),
                             'GPU: driver quiet, last_run.log of B-viewport')
            self.assertIn('GPU: driver quiet', install.read('B', 'summary.txt'))
            # The run worked: no "did not start" scare.
            self.assertIn('came from last_run.log', install.output)
            self.assertNotIn('did not start', install.output)


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class BigZipTest(unittest.TestCase):
    """Over GitHub's attachment limit (T121_ZIP_LIMIT_MB stands in for the 24 MB): a second zip
    without the screenshots, with the same folders."""

    def test_makes_a_text_only_zip(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(Path(tmp))
            install.run('A', extra={'T121_ZIP_LIMIT_MB': '0'})
            self.assertIsNotNone(install.session, install.output)
            stamp = install.session.name
            names = {path.name for path in install.zips}
            self.assertEqual(names, {f'gow3-issue121-{stamp}.zip', f'gow3-issue121-{stamp}-text.zip'},
                             install.output)
            with zipfile.ZipFile(install.desktop / f'gow3-issue121-{stamp}-text.zip') as archive:
                entries = set(archive.namelist())
            self.assertIn(f'{stamp}/A-diagnostics/console.log', entries)
            self.assertIn(f'{stamp}/system.txt', entries)
            self.assertFalse([name for name in entries if name.endswith('.png')])
            self.assertIn('too big for GitHub', install.output)
            self.assertIn(f'gow3-issue121-{stamp}-text.zip', install.output)


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class EveryRunTest(unittest.TestCase):
    """issue121-test.bat ALL, with a saves folder chosen in the launcher (an accented name)."""

    def test_every_run_with_its_variable(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            install = Install(root, saves=root / 'Saves Área ü')
            install.run('ALL')
            self.assertIsNotNone(install.session, install.output)
            self.assertEqual(sorted(path.name for path in install.session.iterdir() if path.is_dir()),
                             sorted(folder for folder, _, _ in RUNS.values()))
            for run, (folder, variable, value) in RUNS.items():
                probe = value if variable == 'GOW3_STATE_PROBE' else ''
                barriers = value if variable == 'GOW3_FULL_BARRIERS' else ''
                self.assertIn(f'vars probe=[{probe}] barriers=[{barriers}] ', install.read(run, 'console.log'))
                self.assertEqual(sorted(path.name for path in (install.session / folder / 'screenshots').iterdir()),
                                 [f'shot_{folder}.png'])
                summary = install.read(run, 'summary.txt')
                self.assertEqual(f'GPU: state probe on: {probe}' in summary, bool(probe), run)
            self.assertEqual(len(install.zips), 1, install.output)


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class RunningGameTest(unittest.TestCase):
    """A game still running stops the script before any run, also with a Unix find (Git, MSYS2)
    ahead of Windows' find.exe in PATH."""

    def test_stops_while_the_game_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            install = Install(root)
            # A Unix find does not understand /i and fails, which reads as "not running".
            (root / 'unix').mkdir()
            (root / 'unix' / 'find.cmd').write_bytes(b'@exit /b 1\r\n')
            install.path_prefix = root / 'unix'
            probe = root / install.probe
            shutil.copy(Path(os.environ['SystemRoot']) / 'System32' / 'PING.EXE', probe)
            game = subprocess.Popen([str(probe), '-n', '120', '127.0.0.1'], stdout=subprocess.DEVNULL,
                                    creationflags=subprocess.CREATE_NO_WINDOW)
            try:
                self.assertEqual(install.run('A'), 1, install.output)
            finally:
                game.kill()
                game.wait()
            self.assertIn('The game is already running.', install.output)
            self.assertIsNone(install.session)


@unittest.skipUnless(sys.platform == 'win32', 'cmd.exe')
class MenuTest(unittest.TestCase):
    """The menu read from stdin: 1 (run A), a key for the pause after it, then Z (zip)."""

    def test_run_a_then_zip(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(Path(tmp))
            install.run(keys=b'1xZx')
            self.assertIsNotNone(install.session, install.output)
            self.assertEqual([path.name for path in install.session.iterdir() if path.is_dir()],
                             ['A-diagnostics'])
            self.assertIn('Run A - diagnostics only. The most important one: do it first.', install.output)
            self.assertEqual(len(install.zips), 1, install.output)

    def test_quit_leaves_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            install = Install(Path(tmp))
            self.assertEqual(install.run(keys=b'Q'), 0, install.output)
            self.assertIsNone(install.session)
            self.assertEqual(install.zips, [])


if __name__ == '__main__':
    unittest.main()
