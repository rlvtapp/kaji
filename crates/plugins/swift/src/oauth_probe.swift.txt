import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Issuer: KajiTransport {
 var count = 0
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  count += 1
  precondition(request.httpMethod == "POST")
  precondition(request.value(forHTTPHeaderField: "Authorization") == "Basic aWQ6c2VjcmV0")
  try await Task.sleep(nanoseconds: 20_000_000)
  return (Data(#"{"access_token":"fresh","token_type":"Bearer","expires_in":60}"#.utf8), HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil, headerFields: nil)!)
 }
 func calls() -> Int { count }
}
struct Terminal: KajiTransport {
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  precondition(["Bearer fresh", "Custom"].contains(request.value(forHTTPHeaderField: "Authorization")!))
  return (Data(), HTTPURLResponse(url: request.url!, statusCode: 204, httpVersion: nil, headerFields: nil)!)
 }
}
actor Rejecting: KajiTransport {
 var count = 0
 func execute(_ request: URLRequest) async throws -> (Data, URLResponse) {
  count += 1
  return (Data(), HTTPURLResponse(url:request.url!, statusCode:401, httpVersion:nil, headerFields:nil)!)
 }
 func calls() -> Int { count }
}
@main struct Probe {
 static func main() async throws {
  let issuer = Issuer()
  let provider = try KajiOAuthClientCredentials(tokenURL: URL(string:"https://issuer.test/token")!, clientID:"id", clientSecret:"secret", transport:issuer)
  try await withThrowingTaskGroup(of: String.self) { group in
   for _ in 0..<16 { group.addTask { try await provider.token() } }
   for try await token in group { precondition(token == "fresh") }
  }
  let first = await issuer.calls(); precondition(first == 1)
  await provider.invalidate("stale"); _ = try await provider.token()
  let second = await issuer.calls(); precondition(second == 1)
  await provider.invalidate("fresh"); _ = try await provider.token()
  let third = await issuer.calls(); precondition(third == 2)
  let transport = KajiOAuthTransport(inner:Terminal(), provider:provider, origin:URL(string:"https://api.test")!)
  _ = try await transport.execute(URLRequest(url:URL(string:"https://api.test/path")!))
  var explicit = URLRequest(url:URL(string:"https://api.test/path")!); explicit.setValue("Custom", forHTTPHeaderField:"Authorization")
  _ = try await transport.execute(explicit)
  do { _ = try await transport.execute(URLRequest(url:URL(string:"https://evil.test/path")!)); fatalError() } catch KajiOAuthError.invalidConfiguration {}
  let rejecting = Rejecting()
  let replay = KajiOAuthTransport(inner:rejecting, provider:provider, origin:URL(string:"https://api.test")!)
  _ = try await replay.execute(URLRequest(url:URL(string:"https://api.test/path")!))
  let replayed = await rejecting.calls(); precondition(replayed == 2)
  var unsafe = URLRequest(url:URL(string:"https://api.test/path")!); unsafe.httpMethod = "POST"
  _ = try await replay.execute(unsafe)
  let unreplayed = await rejecting.calls(); precondition(unreplayed == 3)
  let base = KajiClient(options:.init(baseURL:URL(string:"https://api.test")!, headers:["X-Scope":"base"]), transport:Terminal())
  let scoped = try base.forCall(headers:["x-scope":"call"], timeout:2)
  let scopedRequest = try scoped.makeRequest(method:"GET",path:"/path")
  precondition(scopedRequest.value(forHTTPHeaderField:"X-Scope") == "call" && scopedRequest.timeoutInterval == 2)
  let baseRequest = try base.makeRequest(method:"GET",path:"/path"); precondition(baseRequest.value(forHTTPHeaderField:"X-Scope") == "base")
  print("oauth singleflight origin explicit-auth and scoped controls passed")
 }
}
