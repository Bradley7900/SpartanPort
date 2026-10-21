"""Run the Python suite for the Windows integration test package."""
import json
import os
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
os.chdir(ROOT)
sys.path.insert(0, str(ROOT))
POSIX_ONLY = {
    'test_launcher_update.ReplaceAppImageTest.test_a_matching_download_replaces_the_appimage_and_makes_it_executable',
    'test_launcher_update.ReplaceAppImageTest.test_a_symlink_to_the_appimage_replaces_the_file_behind_it',
}


def cases(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from cases(item)
        else:
            yield item


if __name__ == '__main__':
    discovered = list(cases(unittest.defaultTestLoader.discover(str(ROOT / 'tests'))))
    excluded = [test.id() for test in discovered if os.name == 'nt' and test.id() in POSIX_ONLY]
    for name in excluded:
        print(f'Excluded on Windows (POSIX executable permissions): {name}', flush=True)
    suite = unittest.TestSuite(test for test in discovered if test.id() not in excluded)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    report = {
        'discovered': len(discovered), 'run': result.testsRun,
        'skipped': [(test.id(), reason) for test, reason in result.skipped],
        'excluded': excluded,
        'failures': [(test.id(), error) for test, error in result.failures],
        'errors': [(test.id(), error) for test, error in result.errors],
        'successful': result.wasSuccessful(),
    }
    (ROOT / 'out').mkdir(exist_ok=True)
    (ROOT / 'out/test-release-validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    sys.exit(0 if result.wasSuccessful() else 1)
