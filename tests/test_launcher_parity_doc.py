"""The parity map: every Python launcher test has one row, a status, and a Rust test that exists."""
import collections
import re
import unittest

from paths import ROOT

DOC = ROOT / 'docs/specs/2026-10-21-rust-launcher-parity.md'
RUST = ROOT / 'launcher-rust'
TESTS = ROOT / 'tests'
THIS = 'test_launcher_parity_doc.py'

# Tk-only mechanics the Rust launcher has no equivalent for; every other drop is a gap.
ALLOWED_DROP_FILES = {
    'tests/test_launcher_font.py',
    'tests/test_launcher_scaling.py',
    'tests/test_launcher_stdio.py',
    'tests/test_launcher_diagnostics_layout.py',
}
GAP = 'dropped: no Rust counterpart — '

RUST_ID = re.compile(r'`([^`]+\.rs(?:::\w+)+)`')
STATUS = re.compile(r'^(ported|replaced by (?P<by>.+)|dropped: (?P<why>\S.*))$')


def python_tests():
    """`file::Class::test` for every test method of the files the map covers, in file order."""
    files = sorted(p for p in TESTS.glob('test_launcher_*.py') if p.name != THIS)
    files += [TESTS / 'test_trophies.py', TESTS / 'test_button_icons.py']
    found = []
    for path in files:
        owner = None
        for line in path.read_text(encoding='utf-8').splitlines():
            klass = re.match(r'^class (\w+)', line)
            if klass:
                owner = klass.group(1)
            method = re.match(r'^\s*def (test_\w+)', line)
            if method:
                prefix = f'tests/{path.name}::'
                if line[0].isspace() and owner:
                    prefix += f'{owner}::'
                found.append(prefix + method.group(1))
    return found


def table_rows():
    """(python test, rust cell, status) for each row of the map, in document order: the rows whose
    first cell names a test (`tests/<file>.py::...`), not the per-file counts."""
    rows = []
    for line in DOC.read_text(encoding='utf-8').splitlines():
        if not re.match(r'\| `tests/[^`]+\.py::', line):
            continue
        cells = [cell.strip() for cell in line.strip().strip('|').split(' | ')]
        while len(cells) < 3:
            cells.append('')
        rows.append((cells[0].strip('`'), cells[1], cells[2]))
    return rows


def gap_list():
    """The Python tests the Gaps section names."""
    text = DOC.read_text(encoding='utf-8')
    section = text.split('\n## Gaps', 1)
    if len(section) < 2:
        return None
    body = section[1].split('\n## ', 1)[0]
    return re.findall(r'^- `(tests/[^`]+)`', body, flags=re.MULTILINE)


def rust_tests(path):
    """Names of the `#[test]` functions in a Rust file, and the modules it declares."""
    names, pending = set(), False
    lines = path.read_text(encoding='utf-8').splitlines()
    for line in lines:
        stripped = line.strip()
        if stripped == '#[test]':
            pending = True
            continue
        if pending:
            match = re.match(r'(?:pub\s+)?(?:async\s+)?fn (\w+)\s*\(', stripped)
            if match:
                names.add(match.group(1))
                pending = False
            elif not (stripped.startswith('#[') or stripped.startswith('//') or not stripped):
                pending = False
    modules = set(re.findall(r'^\s*(?:pub\s+)?mod (\w+)\s*\{', '\n'.join(lines), flags=re.MULTILINE))
    return names, modules


def missing_rust_test(identifier):
    """Why `file.rs::module::name` names no Rust test, or None when it does."""
    parts = identifier.split('::')
    path = RUST / parts[0]
    if not parts[0].endswith('.rs') or len(parts) < 2:
        return f'{identifier}: not file.rs::name'
    if not path.is_file():
        return f'{identifier}: no file {path.relative_to(ROOT).as_posix()}'
    names, modules = rust_tests(path)
    for module in parts[1:-1]:
        if module not in modules:
            return f'{identifier}: no module {module}'
    if parts[-1] not in names:
        return f'{identifier}: no #[test] fn {parts[-1]}'
    return None


class ParityDocTest(unittest.TestCase):
    maxDiff = None  # the failures name the rows to fix

    def test_every_python_test_has_exactly_one_row(self):
        expected = python_tests()
        listed = collections.Counter(row[0] for row in table_rows())
        self.assertEqual(sorted(name for name, count in listed.items() if count > 1), [], 'listed twice')
        self.assertEqual(sorted(set(expected) - set(listed)), [], 'no row')
        self.assertEqual(sorted(set(listed) - set(expected)), [], 'row for no Python test')

    def test_every_row_has_a_known_status(self):
        bad = [f'{name}: {status!r}' for name, _, status in table_rows() if not STATUS.match(status)]
        self.assertEqual(bad, [])

    def test_drops_name_no_rust_test_and_outside_tk_files_are_gaps(self):
        bad = []
        for name, rust, status in table_rows():
            if not status.startswith('dropped: '):
                continue
            if rust != '—':
                bad.append(f'{name}: dropped but names {rust}')
            if name.split('::')[0] not in ALLOWED_DROP_FILES and not status.startswith(GAP):
                bad.append(f'{name}: drop outside the Tk-only files must start {GAP!r}')
            if status.startswith(GAP) and len(status) == len(GAP):
                bad.append(f'{name}: gap without what it tested')
        self.assertEqual(bad, [])

    def test_ported_and_replaced_rows_name_rust_tests_that_exist(self):
        bad = []
        for name, rust, status in table_rows():
            match = STATUS.match(status)
            if not match or status.startswith('dropped: '):
                continue
            named = RUST_ID.findall(rust)
            if not named:
                bad.append(f'{name}: no Rust test in {rust!r}')
            if match.group('by') is not None:
                by = RUST_ID.findall(match.group('by'))
                if not by or not set(by) <= set(named):
                    bad.append(f'{name}: replaced by {match.group("by")!r}, not a test of the row')
            for identifier in named:
                problem = missing_rust_test(identifier)
                if problem:
                    bad.append(f'{name}: {problem}')
        self.assertEqual(bad, [])

    def test_gaps_section_lists_every_gap_row(self):
        gaps = gap_list()
        self.assertIsNotNone(gaps, 'no "## Gaps" section')
        rows = sorted(name for name, _, status in table_rows() if status.startswith(GAP))
        self.assertEqual(sorted(gaps), rows)


if __name__ == '__main__':
    unittest.main()
