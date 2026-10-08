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


def package(profile):
    if profile == 'auxiliary':
        return {'language': 'typescript', 'path': profile, 'name': '@kaji/layout-probe',
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
                          'noEmit': True, 'esModuleInterop': True}, 'include': ['**/*.ts', '.kaji/**/*.ts']}))
    return corpus.run_logged(['node', str(node_modules / 'typescript/lib/tsc.js'), '-p', str(config), '--noEmit'],
                             environment, log, timeout)


def audit_case(source, name, profile, root, binary, environment, timeout, warning_bytes,
               native=False, node_modules=None):
    case_root = root / name / profile
    case_root.mkdir(parents=True)
    config = {'openapi': {'input': str(source.resolve()), 'name': 'Layout Probe', 'version': '1.0.0'},
              'output': {'path': str(case_root / 'generated')}, 'packages': [package(profile)]}
    config_path = case_root / 'kaji.json'
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
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, default=Path(os.environ.get('KAJI_BINARY', corpus.PROJECT / 'target/debug/kaji')))
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
        os.environ['KAJI_PUBLIC_CONTRACTS'] = args.contracts
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
                              ('KAJI_OPENAPI_BIN', 'CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS',
                               'CARGO_NET_OFFLINE', 'GOCACHE') if key in environment},
              'binary_sha256': file_digest(args.binary),
              'compiler_sha256': file_digest(Path(environment.get('KAJI_OPENAPI_BIN', args.binary.parent / 'kaji-openapi'))),
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
