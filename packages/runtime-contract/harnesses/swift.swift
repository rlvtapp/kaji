import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@main struct ContractProbe {
    static func main() async throws {
        let environment = ProcessInfo.processInfo.environment
        let scenario = try JSONSerialization.jsonObject(with: Data((environment["KAJI_CONTRACT_SCENARIO"] ?? "{}").utf8)) as! [String: Any]
        let policy = KajiMiddlewareTransport(inner: KajiURLSessionTransport()) { request, following in
            var rewritten = request
            rewritten.setValue("yes", forHTTPHeaderField: "X-Contract-Middleware")
            return try await following(rewritten)
        }
        var options = KajiClientOptions(baseURL: URL(string: environment["KAJI_CONTRACT_URL"]!)!, headers: ["Authorization": "Bearer " + environment["KAJI_CONTRACT_CASE"]!])
        options.maxAttempts = 3
        options.retryBaseDelay = 0
        options.retryMaxDelay = 0
        let client = KajiClient(options: options, transport: policy)
        let result: [String: String]
        do {
            let action = scenario["action"] as? String ?? "getContact"
            var id = ""
            if action == "echoWire" {
                id = try await client.echoWire(key: "café/雪", text: "héllo 雪", flag: false, count: 0, tags: ["a", "b"], xLabel: "caller", body: WireInput(enabled: false, count: 0, note: nil)).id
            } else if action == "listContactsPages" {
                var ids: [String] = []
                for try await page in client.listContactsPages(page: scenario["page"] as? Int, limit: scenario["limit"] as? Int) { ids.append(contentsOf: page.items.map { $0.id }) }
                id = ids.joined(separator: ",")
            } else {
                for _ in 0..<(scenario["repeats"] as? Int ?? 1) {
                    switch action {
                    case "createContact": id = try await client.createContact(xOnce: scenario["caller_key"] as? String).id
                    case "patchContact": id = try await client.patchContact(xOnce: scenario["caller_key"] as? String).id
                    case "unsafeCreateContact": id = try await client.unsafeCreateContact().id
                    case "unsafePatchContact": id = try await client.unsafePatchContact().id
                    default: id = try await client.getContact().id
                    }
                }
            }
            result = ["outcome": "success", "id": id]
        } catch { result = ["outcome": "error"] }
        print(String(data: try JSONSerialization.data(withJSONObject: result), encoding: .utf8)!)
    }
}
