import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@main struct Probe {
 static func main() async throws {
  let base="http://127.0.0.1:__PORT__"
  let root="__ROOT__"
  func client(_ suffix:String="")->PoolsterClient {PoolsterClient(options:PoolsterClientOptions(baseURL:URL(string:base+suffix)!,headers:["Authorization":"Bearer test"]))}
  let events=client().getEvents();let iterator=events.makeAsyncIterator()
  let first=try await iterator.next();precondition(first=="café")
  try "released".write(toFile:root+"/gate",atomically:true,encoding:.utf8)
  let second=try await iterator.next();precondition(second=="second")
  let end=try await iterator.next();precondition(end==nil)
  let cancellation=Task {
   let iterator=client("/cancel").getEvents().makeAsyncIterator()
   let first=try await iterator.next();precondition(first=="waiting")
   try "ready".write(toFile:root+"/ready",atomically:true,encoding:.utf8)
   return try await iterator.next()
  }
  for _ in 0..<500 {if FileManager.default.fileExists(atPath:root+"/ready"){break};try await Task.sleep(nanoseconds:10000000)}
  precondition(FileManager.default.fileExists(atPath:root+"/ready"));cancellation.cancel()
  do {_=try await cancellation.value;fatalError("Cancellation lost")}catch is CancellationError {}
  for _ in 0..<500 {if FileManager.default.fileExists(atPath:root+"/disconnected"){break};try await Task.sleep(nanoseconds:10000000)}
  precondition(FileManager.default.fileExists(atPath:root+"/disconnected"))
  let redirect=client("/redirect").getEvents().makeAsyncIterator()
  do {_=try await redirect.next();fatalError("Redirect followed")}catch PoolsterAPIError.status(let code, _) {precondition(code==302)}
 }
}
