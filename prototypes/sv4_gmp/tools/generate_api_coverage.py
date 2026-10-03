import argparse
import json
import re
from pathlib import Path


def clean(text):
    return re.sub(r'/\*.*?\*/|//[^\n]*', '', text, flags=re.S)


def declarations(text):
    pattern = r'^([A-Za-z_]\w*(?:\s*\*)?\s+((?:sv4_|llg_)\w+)\s*\([^;{}]*\))\s*;'
    return {name: ' '.join(signature.split())
            for signature, name in re.findall(pattern, clean(text), re.M)
            if not signature.startswith('typedef ')}


def macros(text):
    text = clean(text).replace('\\\n', '')
    return {name: ' '.join(body.split()) for name, body in
            re.findall(r'^#define\s+([A-Za-z_]\w*(?:\([^\n]*?\))?)([^\n]*)', text, re.M)}


def types(text):
    text = clean(text)
    result = {}
    for match in re.finditer(r'\btypedef\b', text):
        level = 0
        for end in range(match.end(), len(text)):
            char = text[end]
            if char == '{':
                level += 1
            elif char == '}':
                level -= 1
            elif char == ';' and level == 0:
                statement = text[match.start():end]
                callback = None if '{' in statement else re.search(r'\(\*(\w+)\)', statement)
                name = callback[1] if callback else re.search(r'(\w+)\s*$', statement)[1]
                result[name] = ' '.join(statement.split())
                break
    return result


def audit(root):
    repository = root.parents[1]
    golden = (root / 'golden/llg_value.h').read_text()
    current = (repository / 'src/sim/rt/llg_value.h').read_text()
    baseline, production = declarations(golden), declarations(current)
    if baseline != production:
        raise ValueError('Existing public function declarations changed')
    old_macros, new_macros = macros(golden), macros(current)
    old_types, new_types = types(golden), types(current)
    enum_pattern = r'\b((?:LLG_RESOLVE_|LLG_STRENGTH_)\w+)\s*=\s*([^,\n}]+)'
    old_enums = dict(re.findall(enum_pattern, clean(golden)))
    new_enums = dict(re.findall(enum_pattern, clean(current)))
    if any(new_macros.get(name) != body for name, body in old_macros.items()):
        raise ValueError('Existing public macro changed')
    if any(new_types.get(name) != body for name, body in old_types.items()):
        raise ValueError('Existing public helper type changed')
    if old_enums != new_enums:
        raise ValueError('Existing resolution/strength constants changed')
    facade = (root / 'include/sv4.h').read_text()
    aliases = dict(re.findall(r'^#define\s+(sv4_\w+)\s+(gmp4_\w+)\s*$', facade, re.M))
    functions = [{'legacy_name': name, 'declaration': declaration,
                  'status': 'implemented' if name in aliases else 'not_implemented',
                  'new_symbol': aliases.get(name)}
                 for name, declaration in sorted(production.items()) if name.startswith('sv4_')]
    helpers = {name: declaration for name, declaration in production.items() if name.startswith('llg_')}
    if len(functions) != 109 or len(helpers) != 3 or 'sv4_t' in production:
        raise ValueError('Unexpected production declaration inventory; review the frozen contract')
    accessors = sorted(set(re.findall(r'^static inline[^\n]*\b(llg_sv4_\w+)\s*\(', current, re.M)))
    return {'schema': 'llg.sv4-facade-audit/v2',
            'scope': 'Production declarations, original macros/types, and additive neutral bridge; not language coverage',
            'historical_ledger': {'total': 110, 'implemented': 37,
                                  'correction': 'sv4_t callback return type is not a function'},
            'total': len(functions), 'implemented': sum(row['status'] == 'implemented' for row in functions),
            'functions': functions, 'llg_helpers': helpers,
            'macros': old_macros, 'helper_types': old_types, 'enum_constants': old_enums,
            'neutral_accessors': accessors,
            'neutral_types': {name: body for name, body in new_types.items() if name not in old_types},
            'unchanged_production_surface': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Audit the frozen production declarations, macros, types and bridge')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    report = audit(Path(__file__).resolve().parents[1])
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(f"PASS: {report['implemented']}/{report['total']} GMP aliases; 3 llg helpers; existing signatures/macros/types unchanged")
