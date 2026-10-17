"""The launcher on macOS: open instead of xdg-open, no Linux options, the .app desktop shortcut."""
from pathlib import Path
import plistlib
import shlex
import shutil
import sys
import tempfile
import unittest
from unittest import mock

from paths import ROOT
sys.path.insert(0, str(ROOT / 'launcher'))
import gow3_launcher_win as launcher  # noqa: E402


class MacosAppTest(unittest.TestCase):
    def setUp(self):
        self.desktop = Path(tempfile.mkdtemp()) / 'Desktop folder'
        self.addCleanup(shutil.rmtree, self.desktop.parent, True)
        self.command = ['/opt/homebrew/bin/python3', '/Users/me/God of War 3/launcher/gow3_launcher_win.py']

    def make(self):
        return launcher.write_macos_app(self.command, ROOT / 'launcher' / 'gow3.png', self.desktop)

    def test_bundle_names_its_script_and_icon(self):
        app = self.make()
        self.assertEqual(app, self.desktop / 'God of War III.app')
        info = plistlib.loads((app / 'Contents' / 'Info.plist').read_bytes())
        self.assertEqual(info['CFBundleExecutable'], 'gow3')
        self.assertEqual(info['CFBundleIconFile'], 'gow3')
        self.assertEqual(info['CFBundlePackageType'], 'APPL')
        self.assertEqual(info['CFBundleShortVersionString'], launcher.VERSION)

    def test_script_starts_the_launcher_from_the_port_with_quoted_paths(self):
        script = (self.make() / 'Contents' / 'MacOS' / 'gow3').read_text()
        self.assertTrue(script.startswith('#!/bin/bash\n'))
        self.assertIn('/opt/homebrew/bin', script)
        self.assertIn(f'cd {shlex.quote(str(launcher.PORT_DIR))} ', script)
        self.assertIn("exec /opt/homebrew/bin/python3 '/Users/me/God of War 3/launcher/gow3_launcher_win.py' \"$@\"",
                      script)

    @unittest.skipIf(sys.platform == 'win32', 'POSIX executable permissions')
    def test_script_is_executable(self):
        script = self.make() / 'Contents' / 'MacOS' / 'gow3'
        self.assertEqual(script.stat().st_mode & 0o777, 0o755)

    def test_without_sips_there_is_no_icon_file(self):
        with mock.patch.object(launcher.subprocess, 'run', side_effect=OSError('no sips')):
            app = self.make()
        self.assertFalse((app / 'Contents' / 'Resources' / 'gow3.icns').exists())

    def test_clicking_again_rewrites_the_bundle(self):
        self.assertEqual(self.make(), self.make())


class MacosBehaviourTest(unittest.TestCase):
    def test_opener(self):
        with mock.patch.object(launcher, 'MACOS', True):
            self.assertEqual(launcher.opener(), 'open')
        with mock.patch.object(launcher, 'MACOS', False):
            self.assertEqual(launcher.opener(), 'xdg-open')

    def test_linux_switches_are_not_passed_on_macos(self):
        settings = {**launcher.APP_DEFAULTS, 'game_dir': '', 'video_backend': 'x11', 'gamemode': True,
                    'mangohud': True}
        with mock.patch.object(launcher, 'WINDOWS', False), mock.patch.object(launcher, 'MACOS', True), \
                mock.patch.dict(launcher.os.environ, {}, clear=True):
            env = launcher.game_environment(settings)
        for name in ('SDL_VIDEO_DRIVER', 'SDL_VIDEODRIVER', 'GOW3_GAMEMODE', 'GOW3_MANGOHUD'):
            self.assertNotIn(name, env)
        with mock.patch.object(launcher, 'WINDOWS', False), mock.patch.object(launcher, 'MACOS', False), \
                mock.patch.dict(launcher.os.environ, {}, clear=True):
            env = launcher.game_environment(settings)
        self.assertEqual((env['SDL_VIDEO_DRIVER'], env['GOW3_GAMEMODE'], env['GOW3_MANGOHUD']), ('x11', '1', '1'))


if __name__ == '__main__':
    unittest.main()
