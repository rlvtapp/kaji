//! HTTP runtime assembly and declared resource construction.
use crate::*;

pub(crate) fn render_client(api: &Api, namespace: &str, client_style: SdkClientStyle) -> String {
    let mut output = format!(
        "{NOTICE}\nusing System.Globalization;\nusing System.Net.Http.Json;\nusing System.Text;\nusing System.Text.Json;\nusing System.Text.Json.Serialization;\n\nnamespace {namespace};\n\n/// <summary>Configures a <see cref=\"PoolsterClient\"/> instance.</summary>\npublic sealed record PoolsterClientOptions\n{{\n    /// <summary>Absolute API base URL, for example <c>https://api.example.com</c>.</summary>\n    public required string BaseUrl {{ get; init; }}\n\n    public string? ApiKey {{ get; init; }}\n\n    public string ApiKeyHeader {{ get; init; }} = \"Authorization\";\n\n    /// <summary>Prefix sent before the API key, such as <c>Bearer</c>.</summary>\n    public string? ApiKeyPrefix {{ get; init; }} = \"Bearer\";\n}}\n\n/// <summary>Typed asynchronous client for {}.</summary>\npublic sealed class PoolsterClient\n{{\n    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web)\n    {{\n        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,\n    }};\n\n    private readonly HttpClient _httpClient;\n    private readonly Uri _baseUri;\n    private readonly string? _apiKey;\n    private readonly string _apiKeyHeader;\n    private readonly string? _apiKeyPrefix;\n\n    public PoolsterClient(HttpClient httpClient, PoolsterClientOptions options)\n    {{\n        _httpClient = httpClient ?? throw new ArgumentNullException(nameof(httpClient));\n        ArgumentNullException.ThrowIfNull(options);\n        _baseUri = new Uri(options.BaseUrl.TrimEnd('/') + \"/\", UriKind.Absolute);\n        _apiKey = options.ApiKey;\n        _apiKeyHeader = options.ApiKeyHeader;\n        _apiKeyPrefix = options.ApiKeyPrefix;\n    }}\n\n",
        xml_escape(&api.name),
    );
    // Keep the public options in the generated client file so packages remain
    // dependency-free, while making retry and lifecycle policy explicit.
    output = output.replacen(
        "public sealed class PoolsterClient",
        "public sealed partial class PoolsterClient",
        1,
    );
    output = output.replacen(
        "using System.Net.Http.Json;",
        "using System.Net;\nusing System.Net.Http.Headers;\nusing System.Net.Http.Json;",
        1,
    );
    output = output.replacen(
        "    public string? ApiKeyPrefix { get; init; } = \"Bearer\";\n}",
        "    public string? ApiKeyPrefix { get; init; } = \"Bearer\";\n\n    /// <summary>Null selects Poolster's safe three-attempt policy; set MaxAttempts to 1 to disable retries.</summary>\n    public PoolsterRetryOptions? Retry { get; init; }\n\n    public IPoolsterClientHooks? Hooks { get; init; }\n}\n\n/// <summary>Configures retries for idempotent requests.</summary>\npublic sealed record PoolsterRetryOptions\n{\n    public int MaxAttempts { get; init; } = 3;\n    public TimeSpan InitialDelay { get; init; } = TimeSpan.FromMilliseconds(250);\n    public TimeSpan MaxDelay { get; init; } = TimeSpan.FromSeconds(8);\n}\n\n/// <summary>Optional lifecycle callbacks for package telemetry and policy.</summary>\npublic interface IPoolsterClientHooks\n{\n    void BeforeRequest(HttpRequestMessage request) { }\n    void AfterResponse(HttpResponseMessage response) { }\n    void OnError(Exception error) { }\n}",
        1,
    );
    output = output.replacen(
        "    private readonly string? _apiKeyPrefix;\n",
        "    private readonly string? _apiKeyPrefix;\n    private readonly PoolsterRetryOptions _retry;\n    private readonly IPoolsterClientHooks? _hooks;\n",
        1,
    );
    output = output.replacen(
        "        _apiKeyPrefix = options.ApiKeyPrefix;\n    }",
        "        _apiKeyPrefix = options.ApiKeyPrefix;\n        _retry = options.Retry ?? new PoolsterRetryOptions();\n        if (_retry.MaxAttempts < 1) throw new ArgumentOutOfRangeException(nameof(options.Retry), \"MaxAttempts must be at least 1.\");\n        _hooks = options.Hooks;\n    }",
        1,
    );
    if client_style == SdkClientStyle::Namespaced {
        for group in operation_groups(api) {
            let resource = group.clone();
            let _ = writeln!(
                output,
                "    /// <summary>Operations in the {group} resource.</summary>\n    public {resource}Resource {resource} {{ get; }}"
            );
        }
        output.push('\n');
        // The assignments must live in the constructor. Insert them after the
        // existing option assignments so all readonly properties are assigned.
        let assignments = operation_groups(api)
            .into_iter()
            .map(|group| {
                let resource = group.clone();
                format!("        {resource} = new {resource}Resource(this);\n")
            })
            .collect::<String>();
        output = output.replacen(
            "        _hooks = options.Hooks;\n    }",
            &format!("        _hooks = options.Hooks;\n{assignments}    }}"),
            1,
        );
    }
    output.push_str(
        "    private HttpRequestMessage CreateRequest(HttpMethod method, string path, List<KeyValuePair<string, string?>>? query = null)\n    {\n        var uri = new Uri(_baseUri, path.TrimStart('/'));\n        if (query is { Count: > 0 })\n        {\n            var encoded = string.Join(\"&\", query\n                .Where(item => item.Value is not null)\n                .Select(item => $\"{Uri.EscapeDataString(item.Key)}={Uri.EscapeDataString(item.Value!)}\"));\n            if (!string.IsNullOrEmpty(encoded))\n            {\n                var builder = new UriBuilder(uri) { Query = encoded };\n                uri = builder.Uri;\n            }\n        }\n\n        var request = new HttpRequestMessage(method, uri);\n        request.Headers.Accept.ParseAdd(\"application/json\");\n        if (!string.IsNullOrWhiteSpace(_apiKey))\n        {\n            var credential = string.IsNullOrWhiteSpace(_apiKeyPrefix)\n                ? _apiKey\n                : $\"{_apiKeyPrefix} {_apiKey}\";\n            request.Headers.TryAddWithoutValidation(_apiKeyHeader, credential);\n        }\n        return request;\n    }\n\n    private async Task<T> SendAsync<T>(HttpRequestMessage request, CancellationToken cancellationToken)\n    {\n        using (request)\n        using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);\n        var contents = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);\n        if (!response.IsSuccessStatusCode)\n        {\n            throw new ApiException((int)response.StatusCode, contents);\n        }\n        if (string.IsNullOrWhiteSpace(contents))\n        {\n            return default!;\n        }\n        return JsonSerializer.Deserialize<T>(NormalizeSequentialJson(contents, response.Content.Headers.ContentType?.MediaType), JsonOptions)\n            ?? throw new JsonException(\"Poolster received an empty JSON response.\");\n    }\n\n    private async Task SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)\n    {\n        using (request)\n        using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);\n        var contents = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);\n        if (!response.IsSuccessStatusCode)\n        {\n            throw new ApiException((int)response.StatusCode, contents);\n        }\n    }\n\n    private static string ParameterString<T>(T value)\n    {\n        return value switch\n        {\n            null => string.Empty,\n            bool boolean => boolean ? \"true\" : \"false\",\n            IFormattable formattable => formattable.ToString(null, CultureInfo.InvariantCulture) ?? string.Empty,\n            _ => value.ToString() ?? string.Empty,\n        };\n    }\n}\n",
    );
    // The base template above closes PoolsterClient. The retry helpers are part of
    // that class, so reopen the tail before adding them and restore it below.
    debug_assert!(output.ends_with("}\n"));
    output.truncate(output.len() - 2);
    output.push_str(
        "\n    // The template request is cloned for every attempt: HttpRequestMessage instances\n    // are single-use, and replaying an unsafe POST would be incorrect.\n    private async Task<T> SendWithRetryAsync<T>(HttpRequestMessage template, CancellationToken cancellationToken)\n    {\n        using (template)\n        {\n            var maxAttempts = IsRetryAllowed(template) ? Math.Max(1, _retry.MaxAttempts) : 1;\n            Exception? transportError = null;\n            for (var attempt = 0; attempt < maxAttempts; attempt++)\n            {\n                using var request = await CloneRequestAsync(template, cancellationToken).ConfigureAwait(false);\n                try\n                {\n                    _hooks?.BeforeRequest(request);\n                    using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);\n                    if (attempt + 1 < maxAttempts && IsRetryable(response.StatusCode))\n                    {\n                        await DelayForRetryAsync(attempt, response.Headers, cancellationToken).ConfigureAwait(false);\n                        continue;\n                    }\n                    var contents = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);\n                    if (!response.IsSuccessStatusCode)\n                    {\n                        var error = new ApiException((int)response.StatusCode, contents);\n                        _hooks?.OnError(error);\n                        throw error;\n                    }\n                    _hooks?.AfterResponse(response);\n                    if (string.IsNullOrWhiteSpace(contents)) return default!;\n                    return JsonSerializer.Deserialize<T>(NormalizeSequentialJson(contents, response.Content.Headers.ContentType?.MediaType), JsonOptions)\n                        ?? throw new JsonException(\"Poolster received an empty JSON response.\");\n                }\n                catch (HttpRequestException error) when (attempt + 1 < maxAttempts)\n                {\n                    transportError = error;\n                    await DelayForRetryAsync(attempt, null, cancellationToken).ConfigureAwait(false);\n                }\n                catch (Exception error)\n                {\n                    _hooks?.OnError(error);\n                    throw;\n                }\n            }\n            _hooks?.OnError(transportError ?? new HttpRequestException(\"Poolster retry loop completed without a response.\"));\n            throw transportError ?? new HttpRequestException(\"Poolster retry loop completed without a response.\");\n        }\n    }\n\n    private async Task SendWithRetryAsync(HttpRequestMessage template, CancellationToken cancellationToken)\n    {\n        await SendWithRetryAsync<JsonElement>(template, cancellationToken).ConfigureAwait(false);\n    }\n\n    private static bool IsRetryAllowed(HttpRequestMessage request)\n    {\n        return (request.Options.TryGetValue(new HttpRequestOptionsKey<string>(\"Poolster.IdempotencyHeader\"), out var header) && HasNonEmptyHeader(request, header)) || request.Method == HttpMethod.Get || request.Method == HttpMethod.Head || request.Method == HttpMethod.Options || request.Method == HttpMethod.Trace || request.Method.Method == \"QUERY\" || request.Method == HttpMethod.Put || request.Method == HttpMethod.Delete\n            || ((request.Method == HttpMethod.Post || request.Method == HttpMethod.Patch) && HasNonEmptyHeader(request, \"Idempotency-Key\"));\n    }\n\n    private static bool HasNonEmptyHeader(HttpRequestMessage request, string name)\n    {\n        return request.Headers.TryGetValues(name, out var values) && values.Any(value => !string.IsNullOrWhiteSpace(value));\n    }\n\n    private static bool IsRetryable(HttpStatusCode status)\n    {\n        return status is HttpStatusCode.RequestTimeout or (HttpStatusCode)429 or HttpStatusCode.InternalServerError or HttpStatusCode.BadGateway or HttpStatusCode.ServiceUnavailable or HttpStatusCode.GatewayTimeout;\n    }\n\n    private static double? RetryAfterMilliseconds(HttpResponseHeaders? headers, DateTimeOffset now)\n    {\n        if (headers is null) return null;\n        if (headers.TryGetValues(\"retry-after-ms\", out var values))\n        {\n            var value = values.FirstOrDefault();\n            if (double.TryParse(value, NumberStyles.Float, CultureInfo.InvariantCulture, out var milliseconds) && double.IsFinite(milliseconds) && milliseconds >= 0) return milliseconds;\n        }\n        var retryAfter = headers.RetryAfter;\n        if (retryAfter?.Delta is { } delta) return Math.Max(0, delta.TotalMilliseconds);\n        if (retryAfter?.Date is { } date) return Math.Max(0, (date - now).TotalMilliseconds);\n        return null;\n    }\n\n    private async Task DelayForRetryAsync(int attempt, HttpResponseHeaders? headers, CancellationToken cancellationToken)\n    {\n        // Task.Delay accepts at most uint.MaxValue - 1 milliseconds on .NET 8.\n        var cap = Math.Min(Math.Max(0, _retry.MaxDelay.TotalMilliseconds), uint.MaxValue - 1d);\n        var milliseconds = RetryAfterMilliseconds(headers, DateTimeOffset.UtcNow)\n            ?? Math.Max(0, _retry.InitialDelay.TotalMilliseconds) * Math.Pow(2, Math.Min(attempt, 30));\n        var delay = TimeSpan.FromMilliseconds(Math.Min(milliseconds, cap));\n        if (delay > TimeSpan.Zero) await Task.Delay(delay, cancellationToken).ConfigureAwait(false);\n    }\n\n    private static async Task<HttpRequestMessage> CloneRequestAsync(HttpRequestMessage source, CancellationToken cancellationToken)\n    {\n        var clone = new HttpRequestMessage(source.Method, source.RequestUri);\n        foreach (var header in source.Headers) clone.Headers.TryAddWithoutValidation(header.Key, header.Value);\n        if (source.Content is not null)\n        {\n            var bytes = await source.Content.ReadAsByteArrayAsync(cancellationToken).ConfigureAwait(false);\n            clone.Content = new ByteArrayContent(bytes);\n            foreach (var header in source.Content.Headers) clone.Content.Headers.TryAddWithoutValidation(header.Key, header.Value);\n        }\n        return clone;\n    }\n",
    );
    output = output.replacen(
        "                        var error = new ApiException((int)response.StatusCode, contents);\n                        _hooks?.OnError(error);\n                        throw error;",
        "                        throw new ApiException((int)response.StatusCode, contents);",
        1,
    );
    output.push_str(
        "\n    /// <summary>Downloads a non-JSON representation without attempting JSON deserialization.</summary>\n    private async Task<byte[]> SendBytesAsync(HttpRequestMessage request, CancellationToken cancellationToken)\n    {\n        using (request)\n        using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);\n        var bytes = await response.Content.ReadAsByteArrayAsync(cancellationToken).ConfigureAwait(false);\n        if (!response.IsSuccessStatusCode)\n        {\n            var error = new ApiException((int)response.StatusCode, Encoding.UTF8.GetString(bytes));\n            _hooks?.OnError(error);\n            throw error;\n        }\n        _hooks?.AfterResponse(response);\n        return bytes;\n    }\n\n    /// <summary>Yields payloads from an OpenAPI text/event-stream response. The underlying response is disposed when enumeration ends.</summary>\n    private async IAsyncEnumerable<string> StreamSseAsync(HttpRequestMessage request, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)\n    {\n        using (request)\n        using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);\n        if (!response.IsSuccessStatusCode)\n        {\n            var body = await response.Content.ReadAsStringAsync(cancellationToken).ConfigureAwait(false);\n            var error = new ApiException((int)response.StatusCode, body);\n            _hooks?.OnError(error);\n            throw error;\n        }\n        _hooks?.AfterResponse(response);\n        await using var stream = await response.Content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);\n        using var reader = new StreamReader(stream);\n        var data = new List<string>();\n        while (await reader.ReadLineAsync(cancellationToken).ConfigureAwait(false) is { } line)\n        {\n            if (line.Length == 0)\n            {\n                if (data.Count > 0) { yield return string.Join(\"\\n\", data); data.Clear(); }\n            }\n            else if (line == \"data\") data.Add(\"\");\n            else if (line.StartsWith(\"data:\", StringComparison.Ordinal))\n            {\n                var value = line[5..];\n                data.Add(value.StartsWith(\" \", StringComparison.Ordinal) ? value[1..] : value);\n            }\n        }\n        if (data.Count > 0) yield return string.Join(\"\\n\", data);\n    }\n"
    );
    render_error_deserialization_helper(&mut output);
    // A using declaration cannot be the body of a `using (request)` statement.
    // Keep the request lifetime scoped to each method with a second declaration
    // instead; this is valid C# and disposes both request and response.
    output = output.replace(
        "        using (request)\n        using var response",
        "        using var _request = request;\n        using var response",
    );
    output.push_str(r#"
    // Selectors were parsed by the generator; runtime mismatches end pagination.
    private static string? PoolsterJsonPath(JsonElement value, string path)
    {
        var current = value;
        if (path.StartsWith('/'))
        {
            foreach (var token in path[1..].Split('/'))
            {
                var key = token.Replace("~1", "/").Replace("~0", "~");
                if (current.ValueKind == JsonValueKind.Object) { if (!current.TryGetProperty(key, out current)) return null; }
                else if (current.ValueKind == JsonValueKind.Array && System.Text.RegularExpressions.Regex.IsMatch(key, "^(0|[1-9][0-9]*)$") && int.TryParse(key, out var index) && index < current.GetArrayLength()) current = current[index];
                else return null;
            }
        }
        else
        {
            if (!path.StartsWith('$')) return null;
            var position = 1;
            while (position < path.Length)
            {
                if (path[position] == '.')
                {
                    var start = ++position;
                    while (position < path.Length && path[position] != '.' && path[position] != '[') position++;
                    if (start == position || current.ValueKind != JsonValueKind.Object || !current.TryGetProperty(path[start..position], out current)) return null;
                }
                else if (path[position] == '[')
                {
                    var end = path.IndexOf(']', position);
                    if (end < 0 || current.ValueKind != JsonValueKind.Array || !long.TryParse(path[(position+1)..end], out var index)) return null;
                    if (index < 0) index += current.GetArrayLength();
                    if (index < 0 || index >= current.GetArrayLength()) return null;
                    current = current[(int)index];
                    position = end + 1;
                }
                else return null;
            }
        }
        return current.ValueKind == JsonValueKind.String ? current.GetString() : null;
    }
"#);
    output.push_str(include_str!("../../templates/call_options.cs.tmpl"));
    output.push_str(include_str!("../../templates/sequential_json.cs.tmpl"));
    output.push_str("}\n");
    output=output.replacen("/// <summary>Typed asynchronous client", "public sealed record PoolsterCallOptions { public IReadOnlyDictionary<string,string>? Headers {get;init;} public TimeSpan? Timeout {get;init;} }\n\n/// <summary>Typed asynchronous client",1);
    let deadline = include_str!("../../templates/call_deadline.cs.tmpl");
    let headers = "        if (_callHeaders is not null) foreach(var header in _callHeaders) { request.Headers.Remove(header.Key); request.Headers.TryAddWithoutValidation(header.Key,header.Value); }\n";
    for signature in [
        "    private async Task<T> SendWithRetryAsync<T>(HttpRequestMessage template, CancellationToken cancellationToken)\n    {\n",
        "    private async Task<byte[]> SendBytesAsync(HttpRequestMessage request, CancellationToken cancellationToken)\n    {\n",
        "    private async IAsyncEnumerable<string> StreamSseAsync(HttpRequestMessage request, [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)\n    {\n",
    ] {
        let selected_headers = if signature.contains("template,") {
            headers.replace("request.Headers", "template.Headers")
        } else {
            headers.to_owned()
        };
        output = output.replace(
            signature,
            &format!("{signature}{deadline}{selected_headers}"),
        );
    }
    output
}
