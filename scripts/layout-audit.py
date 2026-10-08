"""Reproducible large-contract audits for SDK, auxiliary and CLI output.

Generation only by default. Native checking uses existing explicitly supplied
cached dependencies; this script never installs packages or calls described APIs.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys

loader = importlib.util.spec_from_file_location('guru_corpus', Path(__file__).with_name('guru-corpus.py'))
corpus = importlib.util.module_from_spec(loader)
loader.loader.exec_module(corpus)
PROFILES = ('go', 'typescript', 'auxiliary', 'typescript-cli', 'rust-cli')
AUXILIARY = ('zod', 'tanstack-react-query', 'tanstack-vue-query', 'swr', 'faker', 'msw', 'cypress')
SOURCE_SUFFIXES = {'.rs', '.go', '.py', '.js', '.cjs', '.mjs', '.ts'}
SOURCE_EXCLUSIONS = {'test', 'tests', 'fixtures', 'generated', 'dist', 'node_modules', 'target', 'templates'}
SOURCE_LINE_LIMIT = 400


def source_files(project):
    """Yield authored implementation files; generated output and probes have separate audits."""
    for root in ('crates', 'packages', 'scripts', 'openapi'):
        for path in (project / root).rglob('*'):
            relative = path.relative_to(project)
            if (not path.is_file() or path.suffix not in SOURCE_SUFFIXES or
                    SOURCE_EXCLUSIONS.intersection(relative.parts)):
                continue
            if root == 'crates' and 'src' not in relative.parts:
                continue
            yield relative, path


def source_size_audit(project, baseline):
    """Reject new oversized files and growth in recorded legacy files."""
    budgets = json.loads(baseline.read_text())['legacy_line_budgets']
    violations = []
    checked = 0
    for relative, path in source_files(project):
        checked += 1
        with path.open(encoding='utf8', errors='replace') as source:
            lines = sum(1 for _ in source)
        limit = max(SOURCE_LINE_LIMIT, budgets.get(relative.as_posix(), 0))
        if lines > limit:
            violations.append({'path': relative.as_posix(), 'lines': lines, 'limit': limit})
    return {'checked': checked, 'line_limit': SOURCE_LINE_LIMIT, 'violations': violations}


def package(profile):
    if profile == 'auxiliary':
        return {'language': 'typescript', 'path': profile, 'name': '@poolster/layout-probe',
                'plugins': [{'name': 'sdk'}, *[{'name': name, 'output': name} for name in AUXILIARY]]}
    return {'language': profile, 'path': profile, 'name': 'layout-probe',
            'plugins': [{'name': 'cli' if profile.endswith('-cli') else 'sdk', **({'jobs': 1} if profile == 'go' else {})}]}


def file_digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest() if path and path.is_file() else None


def native_check(profile, output, environment, log, timeout, node_modules=None):
    if profile == 'go':
        return corpus.run_logged(['go', '-C', str(output), 'test', './...'], environment, log, timeout)
    if profile == 'rust-cli':
        return corpus.run_logged(['cargo', 'check', '--quiet', '--manifest-path', str(output / 'Cargo.toml')],
                                 environment, log, timeout)
    if not node_modules:
        return {'status': 'unavailable', 'reason': 'supply_node_modules_for_native_typescript_check'}
    (output / 'node_modules').symlink_to(node_modules.resolve(), target_is_directory=True)
    config = output / 'tsconfig.json'
    if not config.exists():
        config = output / '.layout-audit-tsconfig.json'
        config.write_text(json.dumps({'compilerOptions': {'target': 'ES2022', 'module': 'NodeNext',
                          'moduleResolution': 'NodeNext', 'strict': True, 'skipLibCheck': True,
                          'noEmit': True, 'esModuleInterop': True}, 'include': ['**/*.ts', '.poolster/**/*.ts']}))
    return corpus.run_logged(['node', str(node_modules / 'typescript/lib/tsc.js'), '-p', str(config), '--noEmit'],
                             environment, log, timeout)


def audit_case(source, name, profile, root, binary, environment, timeout, warning_bytes,
               native=False, node_modules=None):
    case_root = root / name / profile
    case_root.mkdir(parents=True)
    config = {'openapi': {'input': str(source.resolve()), 'name': 'Layout Probe', 'version': '1.0.0'},
              'output': {'path': str(case_root / 'generated')}, 'packages': [package(profile)]}
    config_path = case_root / 'poolster.json'
    config_path.write_text(json.dumps(config, indent=2) + '\n')
    command = [str(binary), 'generate', '--config', str(config_path), '--color', 'never']
    first = corpus.run_logged(command, environment, case_root / 'generate.log', timeout)
    result = {'contract': name, 'profile': profile, 'source_sha256': file_digest(source),
              'source_spec_bytes': source.stat().st_size, 'generation': first, 'status': 'failed'}
    if first['exit_code'] != 0:
        return result
    output = case_root / 'generated' / profile
    result.update(corpus.output_statistics(output, warning_bytes))
    result['output_to_input_ratio'] = round(result['generated_bytes'] / result['source_spec_bytes'], 3)
    before = corpus.output_fingerprint(output)
    # Authored files are part of the owned-workspace regeneration test.
    sentinel = output / 'layout-audit-authored.txt'
    sentinel.write_text('customer-owned overlay\n')
    repeat = corpus.run_logged(command, environment, case_root / 'regenerate.log', timeout)
    result['authored_file_preserved'] = sentinel.is_file() and sentinel.read_text() == 'customer-owned overlay\n'
    if sentinel.exists():
        sentinel.unlink()
    comparison = corpus.compare_fingerprints(before, corpus.output_fingerprint(output))
    result['regeneration'] = {**comparison, 'generation': repeat}
    # A fresh workspace uses a different worker count and output root.
    config['output']['path'] = str(case_root / 'fresh')
    if profile == 'go':
        config['packages'][0]['plugins'][0]['jobs'] = 2
    config_path.write_text(json.dumps(config, indent=2) + '\n')
    fresh_command = [str(binary), 'generate', '--config', str(config_path), '--color', 'never']
    fresh = corpus.run_logged(fresh_command, environment, case_root / 'fresh.log', timeout)
    result['worker_determinism'] = {'worker_counts': [1, 2] if profile == 'go' else None,
                                    'scope': 'worker_counts' if profile == 'go' else 'fresh_workspace',
                                    **corpus.compare_fingerprints(
        before, corpus.output_fingerprint(case_root / 'fresh' / profile)), 'generation': fresh}
    if profile == 'auxiliary':
        result['auxiliary_outputs'] = {name: corpus.output_statistics(output / name, warning_bytes)
                                       for name in AUXILIARY}
    if native:
        result['native'] = native_check(profile, output, environment, case_root / 'native.log',
                                        timeout, node_modules)
    else:
        result['native'] = {'status': 'unavailable', 'reason': 'native_check_not_requested'}
    successful = (repeat['exit_code'] == 0 and fresh['exit_code'] == 0 and
                  comparison['status'] == 'passed' and
                  result['worker_determinism']['status'] == 'passed' and
                  result['authored_file_preserved'])
    if native:
        successful = successful and result['native'].get('exit_code') == 0
    result['status'] = 'passed' if successful else 'failed'
    shutil.rmtree(case_root / 'fresh', ignore_errors=True)
    return result


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check-source-size', action='store_true',
                        help='check authored source against the 400-line limit and legacy ratchet')
    parser.add_argument('--source-size-baseline', type=Path,
                        default=Path(__file__).with_name('source-size-baseline.json'))
    parser.add_argument('--binary', type=Path, default=Path(os.environ.get('POOLSTER_BINARY', corpus.PROJECT / 'target/debug/poolster')))
    parser.add_argument('--manifest', type=Path, default=corpus.PROJECT / 'scripts/fixtures/guru-contracts.json')
    parser.add_argument('--spec-cache', type=Path)
    parser.add_argument('--contracts', default='stripe,github,mailchimp,azure-compute')
    parser.add_argument('--local-spec', action='append', default=[], metavar='NAME=PATH',
                        help='include a local source, for example a separately pinned Microsoft Graph specification')
    parser.add_argument('--profiles', default=','.join(PROFILES))
    parser.add_argument('--timeout', type=int, default=600)
    parser.add_argument('--file-warning-bytes', type=int, default=262144)
    parser.add_argument('--native', action='store_true')
    parser.add_argument('--node-modules', type=Path)
    args = parser.parse_args(arguments)
    if args.check_source_size:
        report = source_size_audit(corpus.PROJECT, args.source_size_baseline)
        for violation in report['violations']:
            print(f"{violation['path']}: {violation['lines']} lines exceeds {violation['limit']}")
        print(f"Checked {report['checked']} authored source files; "
              f"{len(report['violations'])} size violations")
        return int(bool(report['violations']))
    if args.output is None:
        parser.error('--output is required for generated layout audits')
    profiles = args.profiles.split(',')
    if any(profile not in PROFILES for profile in profiles):
        parser.error('unknown profile')
    if not 1 <= args.timeout <= 3600 or args.file_warning_bytes < 0:
        parser.error('invalid timeout or warning budget')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    environment = dict(os.environ)
    sources = []
    provenance = {}
    if args.contracts:
        os.environ['POOLSTER_PUBLIC_CONTRACTS'] = args.contracts
        corpus.public.fetch(args.manifest, root / 'specs', args.spec_cache)
        for contract in corpus.public.contracts(args.manifest):
            sources.append((contract['name'], corpus.public.source_path(root / 'specs', contract)))
            provenance[contract['name']] = {'kind': 'pinned_manifest', 'url': contract['url'],
                                           'sha256': contract['sha256']}
    for entry in args.local_spec:
        name, separator, path = entry.partition('=')
        if not separator or not name or Path(name).name != name or name in ('.', '..'):
            parser.error('--local-spec must be NAME=PATH with a safe name')
        local = Path(path).resolve()
        copied = root / 'specs' / (name + '.local' + local.suffix)
        copied.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(local, copied)
        sources.append((name, copied))
        provenance[name] = {'kind': 'local_snapshot', 'original_path': str(local), 'sha256': file_digest(copied)}
    if not sources:
        parser.error('no sources selected')
    if len({name for name, _ in sources}) != len(sources):
        parser.error('duplicate source names')
    report = {'schema_version': 1, 'host_platform': sys.platform,
              'manifest': str(args.manifest.resolve()), 'source_provenance': provenance,
              'environment': {key: environment[key] for key in
                              ('POOLSTER_OPENAPI_BIN', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS',
                               'CARGO_NET_OFFLINE', 'GOCACHE') if key in environment},
              'binary_sha256': file_digest(args.binary),
              'compiler_sha256': file_digest(Path(environment.get('POOLSTER_OPENAPI_BIN', args.binary.parent / 'poolster-openapi'))),
              'cases': []}
    for name, source in sources:
        for profile in profiles:
            result = audit_case(source, name, profile, root, args.binary.resolve(), environment,
                                args.timeout, args.file_warning_bytes, args.native, args.node_modules)
            report['cases'].append(result)
            report['passed'] = sum(c['status'] == 'passed' for c in report['cases'])
            report['failed'] = len(report['cases']) - report['passed']
            staged = root / 'report.json.tmp'
            staged.write_text(json.dumps(report, indent=2) + '\n')
            staged.replace(root / 'report.json')
            print(f'{name} / {profile}: {result["status"]}', flush=True)
    return int(bool(report['failed']))


if __name__ == '__main__':
    sys.exit(main())
