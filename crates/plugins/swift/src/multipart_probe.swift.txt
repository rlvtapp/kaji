import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

struct ProbeFailure: Error { let message: String }
func require(_ value: Bool, _ message: String) throws { if !value { throw ProbeFailure(message: message) } }
struct MIMEPart { let headers: [String: String]; let body: Data }
func mime(_ request: URLRequest) throws -> [String: [MIMEPart]] {
    let contentType = request.value(forHTTPHeaderField: "Content-Type") ?? ""
    guard let range = contentType.range(of: "boundary=") else { throw ProbeFailure(message: "missing boundary") }
    let boundary = String(contentType[range.upperBound...])
    let body = request.httpBody ?? Data()
    let start = Data(("--\(boundary)\r\n").utf8), next = Data(("\r\n--\(boundary)").utf8)
    try require(body.starts(with: start) && body.suffix(Data(("--\(boundary)--\r\n").utf8).count) == Data(("--\(boundary)--\r\n").utf8), "invalid framing")
    var cursor = start.count, result: [String: [MIMEPart]] = [:]
    while cursor < body.count {
        guard let headerEnd = body.range(of: Data("\r\n\r\n".utf8), in: cursor..<body.count), let finish = body.range(of: next, in: headerEnd.upperBound..<body.count), let text = String(data: body.subdata(in: cursor..<headerEnd.lowerBound), encoding: .utf8) else { throw ProbeFailure(message: "invalid MIME headers") }
        var headers: [String: String] = [:]
        for line in text.components(separatedBy: "\r\n") {
            guard let colon = line.firstIndex(of: ":") else { throw ProbeFailure(message: "invalid header") }
            headers[String(line[..<colon]).lowercased()] = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
        }
        guard let disposition = headers["content-disposition"], let nameStart = disposition.range(of: "name=\""), let nameEnd = disposition[nameStart.upperBound...].firstIndex(of: "\"") else { throw ProbeFailure(message: "missing part name") }
        let name = String(disposition[nameStart.upperBound..<nameEnd])
        result[name, default: []].append(MIMEPart(headers: headers, body: body.subdata(in: headerEnd.upperBound..<finish.lowerBound)))
        cursor = finish.upperBound
        if body[cursor..<min(cursor + 2, body.count)] == Data("--".utf8) { break }
        try require(body[cursor..<min(cursor + 2, body.count)] == Data("\r\n".utf8), "invalid part separator")
        cursor += 2
    }
    return result
}
func waitSync(_ semaphore: DispatchSemaphore) { semaphore.wait() }
func uploadBody() -> UploadMultipartBody {
    UploadMultipartBody(title: "title 雪\nline", enabled: false, count: 0, tags: ["A", "雪"], csv: ["A", "雪"], jsonText: "雪", metadata: Metadata(value: "雪"), metadatas: [Metadata(value: "one"), Metadata(value: "two")], file: KajiMultipartFile(data: Data([0, 255, 13, 10, 65]), filename: "one.bin", headers: ["X-File": "file-value"]), files: [KajiMultipartFile(data: Data([3, 0, 128]), filename: "two.bin"), KajiMultipartFile(data: Data([4, 255]), filename: "three.bin")], partHeaders: ["metadata": ["X-Part": "metadata-value"]])
}
func verify(_ request: URLRequest) throws {
    let parts = try mime(request)
    try require(request.httpMethod == "POST", "method")
    try require(request.value(forHTTPHeaderField: "Authorization") == "Bearer author" && request.value(forHTTPHeaderField: "X-Middleware") == "installed", "HTTP policy bypass")
    try require(String(data: parts["title"]![0].body, encoding: .utf8) == "title 雪\nline", "UTF-8 text")
    try require(parts["enabled"]![0].body == Data("false".utf8) && parts["count"]![0].body == Data("0".utf8), "falsy scalar")
    try require(parts["tags"]!.count == 2 && parts["tags"]![1].body == Data("雪".utf8), "repeated scalar arrays")
    try require(parts["csv"]!.count == 1 && parts["csv"]![0].body == Data("A,雪".utf8) && parts["csv"]![0].headers["content-type"] == "text/plain", "form encoding must ignore contentType")
    try require(try JSONDecoder().decode(String.self, from: parts["jsonText"]![0].body) == "雪", "JSON scalar encoding")
    try require(try JSONDecoder().decode(Metadata.self, from: parts["metadata"]![0].body).value == "雪" && parts["metadata"]![0].headers["x-part"] == "metadata-value", "JSON part and headers")
    try require(parts["metadatas"]!.count == 2 && (try JSONDecoder().decode(Metadata.self, from: parts["metadatas"]![1].body)).value == "two", "repeated JSON parts")
    try require(parts["file"]![0].body == Data([0, 255, 13, 10, 65]) && parts["file"]![0].headers["x-file"] == "file-value", "raw binary and file header")
    try require(parts["files"]!.count == 2 && parts["files"]![0].headers["content-type"] == "image/png", "binary array encoding")
    try require(parts["optional"] == nil, "optional omission")
}
actor Driver: KajiTransport {
    var requests: [URLRequest] = []
    func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
        requests.append(request)
        let status = request.url!.path.contains("/fail/") ? 503 : 204
        return (Data(), HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: [:])!)
    }
    func captured() -> [URLRequest] { requests }
}
func middleware(_ inner: any KajiTransport) -> KajiMiddlewareTransport {
    KajiMiddlewareTransport(inner: inner) { request, following in
        var changed = request; changed.setValue("installed", forHTTPHeaderField: "X-Middleware")
        return try await following(changed)
    }
}
@main struct Probe {
    static func main() async throws {
        let driver = Driver()
        let client = KajiClient(options: KajiClientOptions(baseURL: URL(string: "https://example.invalid")!, headers: ["Authorization": "Bearer author"]), transport: middleware(driver))
        let body = uploadBody()
        try await client.upload(body: body)
        try verify((await driver.captured())[0])
        try await client.submit(body: .json(Metadata(value: "JSON alternative")))
        let jsonRequest = (await driver.captured())[1]
        try require(jsonRequest.value(forHTTPHeaderField: "Content-Type") == "application/json", "mixed representation Content-Type")
        try require(try JSONDecoder().decode(Metadata.self, from: jsonRequest.httpBody!).value == "JSON alternative", "mixed representation bytes")
        let fail = KajiClient(options: KajiClientOptions(baseURL: URL(string: "https://example.invalid/fail")!, headers: ["Authorization": "Bearer author"], retryBaseDelay: 0, retryMaxDelay: 0), transport: middleware(driver))
        do { try await fail.upload(body: body); throw ProbeFailure(message: "503 accepted") } catch KajiAPIError.status(let code, _) { try require(code == 503, "status") }
        try require((await driver.captured()).count == 3, "unsafe POST retried")
        var invalid = body; invalid.file = KajiMultipartFile(data: Data(), filename: "bad\r\nX-Evil: injected", headers: ["X-File": "file-value"])
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "filename injection accepted") } catch KajiMultipartError.invalidFilename {}
        invalid = body; invalid.partHeaders["metadata"] = ["X-Part": "bad\r\nX-Evil: injected"]
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "header injection accepted") } catch KajiMultipartError.invalidHeader {}
        invalid = body; invalid.partHeaders = [:]
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "missing header accepted") } catch KajiMultipartError.missingHeader(let name) { try require(name == "X-Part", "missing header diagnostic") }
        invalid = body; invalid.maximumBodyBytes = 3
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "oversize accepted") } catch KajiMultipartError.bodyTooLarge {}
        invalid = body; invalid.maximumBodyBytes = 0
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "invalid limit accepted") } catch KajiMultipartError.invalidLimit {}
        invalid = body; invalid.tags = Array(repeating: "x", count: 1025)
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "part limit accepted") } catch KajiMultipartError.tooManyParts {}
        invalid = body; invalid.partHeaders["unknown"] = [:]
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "unknown part accepted") } catch KajiMultipartError.unknownPart {}
        invalid = body; invalid.file = KajiMultipartFile(data: Data(), contentType: "text/plain\r\nX-Evil: injected", headers: ["X-File": "file-value"])
        do { try await client.upload(body: invalid); throw ProbeFailure(message: "media injection accepted") } catch KajiMultipartError.invalidContentType {}
        try require((await driver.captured()).count == 3, "invalid body reached transport")
        let entered = DispatchSemaphore(value: 0), gate = DispatchSemaphore(value: 0)
        let task = Task.detached { entered.signal(); waitSync(gate); return try body.kajiEncoded() }
        waitSync(entered); task.cancel(); gate.signal()
        do { _ = try await task.value; throw ProbeFailure(message: "cancelled preparation accepted") } catch is CancellationError {}
        if let raw = ProcessInfo.processInfo.environment["KAJI_MULTIPART_URL"], let url = URL(string: raw) {
            let real = KajiClient(options: KajiClientOptions(baseURL: url, headers: ["Authorization": "Bearer author"]), transport: middleware(KajiURLSessionTransport()))
            try await real.upload(body: body)
            let failing = KajiClient(options: KajiClientOptions(baseURL: url.appendingPathComponent("fail"), headers: ["Authorization": "Bearer author"], retryBaseDelay: 0, retryMaxDelay: 0), transport: middleware(KajiURLSessionTransport()))
            do { try await failing.upload(body: body); throw ProbeFailure(message: "native 503 accepted") } catch KajiAPIError.status(let code, _) { try require(code == 503, "native status") }
            let marker = ProcessInfo.processInfo.environment["KAJI_MULTIPART_CANCEL_MARKER"]!
            let cancelling = KajiClient(options: KajiClientOptions(baseURL: url.appendingPathComponent("cancel"), headers: ["Authorization": "Bearer author"]), transport: middleware(KajiURLSessionTransport()))
            let pending = Task { try await cancelling.upload(body: body) }
            for _ in 0..<300 { if FileManager.default.fileExists(atPath: marker) { break }; try await Task.sleep(nanoseconds: 10_000_000) }
            try require(FileManager.default.fileExists(atPath: marker), "native cancellation did not send request")
            pending.cancel()
            do { try await pending.value; throw ProbeFailure(message: "native cancellation accepted") } catch is CancellationError {} catch let error as URLError { try require(error.code == .cancelled, "native cancellation error") }
        }
        print("MIME bytes, mixed media, policy, limits and cancellation passed")
    }
}
