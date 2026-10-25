"""The runner parity map: every Python runner test has one row, a status, and a Rust test that exists."""
import unittest

from paths import ROOT
from parity_doc import check

DOC = ROOT / 'docs/specs/2026-10-22-rust-runner-parity.md'
TESTS = ROOT / 'tests'

# The Python files of the runner (spec §7), in the order the map lists them.
FILES = [
    'test_prepare.py', 'test_link_libc.py', 'test_link_modules.py', 'test_isa_rewrite.py',
    'test_run_isa.py', 'test_patches.py', 'test_mods.py', 'test_mod_conflicts.py',
    'test_glyphs.py', 'test_content.py', 'test_guest_memory.py', 'test_graphics_options.py',
    'test_save_backup.py', 'test_pause_menu.py', 'test_native_cheats.py', 'test_legacy.py',
    'test_discord_presence.py', 'test_game_update.py',
]

# Mechanics with no Rust equivalent, each named by its test (or class) and explained in its row:
# the C runtime's own test, the Python-only iced-x86 import guard and the PowerShell shortcut rows.
ALLOWED_DROPS = {
    'tests/test_content.py::ContentTests',
    'tests/test_isa_rewrite.py::ReadBootLinkedTests::test_main_without_iced_x86',
}

# Tests main added or changed (merged in Task 42) that a later task of the deepening plan ports:
# each row says `dropped: pending port (Task NN)`, and the porting task takes the test out of here.
PENDING_PORTS = {
    # Task 49
    'tests/test_pause_menu.py::GameScript::test_enemy_health_bar_follows_screen_y_with_native_animation',
    'tests/test_pause_menu.py::GameScript::test_display_initializes_health_value_before_the_row_is_selected',
    'tests/test_pause_menu.py::GameScript::test_options_row',
    'tests/test_pause_menu.py::GameScript::test_every_other_goto_is_unchanged',
    'tests/test_native_cheats.py::NativeCheats::test_new_controls_labels_and_graphics_restore_are_bilingual',
    'tests/test_native_cheats.py::NativeCheats::test_audio_clears_unused_rows_inherited_from_options',
    'tests/test_native_cheats.py::NativeCheats::test_display_contains_eight_new_controls_and_keeps_its_original_rows',
    'tests/test_native_cheats.py::NativeCheats::test_display_scrolls_without_overwriting_original_setting_values',
    'tests/test_native_cheats.py::NativeCheats::test_custom_pages_preserve_the_native_footer',
    'tests/test_native_cheats.py::NativeCheats::test_every_state_reference_and_relative_call_stays_in_the_script',
}


class RunnerParityDocTest(unittest.TestCase):
    maxDiff = None  # the failures name the rows to fix

    def test_the_map_is_complete_and_honest(self):
        check(DOC, [TESTS / name for name in FILES], self, allowed_drops=ALLOWED_DROPS | PENDING_PORTS)


if __name__ == '__main__':
    unittest.main()
