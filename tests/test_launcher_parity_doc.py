"""The launcher parity map: every Python launcher test has one row, a status, and a Rust test that exists."""
import unittest

from paths import ROOT
from parity_doc import check

DOC = ROOT / 'docs/specs/2026-10-21-rust-launcher-parity.md'
TESTS = ROOT / 'tests'
THIS = 'test_launcher_parity_doc.py'

# Tk-only mechanics the Rust launcher has no equivalent for; every other drop is a gap.
ALLOWED_DROP_FILES = {
    'tests/test_launcher_font.py',
    'tests/test_launcher_scaling.py',
    'tests/test_launcher_stdio.py',
    'tests/test_launcher_diagnostics_layout.py',
    'tests/test_launcher_titlebar.py',
}

# Tests main added or changed (merged in Task 42) that a later task of the deepening plan ports:
# each row says `dropped: pending port (Task NN)`, and the porting task takes the test out of here.
PENDING_PORTS = {
    # Task 47
    'tests/test_launcher_controls.py::ControlMapTest::test_glyph_style_ini_overrides_legacy_launcher_choice',
    'tests/test_launcher_monitor.py::MonitorEnvironmentTest::test_ini_migrates_legacy_display_preferences_and_then_takes_precedence',
    'tests/test_launcher_monitor.py::MonitorLauncherTest::test_graphics_page_offers_the_choice',
    'tests/test_launcher_monitor.py::QuickSettingsMonitorTest::test_the_choice_is_saved_and_comes_back_on_reopening',
}


def covered_files():
    """The test files the map covers, in file order."""
    files = sorted(p for p in TESTS.glob('test_launcher_*.py') if p.name != THIS)
    return files + [TESTS / name for name in (
        'test_trophies.py', 'test_button_icons.py', 'test_controller_mappings.py',
        'test_controller_mapper_launch.py', 'test_controller_mapper_sdl.py')]


class ParityDocTest(unittest.TestCase):
    maxDiff = None  # the failures name the rows to fix

    def test_the_map_is_complete_and_honest(self):
        check(DOC, covered_files(), self, allowed_drops=ALLOWED_DROP_FILES | PENDING_PORTS)


if __name__ == '__main__':
    unittest.main()
