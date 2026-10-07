import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@main struct ContractProbe {
    static func main() async throws {
        let environment = ProcessInfo.processInfo.environment
        let policy = KajiMiddlewareTransport(inner: KajiURLSessionTransport()) { request, following in
            var rewritten = request
            rewritten.setValue("yes", forHTTPHeaderField: "X-Contract-Middleware")
            return try await following(rewritten)
        }
        let options = KajiClientOptions(baseURL: URL(string: environment["KAJI_CONTRACT_URL"]!)!, headers: ["Authorization": "Bearer " + environment["KAJI_CONTRACT_CASE"]!])
        let client = KajiClient(options: options, transport: policy)
        let result: [String: String]
        do {
            let model = try await client.getContact()
            result = ["outcome": "success", "id": model.id]
        } catch { result = ["outcome": "error"] }
        print(String(data: try JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
    }
}
