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
    # Task 43
    'tests/test_launcher_controls.py::ControlMapTest::test_stick_directions_are_saved_and_sent_like_other_keyboard_actions',
    'tests/test_launcher_controls.py::ControlMapTest::test_new_directions_do_not_take_keys_from_saved_actions',
    'tests/test_launcher_controls.py::ControlMapTest::test_unbinding_several_inputs_does_not_swap_their_old_keys',
    'tests/test_launcher_controls.py::ControlMapTest::test_direction_conflicts_swap_and_invalid_entries_are_not_forwarded',
    'tests/test_launcher_controls.py::ControlMapTest::test_keyboard_defaults_put_the_attacks_on_the_mouse',
    'tests/test_launcher_controls.py::ControlMapTest::test_actions_belong_to_the_keyboard_table',
    'tests/test_launcher_controls.py::ControlMapTest::test_the_wheel_cycles_the_weapons_by_default',
    'tests/test_launcher_controls.py::ControlMapTest::test_saved_keyboard_binds_keep_their_layout',
    'tests/test_launcher_controls.py::ControlMapTest::test_layout_two_keeps_its_table_and_gets_the_weapon_cycle',
    'tests/test_launcher_controls.py::ControlMapTest::test_rage_moves_to_f_but_a_saved_table_keeps_the_wheel_click',
    'tests/test_launcher_controls.py::ControlMapTest::test_native_translation_carries_over',
    # Task 47
    'tests/test_launcher_controls.py::ControlMapTest::test_glyph_style_ini_overrides_legacy_launcher_choice',
    'tests/test_launcher_monitor.py::MonitorEnvironmentTest::test_ini_migrates_legacy_display_preferences_and_then_takes_precedence',
    'tests/test_launcher_monitor.py::MonitorLauncherTest::test_graphics_page_offers_the_choice',
    'tests/test_launcher_monitor.py::QuickSettingsMonitorTest::test_the_choice_is_saved_and_comes_back_on_reopening',
    # Task 50
    'tests/test_controller_mappings.py::CaptureTest::test_release_gate_stability_and_noise',
    'tests/test_controller_mappings.py::CaptureTest::test_hats_diagonals_axes_triggers_and_inversion',
    'tests/test_controller_mappings.py::CaptureTest::test_stick_prompt_ignores_buttons_and_endpoint_trigger',
    'tests/test_controller_mappings.py::CaptureTest::test_full_axes_conflict_but_opposite_halves_can_be_shared',
    'tests/test_controller_mappings.py::CaptureTest::test_back_skip_and_cross_requirement',
    'tests/test_controller_mappings.py::CaptureTest::test_start_with_a_held_button_is_rejected',
    'tests/test_controller_mappings.py::PersistenceTest::test_edit_one_input_preserves_other_inputs_and_unknown_sdl_fields',
    'tests/test_controller_mappings.py::PersistenceTest::test_partial_profile_can_start_with_any_button',
    'tests/test_controller_mappings.py::PersistenceTest::test_platform_specific_saved_mapping_wins_over_generic',
    'tests/test_controller_mappings.py::PersistenceTest::test_replace_only_this_guid_and_platform_preserving_comments',
    'tests/test_controller_mappings.py::PersistenceTest::test_failed_commit_preserves_profile_and_cleans_temporary_file',
    'tests/test_controller_mappings.py::PersistenceTest::test_invalid_mapping_does_not_write',
    'tests/test_controller_mappings.py::PersistenceTest::test_explicit_overrides_have_priority_and_restarts_do_not_duplicate',
    'tests/test_controller_mappings.py::PersistenceTest::test_no_profile_leaves_environment_untouched',
    'tests/test_controller_mappings.py::PersistenceTest::test_reserved_xinput_override_is_not_discarded',
    'tests/test_controller_mapper_launch.py::LaunchTest::test_direct_run_loads_profile_before_native_detection',
    'tests/test_controller_mapper_launch.py::LaunchTest::test_updater_preserves_profiles',
    # Task 51
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_raw_capture_gamepad_preview_inversion_and_analog_trigger',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_saved_mapping_loads_and_explicit_user_file_wins',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_disconnect_reports_error_instead_of_reading_a_stale_pad',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_editing_an_existing_sdl_mapping_preserves_guide_and_other_buttons',
    # Task 52
    'tests/test_controller_mapper_launch.py::LaunchTest::test_mapper_is_not_opened_over_a_running_game',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_rows_save_out_of_order_and_reload_without_losing_other_bindings',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_cancel_capture_and_close_keep_already_saved_rows',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_clear_only_one_row_and_allow_an_empty_profile',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_failed_save_keeps_previous_profile_and_row_values',
    'tests/test_controller_mapper_sdl.py::SDLIntegrationTest::test_parent_destroy_closes_sdl_without_saving',
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
