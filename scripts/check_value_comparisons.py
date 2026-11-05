import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RT = ROOT / 'src/sim/rt'
STORAGE = ROOT / 'tests/runtime_value_storage'
MANIFEST = STORAGE / 'facade_comparisons.tsv'
BRIDGE_TEST = 'facade_compare_${mode}'
HEADERS = ('value/backend.h', 'value/bridge.h', 'value/consumer_bridge.h', 'value/destinations.h')
SKIP_MACROS = {'LLG_VALUE_BUILD_CONFIG', 'LLG_VALUE_LINK_GUARD'}
COMMENT = re.compile(r'//[^\n]*|/\*.*?\*/', re.S)
DEFINE = re.compile(r'^[ \t]*#[ \t]*define[ \t]+([A-Z][A-Z0-9_]*)(\()?', re.M)
IDENT = re.compile(r'[A-Za-z_]\w*')


def strip(text):
    text = COMMENT.sub(lambda m: '\n' * m.group(0).count('\n'), text)
    return re.sub(r'\\\n', ' ', text)


def declarations(text):
    result = {}
    for match in DEFINE.finditer(text):
        name = match.group(1)
        if name.endswith('_H') or name in SKIP_MACROS:
            continue
        result[name] = 'macro'
    body = re.sub(r'^[ \t]*#[^\n]*', '', text, flags=re.M)
    depth = 0
    statement = []
    index = 0
    while index < len(body):
        char = body[index]
        if char == '{':
            head = ''.join(statement).strip()
            if depth == 0 and re.match(r'extern\s+"C"$', head):
                statement = []
                index += 1
                continue
            end = index
            level = 0
            while end < len(body):
                level += body[end] == '{'
                level -= body[end] == '}'
                end += 1
                if not level:
                    break
            inner = body[index + 1:end - 1]
            first = head.split(None, 1)[0] if head else ''
            if first == 'enum' or re.match(r'typedef\s+enum', head):
                for item in inner.split(','):
                    found = IDENT.match(item.strip())
                    if found:
                        result[found.group(0)] = 'constant'
                statement.append(' {} ')
            elif first in ('typedef', 'struct', 'union'):
                statement.append(' {} ')
            else:
                name = IDENT.findall(head.split('(')[0])
                if name and '(' in head:
                    result[name[-1]] = 'inline' if 'static' in head.split('(')[0].split() else 'function'
                statement = []
            index = end
            continue
        if char == ';':
            head = ''.join(statement).strip()
            statement = []
            if '(' in head and not head.startswith('typedef') and 'struct' not in head.split('(')[0].split()[:1]:
                names = IDENT.findall(head.split('(')[0])
                if names:
                    result[names[-1]] = 'function'
            index += 1
            continue
        statement.append(char)
        index += 1
    return result


def operations():
    found = {}
    for header in HEADERS:
        for name, kind in declarations(strip((RT / header).read_text(encoding='utf-8'))).items():
            found.setdefault(name, (kind, header))
    return found


def render(found):
    return ''.join(f'{name}\t{kind}\t{header}\n' for name, (kind, header) in sorted(found.items()))


def load_manifest():
    rows = {}
    for number, line in enumerate(MANIFEST.read_text(encoding='utf-8').splitlines(), 1):
        if not line or line.startswith('#'):
            continue
        fields = line.split('\t')
        if len(fields) != 3:
            raise SystemExit(f'{MANIFEST}:{number}: expected operation, ctest name, source file')
        if fields[0] in rows:
            raise SystemExit(f'{MANIFEST}:{number}: duplicate operation {fields[0]}')
        rows[fields[0]] = (fields[1], fields[2], number)
    return rows


def cmake_tests():
    text = ''
    for path in sorted(STORAGE.glob('*.cmake')) + [STORAGE / 'CMakeLists.txt']:
        text += path.read_text(encoding='utf-8')
    return set(re.findall(r'add_test\(\s*NAME\s+(\S+)', text))


def prefixed(name):
    return 'llg_gmp_' + name[4:] if name.startswith('llg_') else 'llg_gmp_' + name


def test_exists(test, tests):
    if not test.startswith('rust:'):
        return test in tests
    path, _, function = test[5:].partition('::')
    source = ROOT / 'tests' / path
    return source.is_file() and re.search(r'\bfn ' + re.escape(function) + r'\b', source.read_text(encoding='utf-8')) is not None


def listing_problems(found, rows, tests):
    errors = []
    for name in sorted(set(found) - set(rows)):
        errors.append(f'facade operation {name} ({found[name][1]}) has no entry in {MANIFEST.name}: add a test that feeds '
                      'the same input to the legacy and the compact backend and compare the outputs')
    for name in sorted(set(rows) - set(found)):
        errors.append(f'{MANIFEST.name}:{rows[name][2]}: {name} is no longer a facade operation')
    for name, (test, _, number) in sorted(rows.items()):
        if name in found and not test_exists(test, tests):
            errors.append(f'{MANIFEST.name}:{number}: {name}: test {test} does not exist')
    return errors


def source_problems(found, rows):
    errors = []
    for name, (test, source, number) in sorted(rows.items()):
        if name not in found:
            continue
        path = STORAGE / source
        if not path.is_file():
            errors.append(f'{MANIFEST.name}:{number}: {name}: {source} does not exist')
        elif test != BRIDGE_TEST:
            words = r'\b(?:' + re.escape(name) + '|' + re.escape(prefixed(name)) + r')\b'
            if not re.search(words, strip(path.read_text(encoding='utf-8'))):
                errors.append(f'{MANIFEST.name}:{number}: {name} is not used in {source}')
    return errors


def check():
    found = operations()
    rows = load_manifest()
    return listing_problems(found, rows, cmake_tests()) + source_problems(found, rows)


def self_test():
    found = {'sv4_a': ('function', 'h'), 'sv4_b': ('function', 'h')}
    tests = {'known_test'}
    rows = {'sv4_a': ('known_test', 'x.c', 1), 'sv4_b': ('known_test', 'x.c', 2)}
    assert not listing_problems(found, rows, tests)
    unlisted = listing_problems({**found, 'sv4_new': ('function', 'h')}, rows, tests)
    assert len(unlisted) == 1 and 'sv4_new' in unlisted[0], unlisted
    stale = listing_problems({'sv4_a': found['sv4_a']}, rows, tests)
    assert len(stale) == 1 and 'sv4_b' in stale[0], stale
    unknown = listing_problems(found, {**rows, 'sv4_b': ('missing_test', 'x.c', 2)}, tests)
    assert len(unknown) == 1 and 'missing_test' in unknown[0], unknown
    parsed = declarations(strip('void sv4_x(int a);\nstatic inline int sv4_y(void) { return 1; }\n'
                                'enum { SV4_E = 1, SV4_F };\n#define SV4_M(a) (a)\n'))
    assert parsed == {'sv4_x': 'function', 'sv4_y': 'inline', 'SV4_E': 'constant', 'SV4_F': 'constant', 'SV4_M': 'macro'}, parsed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--list', action='store_true', help='print every facade operation')
    parser.add_argument('--self-test', action='store_true')
    arguments = parser.parse_args()
    if arguments.self_test:
        self_test()
        return 0
    if arguments.list:
        sys.stdout.write(render(operations()))
        return 0
    errors = check()
    for error in errors:
        print(error, file=sys.stderr)
    return 1 if errors else 0


if __name__ == '__main__':
    sys.exit(main())
