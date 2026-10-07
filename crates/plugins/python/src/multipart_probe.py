import asyncio
import json
import threading
from email import policy
from email.parser import BytesParser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from multipart_sdk.models import VoiceOptions
from multipart_sdk import Client, AsyncClient, MultipartBody, FilePart, JsonPart, RawJsonPart

seen = []
counts = {}
class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_): pass
    def do_POST(self):
        data = self.rfile.read(int(self.headers['Content-Length']))
        marker = self.headers['X-Probe']
        seen.append((marker, data, self.headers['Content-Type']))
        counts[marker] = counts.get(marker, 0) + 1
        self.send_response(503 if counts[marker] == 1 else 200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(b'{}')
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
thread = threading.Thread(target=server.serve_forever, daemon=True)
thread.start()
try:
    mutable = bytearray(b'\x00\xff\r\nfile')
    file = FilePart(mutable, filename='雪.txt', content_type='application/octet-stream')
    mutable[0] = 99
    body = MultipartBody([
        ('file', file), ('text', 'snow 雪'), ('enabled', False), ('count', 0), ('missing', None),
        ('metadata', JsonPart({'array': [0, False, None]})), ('raw', RawJsonPart(b'{"raw":1.00}')),
        ('tag', 'first'), ('tag', 'second'), ('null', JsonPart(None)),
    ])
    base_url = f'http://127.0.0.1:{server.server_port}'
    client = Client(base_url=base_url, headers={'X-Probe':'sync'}, max_retries=1, retry_initial_delay=0)
    assert client.for_call(headers={'content-type':'wrong'}).create_transcription(body=body, idempotency_key='stable') == {}
    assert client.for_call(headers={'X-Probe':'sync-mixed'}).create_voice(body=body, idempotency_key='stable') == {}
    assert client.for_call(headers={'X-Probe':'sync-json'}).create_voice(body={'normal':False,'count':0}, idempotency_key='stable') == {}
    before = len(seen)
    try: client.json_only(body=body, idempotency_key='stable')
    except TypeError: pass
    else: raise AssertionError('JSON-only operation accepted multipart')
    assert len(seen) == before
    async def run():
        async with AsyncClient(base_url=base_url, headers={'X-Probe':'async'}, max_retries=1, retry_initial_delay=0) as client:
            assert await client.for_call(headers={'content-type':'wrong'}).create_transcription(body=body, idempotency_key='stable') == {}
            assert await client.for_call(headers={'X-Probe':'async-mixed'}).create_voice(body=body, idempotency_key='stable') == {}
            assert await client.for_call(headers={'X-Probe':'async-json'}).create_voice(body=VoiceOptions(normal=False,count=0), idempotency_key='stable') == {}
            before = len(seen)
            try: await client.json_only(body=body, idempotency_key='stable')
            except TypeError: pass
            else: raise AssertionError('JSON-only operation accepted multipart')
            assert len(seen) == before
    asyncio.run(run())
    for marker in ('sync-json', 'async-json'):
        requests = [(data, content_type) for role, data, content_type in seen if role == marker]
        assert len(requests) == 2 and requests[0] == requests[1]
        assert requests[0][1] == 'application/json'
        assert json.loads(requests[0][0]) == {'normal':False,'count':0}
    for marker in ('sync', 'async', 'sync-mixed', 'async-mixed'):
        requests = [(data, content_type) for role, data, content_type in seen if role == marker]
        assert len(requests) == 2 and requests[0] == requests[1], 'retry changed multipart bytes or boundary'
        data, content_type = requests[0]
        assert content_type.startswith('multipart/form-data; boundary=kaji-')
        message = BytesParser(policy=policy.default).parsebytes(f'Content-Type: {content_type}\r\nMIME-Version: 1.0\r\n\r\n'.encode() + data)
        assert message.is_multipart()
        parts = list(message.iter_parts())
        fields = {part.get_param('name', header='content-disposition'): part for part in parts}
        assert 'missing' not in fields
        assert fields['file'].get_payload(decode=True) == b'\x00\xff\r\nfile'
        assert fields['file'].get_filename() == '雪.txt'
        assert fields['text'].get_payload(decode=True).decode() == 'snow 雪'
        assert fields['enabled'].get_payload(decode=True) == b'false'
        assert fields['count'].get_payload(decode=True) == b'0'
        assert json.loads(fields['metadata'].get_payload(decode=True)) == {'array': [0, False, None]}
        assert fields['raw'].get_payload(decode=True) == b'{"raw":1.00}'
        assert fields['null'].get_payload(decode=True) == b'null'
        assert [part.get_payload(decode=True) for part in parts if part.get_param('name', header='content-disposition') == 'tag'] == [b'first', b'second']
    for make in (
        lambda: FilePart(b'x', filename='bad\r\nInjected: yes'),
        lambda: FilePart(b'x', content_type='x/y\r\nInjected: yes'),
        lambda: MultipartBody({'bad\r\nname':'x'}),
        lambda: MultipartBody({'file':b'x'*100}, max_bytes=10),
        lambda: MultipartBody({'number':float('nan')}).encode(),
        lambda: RawJsonPart(b'{"bad":NaN}'),
        lambda: MultipartBody({'file':file}, max_bytes=10).encode(),
    ):
        try: make()
        except (TypeError, ValueError): pass
        else: raise AssertionError('invalid multipart input accepted')
finally:
    server.shutdown()
    server.server_close()
    thread.join()
