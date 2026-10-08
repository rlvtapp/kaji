#!/usr/bin/env python3
"""Publish the public workspace graph in dependency order; safe to resume."""
import argparse
import json
import subprocess
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def publication_order(metadata):
    members = set(metadata['workspace_members'])
    packages = {p['name']: p for p in metadata['packages']
                if p['id'] in members and p['publish'] != []}
    ordered, visiting, visited = [], set(), set()

    def visit(name):
        if name in visited:
            return
        if name in visiting:
            raise ValueError(f'Publication dependency cycle at {name}')
        visiting.add(name)
        for dependency in packages[name]['dependencies']:
            dep = dependency['name']
            # Versioned dev dependencies remain in published manifests and are
            # resolved while Cargo prepares the archive's registry lockfile.
            # Path-only dev dependencies use req='*' and are omitted by Cargo.
            published_dev = dependency.get('kind') == 'dev' and dependency.get('req', '*') != '*'
            if dependency.get('path') and (dependency.get('kind') != 'dev' or published_dev):
                if dep not in packages:
                    raise ValueError(f'{name} depends on unpublished crate {dep}')
                visit(dep)
        visiting.remove(name)
        visited.add(name)
        ordered.append(packages[name])

    for name in sorted(packages):
        visit(name)
    return ordered


def already_published(name, version):
    request = urllib.request.Request(
        f'https://crates.io/api/v1/crates/{name}/{version}',
        headers={'User-Agent': 'poolster-release (https://github.com/rlvtapp/poolster)'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = json.load(response)
            if data['version'].get('yanked'):
                raise RuntimeError(f'{name}@{version} is yanked; refusing to skip it')
            return True
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return False
        raise


def publish(packages, exists=already_published, run=subprocess.run):
    for package in packages:
        name, version = package['name'], package['version']
        if exists(name, version):
            print(f'Skip published {name}@{version}', flush=True)
            continue
        print(f'Publish {name}@{version}', flush=True)
        # Cargo verifies the archive and waits for index availability before returning.
        run(['cargo', 'publish', '--locked', '--registry', 'crates-io',
             '--package', name], cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', action='store_true', help='Print order without registry access or uploads')
    parser.add_argument('--tag', help='Require every crate version to match this v-prefixed release tag')
    args = parser.parse_args()
    result = subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version', '1', '--locked'],
                            cwd=ROOT, check=True, capture_output=True, text=True)
    packages = publication_order(json.loads(result.stdout))
    if args.tag and any('v' + package['version'] != args.tag for package in packages):
        parser.error('Release tag does not match all public crate versions')
    if not args.plan and not args.tag:
        parser.error('Publishing requires --tag v<version>')
    if args.plan:
        for package in packages:
            print(f"{package['name']}@{package['version']}")
    else:
        publish(packages)


if __name__ == '__main__':
    main()
