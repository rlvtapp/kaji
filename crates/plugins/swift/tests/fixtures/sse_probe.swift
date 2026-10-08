import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
final class Tracker: @unchecked Sendable {
 let lock=NSLock();var calls=0;var cancellations=0
 func call(){lock.lock();calls+=1;lock.unlock()}
 func cancel(){lock.lock();cancellations+=1;lock.unlock()}
 func snapshot()->(Int,Int){lock.lock();defer{lock.unlock()};return(calls,cancellations)}
}
struct Driver:PoolsterStreamingTransport {
 let tracker:Tracker;let packets:[Data];var suspended=false
 func execute(_ request:URLRequest) async throws ->(Data,URLResponse){fatalError("Buffered stream substitution")}
 func stream(_ request:URLRequest) async throws ->PoolsterByteStream {
  tracker.call();precondition(request.value(forHTTPHeaderField:"Authorization")=="Bearer test");precondition(request.value(forHTTPHeaderField:"X-Stream-Policy")=="enabled")
  let chunks=AsyncThrowingStream<Data,Error>{continuation in for packet in packets {continuation.yield(packet)};if !suspended {continuation.finish()}}
  return PoolsterByteStream(response:HTTPURLResponse(url:request.url!,statusCode:200,httpVersion:nil,headerFields:["Content-Type":"text/event-stream; charset=utf-8"])!,chunks:chunks,cancel:{tracker.cancel()})
 }
}
@main struct Probe {
 static func main() async throws {
  let options=PoolsterClientOptions(baseURL:URL(string:"https://unused.test")!,headers:["Authorization":"Bearer test"])
  let policy:PoolsterStreamingMiddleware={request,next in var request=request;request.setValue("enabled",forHTTPHeaderField:"X-Stream-Policy");return try await next(request)}
  let bytes=Array("\u{feff}:comment\r\nid:evt\r\ndata: café\r\ndata: 雪\r\n\r\ndata:\n\ndata: incompleteEOF".utf8)
  let tracker=Tracker();let driver=Driver(tracker:tracker,packets:bytes.map{Data([$0])});let client=PoolsterClient(options:options,transport:PoolsterStreamingMiddlewareTransport(inner:driver,middleware:policy))
  let events=client.events.getEvents();precondition(tracker.snapshot().0==0)
  var values:[String]=[];for try await event in events {values.append(event)};precondition(values==["café\n雪",""]);precondition(tracker.snapshot().0==1 && tracker.snapshot().1>0)
  let blocked=Tracker();let buffered=PoolsterClient(options:options,transport:PoolsterMiddlewareTransport(inner:Driver(tracker:blocked,packets:[]),middleware:{request,next in try await next(request)}));let unsupported=buffered.getEvents().makeAsyncIterator()
  do {_=try await unsupported.next();fatalError("Buffered policy bypassed")}catch PoolsterStreamingError.unsupportedTransport {}
  precondition(blocked.snapshot().0==0)
  let oversized=Tracker();let big=Driver(tracker:oversized,packets:Array(repeating:Data(repeating:97,count:65536),count:17));let large=PoolsterClient(options:options,transport:PoolsterStreamingMiddlewareTransport(inner:big,middleware:policy));let limit=large.getEvents().makeAsyncIterator()
  do {_=try await limit.next();fatalError("Unbounded frame")}catch PoolsterStreamingError.frameLimit {}
  precondition(oversized.snapshot().1>0)
  let cancellation=Tracker();let waiting=PoolsterClient(options:options,transport:PoolsterStreamingMiddlewareTransport(inner:Driver(tracker:cancellation,packets:[],suspended:true),middleware:policy))
  let task=Task {let iterator=waiting.getEvents().makeAsyncIterator();return try await iterator.next()}
  try await Task.sleep(nanoseconds:10000000);task.cancel();do {_=try await task.value;fatalError("Cancellation lost")}catch is CancellationError {}
  precondition(cancellation.snapshot().1>0)
  let invalid=Tracker();let replacement=PoolsterClient(options:options,transport:PoolsterStreamingMiddlewareTransport(inner:Driver(tracker:invalid,packets:[Data([100,97,116,97,58,32,255,10,10])]),middleware:policy));let iterator=replacement.getEvents().makeAsyncIterator();let event=try await iterator.next();precondition(event=="�")
 }
}
