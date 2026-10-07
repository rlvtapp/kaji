# Buffered multipart uploads

Multipart-only operation inputs use `Body *KajiMultipartBody`. For operations with JSON and multipart alternatives, `Body` accepts the generated JSON model or an explicit multipart body.

```go
body := &KajiMultipartBody{}
body.AddFile("file", "recording.wav", "audio/wav", fileBytes)
body.AddText("model", "transcription-model")
body.AddText("items[]", "first")
body.AddText("items[]", "second")
if err := body.AddJSON("metadata", map[string]any{"enabled": false, "count": 0}); err != nil {
    return err
}
// Use your generated operation/request name:
// result, err := client.CreateTranscription(ctx, &CreateTranscriptionRequest{Body: body})
```

Part names are explicit wire names. Repeating a name emits repeated parts; choose JSON or joined text explicitly when your API declares another encoding. `AddFile` copies its bytes. `AddJSON` uses the generated model's ordinary JSON marshaler. `KajiMultipartPart.Headers` supports extra per-part headers; Content-Type is selected with the part field and Content-Disposition is encoder-owned.

The final payload is bounded to 64 MiB and 10,000 parts. Files are buffered, with no filesystem or streaming-reader access. MIME names, filenames, content types and headers are validated before sending. Unicode filenames/text and binary bytes are preserved. Safe retries reuse the same complete bytes and boundary; unsafe mutations retain the ordinary idempotency guard. Go request context cancellation, scoped headers, middleware and OAuth still apply.

This is an explicit part builder, not automatic schema-to-form conversion. The API still validates required fields, unions and encoding choices. Generated operation smoke tests report unsupported multipart samples separately; native MIME/retry tests verify the maintained encoder.
