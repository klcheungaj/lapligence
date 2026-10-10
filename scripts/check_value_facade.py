import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEX = re.compile(r'//[^\n]*|/\*.*?\*/|r(?P<hash>\#{0,8})".*?"(?P=hash)|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])\'', re.S)
MEMBER = re.compile(r'(?P<receiver>(?:[a-zA-Z_]\w*|\{[^{}]*\})(?:(?:\.|->)[a-zA-Z_]\w*|\[[^\]\n]*\])*)?\s*(?:\.|->)\s*(?P<field>bits|x|z|width|is_signed)\b')
METADATA = {
    'rt/container/dynamic_arrays.c': ('changes',),
    'rt/container/queues.c': ('changes',),
    'rt/llg_container_prelude.c': ('record',),
    # llg_dpi_type_t layout tables: `width` is a DPI payload bit count.
    'rt/llg_dpi.c': ('element', 'type', 'type->element', 'type->members[m].type'),
    'rt/llg_rt_selftest.c': ('net', ''),
    'rt/llg_vpi.c': ('args[i]', 'argument', 'call->args[i]', 'object', 'site->args[i]'),
    'rt/llg_wave.c': ('g_wave.regs[alias]', 'g_wave.regs[i]', 'reg'),
    'rt/scheduler/activations.c': ('binding->descriptor',),
    'rt/scheduler/concurrent_assertions.c': ('local',),
    'rt/scheduler/dependencies.c': ('dependency', 'single'),
    'rt/scheduler/file_io.c': ('target',),
    'rt/scheduler/force.c': ('part',),
    'rt/scheduler/formatting.c': ('spec',),
    'rt/scheduler/mailboxes.c': ('mailbox', 'ref', 'result', 'target', 'value'),
    'rt/scheduler/nets.c': ('driver->net', 'net', 'part->net'),
    'rt/scheduler/process_waits.c': ('deps[i]',),
    'rt/scheduler/reference_writes.c': ('part', 'ref'),
    'rt/scheduler/scanning.c': ('spec', 'target', 'target->packed'),
    'rt/scheduler/sequences.c': ('from', 'local'),
    'rt/scheduler/stochastic.c': ('target',),
    'rt/scheduler/wait_queues.c': ('single',),
    'emit_c/expressions/input.rs': ('',),
    'emit_c/model/dpi.rs': ('',),
    'emit_c/model/storage.rs': ('',),
    'emit_c/model.rs': ('',),
    'emit_c/owned/events.rs': ('',),
    'emit_c/owned/input.rs': ('',),
    'emit_c/owned/model/initialization.rs': ('net',),
    'emit_c/owned/model/lifecycle.rs': ('llg_model_startup_0[_llg_n]',),
    'emit_c/owned/native_tasks.rs': ('',),
    'emit_c/owned/references.rs': ('',),
    'emit_c/statements/events.rs': ('',),
}


def consumer_text(path, text):
    if path.endswith('.rs'):
        literals = []
        for match in LEX.finditer(text):
            literal = match[0]
            if literal.startswith('"'):
                literal = re.sub(r'\\\n[ \t]*', '', literal[1:-1])
                literals.append(literal.replace('\\n', '\n').replace('\\r', '\n'))
            elif literal.startswith(('r"', 'r#')):
                hashes = match['hash']
                literals.append(literal[2 + len(hashes):-(1 + len(hashes))])
        return '\n'.join(literals)
    return LEX.sub(lambda match: '\n' * match[0].count('\n'), text)


def findings(path, source):
    text = consumer_text(path, source)
    errors = []
    for match in MEMBER.finditer(text):
        receiver = match['receiver'] or ''
        field = match['field']
        allowed = field in ('width', 'is_signed') and receiver in METADATA.get(path, ())
        if not receiver:
            before = text[:match.start()].rstrip()
            after = text[match.end():].lstrip()
            allowed = allowed and before.endswith(('{', ',')) and after.startswith('=')
        if not allowed:
            errors.append(f'{path}:{text[:match.start()].count(chr(10)) + 1}: {match[0].strip()}')
    brace = r'\{\{(?!\s*0\s*\}\})' if path.endswith('.rs') else r'\{(?!\s*0\s*\})'
    for match in re.finditer(r'\bsv4_t\s*(?:\)|\w+\s*=)\s*' + brace, text):
        errors.append(f'{path}: private packed initializer: {match[0]}')
    nested = r'\{\{\{\{' if path.endswith('.rs') else r'\{\{'
    for match in re.finditer(r'\bsv4_t\s*\w+\s*\[[^\]]*\]\s*=\s*' + nested, text):
        errors.append(f'{path}: private packed array initializer: {match[0]}')
    return errors


def main():
    if '--self-test' in sys.argv:
        assert findings('rt/new.c', 'value.width = 65; value.bits[0] = 1;')
        assert findings('rt/new.c', '(*value).width = 65;')
        assert findings('rt/new.c', 'sv4_t value = {NULL, NULL, NULL, 0, 0};')
        assert findings('rt/new.c', 'sv4_t values[2] = {{NULL, NULL, NULL, 0, 0}};')
        assert findings('emit_c/new.rs', 'format!("{}.is_signed = 1;", name)')
        assert findings('emit_c/new.rs', 'r#"value->z[0]"#')
        assert findings('emit_c/model/storage.rs', '"(*value).width = 65;"')
        assert not findings('rt/new.c', 'sv4_t value = SV4_EMPTY; llg_sv4_set_state(&value, 0, 1);')
        assert not findings('emit_c/new.rs', 'format!("sv4_zero({})", value.width)')
        assert not findings('rt/scheduler/nets.c', 'net->width')
        assert not findings('emit_c/model/storage.rs', '"{ .width = 65, .is_signed = 0 }"')
        print('value facade scanner self-tests passed')
        return 0
    errors = []
    scanned = 0
    for folder, suffixes in (('rt', ('.c', '.h')), ('emit_c', ('.rs',))):
        paths = list((ROOT / 'src/sim' / folder).rglob('*'))
        if folder == 'emit_c':
            paths.append(ROOT / 'src/sim/emit_c.rs')
        for path in sorted(paths):
            if path.suffix not in suffixes:
                continue
            relative = path.relative_to(ROOT / 'src/sim')
            if folder == 'rt' and (relative.parts[1] in ('value', 'value_gmp') or
                                   path.name.startswith('llg_value')):
                continue
            scanned += 1
            errors.extend(findings(relative.as_posix(), path.read_text(encoding='utf-8')))
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print(f'value facade audit passed: {scanned} runtime/template files; only named nonpacked metadata allowed')
    return 0


if __name__ == '__main__':
    sys.exit(main())
