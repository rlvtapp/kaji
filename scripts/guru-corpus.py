"""Compile a pinned APIs.guru corpus; never call the described production APIs."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

PROJECT = Path(__file__).resolve().parent.parent
LANGUAGES = ('rust', 'typescript', 'go', 'python', 'php', 'java', 'csharp', 'elixir', 'ruby', 'swift')
loader = importlib.util.spec_from_file_location('public_contracts', PROJECT / 'scripts/public-contracts.py')
public = importlib.util.module_from_spec(loader)
loader.loader.exec_module(public)


def run_logged(command, environment, log, timeout):
    started = time.monotonic()
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open('w') as writer:
        try:
            process = subprocess.Popen(command, env=environment, stdout=writer,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            code = process.wait(timeout=timeout)
            return {'exit_code': code, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': False}
        except subprocess.TimeoutExpired:
            # A shell phase owns compiler children too. Stop the whole owned
            # group before removing its generated workspace.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
            writer.write('\nCorpus phase exceeded its time limit.\n')
            return {'exit_code': None, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': True}
        except OSError as error:
            writer.write(str(error) + '\n')
            return {'exit_code': None, 'seconds': round(time.monotonic() - started, 3),
                    'log': str(log), 'timed_out': False}



SOURCE_SUFFIXES = frozenset(('.rs', '.ts', '.tsx', '.go', '.py', '.php', '.java', '.cs', '.ex', '.exs', '.rb', '.swift'))


def output_statistics(package, warning_bytes=0):
    """Measure owned output before native checks create build artifacts.

    Metadata is reported separately; JSON collections and other assets remain
    in total bytes. Budgets are reporting heuristics, never compiler limits.
    """
    files = []
    for path in package.rglob('*'):
        if path.is_file():
            relative = path.relative_to(package)
            files.append((relative.as_posix(), path.stat().st_size,
                          '.kaji' in relative.parts, path.suffix in SOURCE_SUFFIXES))
    source = sorted(((name, size) for name, size, metadata, is_source in files
                     if is_source and not metadata), key=lambda item: (-item[1], item[0]))
    oversized = sorted(((name, size) for name, size, metadata, _ in files
                        if not metadata and warning_bytes and size > warning_bytes),
                       key=lambda item: (-item[1], item[0]))
    return {
        'generated_files': len(files),
        'generated_bytes': sum(size for _, size, _, _ in files),
        'source_files': len(source),
        'source_bytes': sum(size for _, size in source),
        'metadata_bytes': sum(size for _, size, metadata, _ in files if metadata),
        'largest_source_bytes': source[0][1] if source else 0,
        'largest_source_files': [{'path': name, 'bytes': size} for name, size in source[:10]],
        'file_warning_bytes': warning_bytes or None,
        'oversized_files': [{'path': name, 'bytes': size} for name, size in oversized],
    }


def run_case(contract, language, manifest, root, environment, timeout, keep_generated, warning_bytes=0):
    name = contract['name']
    report = {'contract': name, 'language': language, 'source_url': contract['url'],
              'source_sha256': contract['sha256'], 'input_bytes': contract.get('bytes'),
              'status': 'failed', 'generation': None, 'native': None}
    with tempfile.TemporaryDirectory(prefix=name + '-', dir=root / 'work') as temporary:
        scratch = Path(temporary)
        env = {**environment, 'KAJI_PUBLIC_CONTRACT_MANIFEST': str(manifest),
               'KAJI_PUBLIC_CONTRACTS': name, 'KAJI_PUBLIC_LANGUAGES': language,
               'KAJI_PUBLIC_SPEC_DIR': str(root / 'specs'),
               'KAJI_PUBLIC_CONTRACT_ROOT': str(scratch)}
        logs = root / 'logs' / name / language
        report['generation'] = run_logged(['bash', str(PROJECT / 'scripts/test-public-contracts.sh'),
                                          'generate'], env, logs / 'generate.log', timeout)
        sdk_root = scratch / name
        if report['generation']['exit_code'] == 0:
            package = sdk_root / language
            report.update(output_statistics(package, warning_bytes))
            report['native'] = run_logged(['bash', str(PROJECT / 'scripts/test-public-contracts.sh'),
                                          'check', language], env, logs / 'native.log', timeout)
            if report['native']['exit_code'] == 0:
                report['status'] = 'passed'
        for metadata in (sdk_root / '.kaji', sdk_root / language / '.kaji'):
            if metadata.is_dir():
                relative = metadata.relative_to(sdk_root)
                shutil.copytree(metadata, root / 'metadata' / name / language / relative,
                                dirs_exist_ok=True)
        if keep_generated and sdk_root.exists():
            destination = root / 'generated' / name / language
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.move(str(sdk_root), destination)
    return report


def save_report(root, manifest, cases):
    result = {'schema_version': 1, 'manifest': str(manifest),
              'passed': sum(case['status'] == 'passed' for case in cases),
              'failed': sum(case['status'] != 'passed' for case in cases), 'cases': cases}
    destination = root / 'report.json'
    staged = root / 'report.json.tmp'
    staged.write_text(json.dumps(result, indent=2) + '\n')
    staged.replace(destination)
    return result


def main(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=PROJECT / 'scripts/fixtures/guru-contracts.json')
    parser.add_argument('--output', type=Path, required=True, help='new disposable directory')
    parser.add_argument('--language', choices=LANGUAGES, required=True)
    parser.add_argument('--contracts', default='', help='comma-separated pinned names; default all')
    parser.add_argument('--spec-cache', type=Path)
    parser.add_argument('--timeout', type=int, default=600, help='seconds per generation/native phase')
    parser.add_argument('--keep-generated', action='store_true')
    parser.add_argument('--file-warning-bytes', type=int, default=0,
                        help='report generated files above this size; 0 disables warnings')
    args = parser.parse_args(arguments)
    if not 1 <= args.timeout <= 3600:
        parser.error('--timeout must be between 1 and 3600 seconds')
    if args.file_warning_bytes < 0:
        parser.error('--file-warning-bytes cannot be negative')
    manifest = args.manifest.resolve()
    if args.contracts:
        os.environ['KAJI_PUBLIC_CONTRACTS'] = args.contracts
    selected = public.contracts(manifest)
    if not selected:
        parser.error('no contracts selected')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    (root / 'work').mkdir()
    (root / 'generated').mkdir()
    try:
        public.fetch(manifest, root / 'specs', args.spec_cache)
    except (ValueError, OSError) as error:
        (root / 'fetch-error.txt').write_text(str(error) + '\n')
        raise
    cases = []
    for contract in selected:
        case = run_case(contract, args.language, manifest, root, dict(os.environ),
                        args.timeout, args.keep_generated, args.file_warning_bytes)
        cases.append(case)
        result = save_report(root, manifest, cases)
        print(f"{contract['name']} / {args.language}: {case['status']}", flush=True)
    print(f"Passed {result['passed']}; failed {result['failed']}; report: {root / 'report.json'}")
    return 1 if result['failed'] else 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError) as error:
        sys.exit(str(error))
