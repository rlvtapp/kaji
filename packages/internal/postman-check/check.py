"""Validate exports; never execute collection scripts or send requests."""
import json
import os
from pathlib import Path


def checkout_file(root, name):
    if not name or Path(name).is_absolute() or '..' in Path(name).parts:
        raise ValueError('Export path must stay within the checkout')
    root = Path(root).resolve()
    candidate = root
    for part in Path(name).parts:
        candidate = candidate / part
        if candidate.is_symlink():
            raise ValueError('Export paths must not traverse symlinks')
    resolved = candidate.resolve(strict=True)
    if not resolved.is_relative_to(root) or not resolved.is_file():
        raise ValueError('Export path must be a checkout file')
    return resolved


def check(collection, environment=None):
    from jsonschema import Draft4Validator
    schema = json.loads(Path(__file__).with_name('collection-v2.1.0.json').read_text())
    Draft4Validator.check_schema(schema)
    # Surface only paths, never instance values which might contain credentials.
    errors = sorted(Draft4Validator(schema).iter_errors(collection), key=lambda error: str(list(error.path)))
    if errors:
        raise ValueError('Collection schema validation failed at ' + ', '.join('/'.join(map(str, error.path)) or '<root>' for error in errors[:10]))
    ids = set()
    def items(entries):
        for item in entries:
            identifier = item.get('id')
            if 'request' in item:
                if not identifier or identifier in ids:
                    raise ValueError('Requests require unique stable IDs')
                ids.add(identifier)
            if 'item' in item:
                items(item['item'])
    items(collection['item'])
    if environment is not None:
        if not isinstance(environment, dict) or not isinstance(environment.get('values'), list):
            raise ValueError('Invalid environment template')
        names = set()
        for item in environment['values']:
            if not isinstance(item, dict) or not isinstance(item.get('key'), str) or not item['key'] or item['key'] in names:
                raise ValueError('Environment requires unique variable keys')
            names.add(item['key'])
            if item.get('type') == 'secret' and item.get('value') != '':
                raise ValueError('Environment secret values must be blank')
    return len(ids)


def main():
    root = os.environ.get('GITHUB_WORKSPACE', os.getcwd())
    collection = json.loads(checkout_file(root, os.environ['POOLSTER_COLLECTION']).read_text())
    name = os.environ.get('POOLSTER_ENVIRONMENT')
    environment = json.loads(checkout_file(root, name).read_text()) if name else None
    count = check(collection, environment)
    print(f'Validated {count} Postman requests; no API requests executed')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        # JSON parser/schema exceptions can embed instance contents; don't log them.
        print(str(error) if isinstance(error, ValueError) and not isinstance(error, json.JSONDecodeError) else 'Export validation failed')
        raise SystemExit(1)
