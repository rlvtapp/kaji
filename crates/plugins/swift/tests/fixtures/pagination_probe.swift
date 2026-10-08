import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
actor Calls { var urls:[String]=[];func add(_ url:String){urls.append(url)};func snapshot()->[String]{urls} }
struct Driver: PoolsterTransport {
 let calls:Calls
 let target:String?
 func execute(_ request:URLRequest) async throws -> (Data,URLResponse) {
  precondition(request.value(forHTTPHeaderField:"Authorization")=="Bearer test")
  let url=request.url!;await calls.add(url.absoluteString)
  let query=URLComponents(url:url,resolvingAgainstBaseURL:false)!.queryItems ?? []
  let body:String
  if url.path=="/items" {let offset=query.first(where:{$0.name=="offset"})?.value;body=offset=="0" ? "[\"a\",\"b\"]" : "[\"c\"]"}
  else if let target {body=String(data:try JSONSerialization.data(withJSONObject:["next":target]),encoding:.utf8)!}
  else {body=query.contains(where:{$0.name=="page" && $0.value=="2"}) ? "{}" : "{\"next\":\"https://api.example.test/links?page=2&tenant=server\"}"}
  return (Data(body.utf8),HTTPURLResponse(url:url,statusCode:200,httpVersion:nil,headerFields:nil)!)
 }
}
@main struct Probe {
 static func main() async throws {
  let options=PoolsterClientOptions(baseURL:URL(string:"https://api.example.test")!,headers:["Authorization":"Bearer test"])
  let calls=Calls();let client=PoolsterClient(options:options,transport:Driver(calls:calls,target:nil))
  let sequence=client.listItemsPages(limit:2);let before=await calls.snapshot();precondition(before.isEmpty)
  var pages:[[String]]=[];for try await page in sequence {pages.append(page)};precondition(pages==[["a","b"],["c"]])
  let offsets=await calls.snapshot();precondition(offsets.count==2 && offsets[0].contains("offset=0") && offsets[1].contains("offset=2"))
  let links=client.links.listLinksPages(tenant:"kept");var count=0;for try await _ in links {count+=1};precondition(count==2)
  let urls=await calls.snapshot();precondition(urls[2].contains("tenant=kept") && urls[3]=="https://api.example.test/links?page=2&tenant=server")
  for target in ["https://evil.example/links","http://api.example.test/links","https://api.example.test:444/links","https://user@api.example.test/links","https://api.example.test/links#fragment","/relative"] {
   let calls=Calls();let client=PoolsterClient(options:options,transport:Driver(calls:calls,target:target));var iterator=client.listLinksPages().makeAsyncIterator()
   _=try await iterator.next();do {_=try await iterator.next();fatalError("Unsafe URL accepted")}catch PoolsterAPIError.invalidResponse {}
   let recorded=await calls.snapshot();precondition(recorded.count==1)
  }
  let loopCalls=Calls();let looping=PoolsterClient(options:options,transport:Driver(calls:loopCalls,target:"https://api.example.test/links"));var iterator=looping.listLinksPages().makeAsyncIterator();_=try await iterator.next();_=try await iterator.next()
  do {_=try await iterator.next();fatalError("Loop accepted")}catch PoolsterPaginationError.repeatedURL {}
  var invalid=client.listItemsPages(offset:-1).makeAsyncIterator();do {_=try await invalid.next();fatalError("Negative offset")}catch PoolsterPaginationError.invalidOffset {}
 }
}
