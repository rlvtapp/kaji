"""Distinct-connection graphql-sse and multipart deferSpec=20220824, no replay."""
import json
from urllib.request import Request, urlopen

def sse_events(response, max_event_bytes):
    event, data, size = '', [], 0
    while True:
        raw = response.readline(max_event_bytes + 1)
        if not raw:
            raise ValueError('GraphQL SSE closed before complete')
        line = raw.decode('utf-8').rstrip('\r\n')
        if line.startswith(':'):
            continue
        size += len(raw)
        if size > max_event_bytes:
            raise ValueError('GraphQL SSE event exceeds limit')
        if not line:
            if event == 'complete':
                return
            if event == 'next':
                yield json.loads('\n'.join(data))
            elif event:
                raise ValueError('Unsupported GraphQL SSE event')
            event, data, size = '', [], 0
        elif not line.startswith(':'):
            field, separator, value = line.partition(':')
            if value.startswith(' '):
                value = value[1:]
            if field == 'event':
                event = value
            elif field == 'data':
                data.append(value)

def multipart_frames(response, max_frame_bytes):
    import email.message
    header = email.message.Message()
    header['content-type'] = response.headers.get('Content-Type', '')
    if header.get_content_type() != 'multipart/mixed' or header.get_param('deferspec') != '20220824':
        raise ValueError('Incremental delivery requires multipart/mixed; deferSpec=20220824')
    boundary = header.get_param('boundary')
    if not boundary or '\r' in boundary or '\n' in boundary:
        raise ValueError('Missing or invalid multipart boundary')
    delimiter = ('--' + boundary).encode()
    while True:
        line = response.readline(max_frame_bytes + 1)
        if not line:
            raise ValueError('Multipart response closed before boundary')
        if line.rstrip(b'\r\n') == delimiter:
            break
    while True:
        size = 0
        headers = []
        while True:
            line = response.readline(max_frame_bytes + 1)
            size += len(line)
            if not line or size > max_frame_bytes:
                raise ValueError('Malformed or oversized multipart headers')
            if not line.strip():
                break
            headers.append(line.decode('ascii'))
        if not any(line.lower().startswith('content-type: application/json') for line in headers):
            raise ValueError('Incremental parts require application/json')
        body = []
        while True:
            line = response.readline(max_frame_bytes + 1)
            if not line:
                raise ValueError('Multipart response closed before boundary')
            if line.rstrip(b'\r\n') in (delimiter, delimiter + b'--'):
                break
            size += len(line)
            if size > max_frame_bytes:
                raise ValueError('Incremental frame exceeds limit')
            body.append(line)
        value = json.loads(b''.join(body))
        if not isinstance(value, dict) or not isinstance(value.get('hasNext'), bool):
            raise ValueError('Incremental frames require boolean hasNext')
        if 'pending' in value or 'completed' in value:
            raise ValueError('Unsupported incremental delivery dialect')
        terminal = line.rstrip(b'\r\n') == delimiter + b'--'
        if terminal:
            if value['hasNext']:
                raise ValueError('Incremental response closed with hasNext=true')
            yield value
            return
        if not value['hasNext']:
            raise ValueError('Frames follow hasNext=false')
        yield value

def apply_incremental(snapshot, frame):
    import copy
    snapshot = copy.deepcopy(snapshot)
    for patch in frame.get('incremental', []):
        path = patch.get('path')
        if not isinstance(path, list) or any(not isinstance(key, (str, int)) or isinstance(key, bool) or (isinstance(key, int) and key < 0) for key in path):
            raise ValueError('Incremental patches require a valid path')
        target = snapshot
        for key in path[:-1]:
            target = target[key]
        key = path[-1] if path else None
        if 'data' in patch:
            if key is None:
                if isinstance(snapshot, dict) and isinstance(patch['data'], dict):
                    snapshot.update(patch['data'])
                else:
                    snapshot = patch['data']
            elif isinstance(target[key], dict) and isinstance(patch['data'], dict):
                target[key].update(patch['data'])
            else:
                target[key] = patch['data']
        elif 'items' in patch:
            if not path or not isinstance(key, int) or not isinstance(target, list) or not isinstance(patch['items'], list) or key != len(target):
                raise ValueError('Stream patches must append list items at the next index')
            target.extend(patch['items'])
        elif not patch.get('errors'):
            raise ValueError('Incremental patch requires data, items or errors')
    return snapshot
