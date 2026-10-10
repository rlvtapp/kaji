//! Runtime emission for the Swift HTTP SDK.
use crate::*;

pub(crate) fn json_value() -> String {
    format!(
        "{NOTICE}\nimport Foundation\n\n/// A JSON value used for unconstrained OpenAPI schemas.\npublic enum JSONValue: Codable, Sendable, Equatable {{\n  case null\n    case bool(Bool)\n    case integer(Int64)\n    case unsignedInteger(UInt64)\n    case number(Double)\n    case string(String)\n    case array([JSONValue])\n    case object([String: JSONValue])\n\n    public init(from decoder: Decoder) throws {{\n    let container = try decoder.singleValueContainer()\n        if container.decodeNil() {{\n      self = .null\n    }}\n    \n        else if let value = try? container.decode(Bool.self) {{\n      self = .bool(value)\n    }}\n    \n        else if let value = try? container.decode(Int64.self) {{\n      self = .integer(value)\n    }}\n    \n        else if let value = try? container.decode(UInt64.self) {{\n      self = .unsignedInteger(value)\n    }}\n    \n        else if let value = try? container.decode(Double.self) {{\n      self = .number(value)\n    }}\n    \n        else if let value = try? container.decode(String.self) {{\n      self = .string(value)\n    }}\n    \n        else if let value = try? container.decode([JSONValue].self) {{\n      self = .array(value)\n    }}\n    \n        else {{\n      self = .object(try container.decode([String: JSONValue].self))\n    }}\n    \n\n  }}\n  \n\n    public func encode(to encoder: Encoder) throws {{\n    var container = encoder.singleValueContainer()\n        switch self {{\n      case .null: try container.encodeNil()\n        case .bool(let value): try container.encode(value)\n        case .integer(let value): try container.encode(value)\n        case .unsignedInteger(let value): try container.encode(value)\n        case .number(let value): try container.encode(value)\n        case .string(let value): try container.encode(value)\n        case .array(let value): try container.encode(value)\n        case .object(let value): try container.encode(value)\n\n    }}\n    \n\n  }}\n  \n}}\n\n\ninternal struct PoolsterCodingKey: CodingKey {{\n  let stringValue: String\n    let intValue: Int? = nil\n    init(_ value: String) {{\n    self.stringValue = value\n  }}\n  \n    init?(stringValue: String) {{\n    self.stringValue = stringValue\n  }}\n  \n    init?(intValue: Int) {{\n    return nil\n  }}\n  \n}}\n\n"
    )
}

pub(crate) fn client_runtime() -> String {
    let runtime = format!(
        "{NOTICE}\nimport Foundation\n#if canImport(FoundationNetworking)\nimport FoundationNetworking\n#endif\n\npublic struct PoolsterClientOptions: Sendable {{\n    public var baseURL: URL\n    public var headers: [String: String]\n    public var timeout: TimeInterval\n\n    public init(baseURL: URL, headers: [String: String] = [:], timeout: TimeInterval = 30) {{\n        self.baseURL = baseURL\n        self.headers = headers\n        self.timeout = timeout\n    }}\n}}\n\npublic enum PoolsterAPIError: Error, Sendable {{\n    case invalidURL(String)\n    case invalidResponse\n    case status(code: Int, body: Data)\n}}\n\n/// Receives lifecycle notifications without requiring a logging framework.\npublic protocol PoolsterClientHook: Sendable {{\n    func willSend(_ request: URLRequest)\n    func didReceive(_ response: HTTPURLResponse, body: Data)\n}}\n\npublic final class PoolsterClient: @unchecked Sendable {{\n    private let options: PoolsterClientOptions\n    private let session: URLSession\n    private let hooks: [any PoolsterClientHook]\n    private let encoder = JSONEncoder()\n    private let decoder = JSONDecoder()\n\n    public init(options: PoolsterClientOptions, session: URLSession = .shared, hooks: [any PoolsterClientHook] = []) {{\n        self.options = options\n        self.session = session\n        __POOLSTER_TRANSPORT_INIT__\n        self.hooks = hooks\n    }}\n\n    internal func makeRequest(method: String, path: String, query: [URLQueryItem] = []) throws -> URLRequest {{\n        guard var components = URLComponents(url: options.baseURL, resolvingAgainstBaseURL: false), let operationPath = URLComponents(string: path.replacingOccurrences(of: \"?\", with: \"%3F\").replacingOccurrences(of: \"#\", with: \"%23\"))?.percentEncodedPath else {{\n            throw PoolsterAPIError.invalidURL(path)\n        }}\n        let basePath = components.percentEncodedPath.trimmingCharacters(in: CharacterSet(charactersIn: \"/\"))\n        components.percentEncodedPath = (basePath.isEmpty ? \"\" : \"/\" + basePath) + \"/\" + operationPath.trimmingCharacters(in: CharacterSet(charactersIn: \"/\"))\n        components.queryItems = query.isEmpty ? nil : query\n        guard let url = components.url else {{ throw PoolsterAPIError.invalidURL(path) }}\n        var request = URLRequest(url: url, timeoutInterval: options.timeout)\n        request.httpMethod = method\n        request.setValue(\"application/json\", forHTTPHeaderField: \"Accept\")\n        for (name, value) in options.headers {{ request.setValue(value, forHTTPHeaderField: name) }}\n        return request\n    }}\n\n    internal func send<T: Decodable>(_ request: URLRequest, as type: T.Type) async throws -> T {{\n        hooks.forEach {{ $0.willSend(request) }}\n        let (data, response) = try await session.data(for: request)\n        guard let http = response as? HTTPURLResponse else {{ throw PoolsterAPIError.invalidResponse }}\n        hooks.forEach {{ $0.didReceive(http, body: data) }}\n        guard (200..<300).contains(http.statusCode) else {{ throw PoolsterAPIError.status(code: http.statusCode, body: data) }}\n        return try decoder.decode(T.self, from: normalizeSequentialJSON(data, contentType: http.value(forHTTPHeaderField: \"Content-Type\")))\n    }}\n\n    internal func sendVoid(_ request: URLRequest) async throws {{\n        hooks.forEach {{ $0.willSend(request) }}\n        let (data, response) = try await session.data(for: request)\n        guard let http = response as? HTTPURLResponse else {{ throw PoolsterAPIError.invalidResponse }}\n        hooks.forEach {{ $0.didReceive(http, body: data) }}\n        guard (200..<300).contains(http.statusCode) else {{ throw PoolsterAPIError.status(code: http.statusCode, body: data) }}\n    }}\n\n    internal func encode<T: Encodable>(_ body: T) throws -> Data {{ try encoder.encode(body) }}\n}}\n\nextension String {{\n    var poolsterPathComponent: String {{ addingPercentEncoding(withAllowedCharacters: CharacterSet(charactersIn: \"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~\")) ?? self }}\n}}\n"
    );
    runtime.replace("public final class PoolsterClient: @unchecked Sendable {", r#"
/// Replace execution or compose middleware around the Foundation transport.
public protocol PoolsterTransport: Sendable {
    func execute(_ request: URLRequest) async throws -> (Data, URLResponse)
}
public struct PoolsterURLSessionTransport: PoolsterTransport {
    private let session: URLSession
    public init(session: URLSession = .shared) { self.session = session }
    public func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
        try await session.data(for: request)
    }
}
public typealias PoolsterNext = @Sendable (URLRequest) async throws -> (Data, URLResponse)
public typealias PoolsterMiddleware = @Sendable (URLRequest, PoolsterNext) async throws -> (Data, URLResponse)
/// Outer middleware runs first on requests and last on responses.
public struct PoolsterMiddlewareTransport: PoolsterTransport {
    private let inner: any PoolsterTransport
    private let middleware: PoolsterMiddleware
    public init(inner: any PoolsterTransport, middleware: @escaping PoolsterMiddleware) {
        self.inner = inner; self.middleware = middleware
    }
    public func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
        try await middleware(request, { request in try await inner.execute(request) })
    }
}
public final class PoolsterClient: @unchecked Sendable {"#)
    .replace("    internal func makeRequest(method:", r#"    /// Independent options for a call or traversal. Native request timeout applies per attempt.
    public func forCall(headers: [String: String] = [:], timeout: TimeInterval? = nil) throws -> PoolsterClient {
        var scoped = options
        if let timeout { guard timeout.isFinite && timeout > 0 else { throw PoolsterAPIError.invalidResponse }; scoped.timeout = timeout }
        for (name, value) in headers {
            guard !name.isEmpty && !name.contains("\r") && !name.contains("\n") && !value.contains("\r") && !value.contains("\n") else { throw PoolsterAPIError.invalidResponse }
            scoped.headers = scoped.headers.filter { $0.key.lowercased() != name.lowercased() }
            scoped.headers[name] = value
        }
        return PoolsterClient(options: scoped, session: session, hooks: hooks, transport: transport)
    }
    internal func makeRequest(method:"#)
    .replace("    private let hooks: [any PoolsterClientHook]", "    private let transport: any PoolsterTransport\n    private let hooks: [any PoolsterClientHook]")
    .replace("hooks: [any PoolsterClientHook] = [])", "hooks: [any PoolsterClientHook] = [], transport: (any PoolsterTransport)? = nil)")
    .replace("__POOLSTER_TRANSPORT_INIT__", "self.transport = transport ?? PoolsterURLSessionTransport(session: session)")
    .replace("try await session.data(for: request)", "try await transport.execute(request)")
    .replace("    public var timeout: TimeInterval", "    public var timeout: TimeInterval\n    public var maxAttempts: Int\n    public var retryBaseDelay: Double\n    public var retryMaxDelay: Double")
    .replace("timeout: TimeInterval = 30)", "timeout: TimeInterval = 30, maxAttempts: Int = 1, retryBaseDelay: Double = 0.5, retryMaxDelay: Double = 30)")
    .replace("        self.timeout = timeout", "        self.timeout = timeout\n        self.maxAttempts = min(10,max(1,maxAttempts))\n        self.retryBaseDelay = retryBaseDelay.isFinite ? max(0,min(60,retryBaseDelay)) : 0.5\n        self.retryMaxDelay = retryMaxDelay.isFinite ? max(0,min(60,retryMaxDelay)) : 30")
    .replace("    internal func makeRequest", &(r#"    internal func poolsterContinuationURL(_ next: URL?) throws -> URL? {
        guard let next else { return nil }
        let baseURL=options.baseURL
        func port(_ url: URL) -> Int? { url.port ?? (url.scheme?.lowercased() == "https" ? 443 : url.scheme?.lowercased() == "http" ? 80 : nil) }
        guard let scheme=next.scheme?.lowercased(), ["http", "https"].contains(scheme), scheme == baseURL.scheme?.lowercased(),
              let host=next.host?.lowercased(), host == baseURL.host?.lowercased(), port(next) == port(baseURL),
              next.user == nil, next.password == nil, next.fragment == nil else { throw PoolsterAPIError.invalidResponse }
        return next
    }
"#.to_owned()+"    internal func makeRequest"))
    .replace("_ request: URLRequest, as type: T.Type)", "_ request: URLRequest, as type: T.Type, idempotencyHeader: String? = nil)")
    .replace("sendVoid(_ request: URLRequest)", "sendVoid(_ request: URLRequest, idempotencyHeader: String? = nil)")
    .replace("        hooks.forEach { $0.willSend(request) }\n", "")
    .replace("        hooks.forEach { $0.didReceive(http, body: data) }\n", "")
    .replace("let (data, response) = try await transport.execute(request)", "let (data, response) = try await executeWithRetry(request, idempotencyHeader: idempotencyHeader)")
    .replace("    internal func encode<T:", &(include_str!("../../templates/sequential_json.swift.tmpl").to_owned()+"\n    internal func encode<T:"))
    .replace("    internal func encode<T:", &(include_str!("../../templates/retry_runtime.swift.tmpl").to_owned()+"\n    internal func encode<T:"))
    // Preserve the default adapter's direct Foundation execution.
    .replace("        try await transport.execute(request)\n    }\n}\npublic typealias", "        try await session.data(for: request)\n    }\n}\npublic typealias")
}

pub(crate) fn client_runtime_with_streaming() -> String {
    let source = client_runtime().replace(
        "public struct PoolsterURLSessionTransport: PoolsterTransport",
        "public struct PoolsterURLSessionTransport: PoolsterStreamingTransport",
    );
    let source = source.replacen(
        "    public func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {",
        r#"    public func stream(_ request: URLRequest) async throws -> PoolsterByteStream {
        guard session.delegate == nil else {throw PoolsterStreamingError.customSessionDelegate}
        return try await PoolsterURLSessionStream(configuration:session.configuration).open(request)
    }
    public func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {"#,
        1,
    );
    source.replace("    internal func encode<T:",r#"    internal func streamEvents(_ request:URLRequest) async throws -> PoolsterByteStream {
        try Task.checkCancellation()
        guard let streaming=transport as? any PoolsterStreamingTransport else {throw PoolsterStreamingError.unsupportedTransport}
        hooks.forEach {$0.willSend(request)}
        let stream=try await streaming.stream(request)
        hooks.forEach {$0.didReceive(stream.response,body:Data())}
        guard (200..<300).contains(stream.response.statusCode) else {stream.cancel();throw PoolsterAPIError.status(code:stream.response.statusCode,body:Data())}
        guard stream.response.value(forHTTPHeaderField:"Content-Type")?.split(separator:";",maxSplits:1).first?.trimmingCharacters(in:.whitespaces).lowercased()=="text/event-stream" else {stream.cancel();throw PoolsterStreamingError.invalidContentType}
        return stream
    }
    internal func encode<T:"#)
}
