# Multipart uploads

Multipart operations accept `MultipartBody` or a dictionary. JSON model declarations stay unchanged; multipart input is a separate runtime representation so union/file schemas do not force JSON serialization of files.

```python
from your_sdk.multipart import MultipartBody, FilePart, JsonPart, RawJsonPart

body = MultipartBody({
    "file": FilePart(b"audio bytes", filename="recording.wav", content_type="audio/wav"),
    "model": "transcription-model",
    "enabled": False,
    "count": 0,
    "metadata": JsonPart({"language": "日本語"}),
})
result = client.create_transcription(body=body)
# Async SDKs use the same body: await async_client.create_transcription(body=body).
```

For operations declaring both JSON and multipart, dictionaries and generated models use JSON; pass `MultipartBody` explicitly to select multipart, regardless of declaration order. Operations that do not declare multipart reject `MultipartBody`.

Use your operation's actual name and fields. Scalar text is UTF-8, booleans use `true`/`false`, and zero/empty strings remain present. `None` omits a part; `JsonPart(None)` explicitly sends JSON null. Dictionaries/lists become JSON parts, and `RawJsonPart` validates UTF-8 JSON while preserving its bytes exactly. Direct bytes-like values become file parts with filename `blob`. To repeat a field, pass an ordered sequence such as `MultipartBody([("file", first_file), ("file", second_file)])`.

File bytes are copied from mutable input buffers. The encoder builds the complete payload once per logical request, then sync urllib and async HTTPX retries reuse identical bytes and boundary. The multipart encoder owns the Content-Type boundary; customer middleware can inspect the resulting native request. Per-call headers and timeouts remain available through `client.for_call(...)`.

The default encoded payload limit is 64 MiB with at most 1,024 parts. Set `MultipartBody(..., max_bytes=...)` to change the payload limit and `FilePart(..., max_bytes=...)` to change the file buffer limit. Multipart is buffered; file handles and streaming upload readers are unsupported. Field names, filenames and media types are validated before execution to prevent header injection. Unicode filenames and text are preserved.
