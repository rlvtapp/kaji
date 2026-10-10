"""Audit default HTTP/GraphQL SDK output across languages and API styles.

This checks generation, regeneration and final source layout. It deliberately
does not claim compilation, runtime behavior or formatter conformance.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

LANGUAGES = ('typescript', 'rust', 'go', 'python', 'php', 'java', 'csharp',
             'ruby', 'swift', 'elixir', 'symfony')
EXTENSIONS = {'.ts', '.rs', '.go', '.py', '.php', '.java', '.cs', '.rb', '.swift', '.ex'}


def statistics(root, budget):
    sources = []
    diagnostics = []
    for path in root.rglob('source-layout-diagnostics.json'):
        diagnostics.extend(json.loads(path.read_text()))
    exceptions = {entry['path'] for entry in diagnostics}
    for path in sorted(root.rglob('*')):
        if not path.is_file() or path.suffix not in EXTENSIONS:
            continue
        data = path.read_bytes()
        text = data.decode('utf8')
        relative = path.relative_to(root).as_posix()
        lines = text.splitlines()
        sources.append({'path': relative, 'bytes': len(data), 'lines': len(lines),
                        'max_line_columns': max(map(len, lines), default=0),
                        'final_newline': not data or data.endswith(b'\n'), 'lf_only': b'\r' not in data,
                        'within_budget': len(data) <= budget,
                        'size_exception': relative in exceptions})
    violations = [file for file in sources if not file['final_newline'] or
                  not file['lf_only'] or not (file['within_budget'] or file['size_exception'])]
    return {'source_files': sources, 'violations': violations,
            'long_lines': [file['path'] for file in sources if file['max_line_columns'] > 120]}


def fingerprint(root):
    return {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in sorted(root.rglob('*')) if path.is_file()}


def package(language, protocol, style):
    name = ('@poolster/quality-probe' if language == 'typescript' else
            'poolster/quality-probe' if language in ('php', 'symfony') else 'quality_probe')
    plugin = {'name': 'graphql' if protocol == 'graphql' else 'sdk'}
    result = {'language': language, 'path': 'sdk', 'name': name, 'plugins': [plugin]}
    if protocol == 'graphql':
        plugin['contracts'] = {'graphql': {'style': style}}
    else:
        result['client_style'] = style
    return result


def run_case(binary, root, language, protocol, style, source, operations, budget):
    case = root / protocol / language / style
    case.mkdir(parents=True, exist_ok=False)
    output = case / 'generated'
    config = {'output': {'path': str(output)}, 'packages': [package(language, protocol, style)]}
    if protocol == 'http':
        config['openapi'] = {'input': str(source), 'name': 'Quality Probe', 'version': '1.0.0'}
    else:
        config['input'] = {'format': 'graphql', 'path': str(source),
                           'options': {'operation_files': [str(operations)]}}
    recipe = case / 'poolster.json'
    recipe.write_text(json.dumps(config, indent=2) + '\n')
    command = [str(binary), 'generate', '--config', str(recipe), '--color', 'never']
    result = {'language': language, 'protocol': protocol, 'style': style, 'status': 'failed'}
    for stage in ('generation', 'regeneration'):
        try:
            completed = subprocess.run(command, capture_output=True, text=True, timeout=180)
        except subprocess.TimeoutExpired:
            result[stage] = {'status': 'timeout'}
            return result
        (case / (stage + '.log')).write_text(completed.stdout + completed.stderr)
        result[stage] = {'exit_code': completed.returncode}
        if completed.returncode:
            return result
        current = fingerprint(output)
        if stage == 'generation':
            first = current
        else:
            result['deterministic'] = first == current
            result['changed_paths'] = sorted(path for path in first.keys() | current.keys()
                                             if first.get(path) != current.get(path))
    result.update(statistics(output / 'sdk', budget))
    result['status'] = ('pass' if result['source_files'] and result['deterministic'] and
                        not result['violations'] else 'failed')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True, help='New empty audit directory')
    parser.add_argument('--openapi', type=Path, required=True)
    parser.add_argument('--graphql-schema', type=Path, required=True)
    parser.add_argument('--graphql-operations', type=Path, required=True)
    parser.add_argument('--languages', nargs='+', choices=LANGUAGES, default=LANGUAGES)
    parser.add_argument('--protocols', nargs='+', choices=('http', 'graphql'), default=('http', 'graphql'))
    parser.add_argument('--max-file-bytes', type=int, default=128 * 1024)
    args = parser.parse_args()
    if args.max_file_bytes <= 0:
        parser.error('--max-file-bytes must be positive')
    args.output.mkdir(parents=True, exist_ok=False)
    results = []
    for protocol, styles in (('http', ('flat', 'namespaced')),
                             ('graphql', ('raw', 'flat', 'idiomatic'))):
        if protocol not in args.protocols:
            continue
        source = (args.openapi if protocol == 'http' else args.graphql_schema).resolve(strict=True)
        for language in args.languages:
            for style in styles:
                result = run_case(args.binary.resolve(strict=True), args.output.resolve(), language,
                                  protocol, style, source, args.graphql_operations.resolve(strict=True),
                                  args.max_file_bytes)
                results.append(result)
                print(f"{protocol} {language} {style}: {result['status']}", flush=True)
    report = {'version': 1, 'scope': 'default output structure and regeneration only',
              'formatter_conformance': 'not assessed', 'compilation_runtime': 'separate native tests',
              'cases': results}
    (args.output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    return int(any(case['status'] != 'pass' for case in results))


if __name__ == '__main__':
    raise SystemExit(main())
