import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
enum ProbeError:Error {case mismatch}
func canonical(_ value:Any)throws->Data {try JSONSerialization.data(withJSONObject:value,options:[.sortedKeys,.fragmentsAllowed])}
final class ProbeTransport:PoolsterTransport,@unchecked Sendable {
 let sample:[String:Any];var calls=0
 init(sample:[String:Any]){self.sample=sample}
 func execute(_ request:URLRequest)async throws->(Data,URLResponse){
 calls+=1
 guard request.httpMethod==sample["method"] as? String,request.url?.path==sample["path"] as? String else{throw ProbeError.mismatch}
 let query=URLComponents(url:request.url!,resolvingAgainstBaseURL:false)?.queryItems ?? []
 for (name,value) in sample["query"] as! [String:Any] {let text=(value as? String) ?? String(describing:value);guard query.first(where:{$0.name==name})?.value==text else{throw ProbeError.mismatch}}
 for (name,value) in sample["headers"] as! [String:String] {guard request.value(forHTTPHeaderField:name)==value else{throw ProbeError.mismatch}}
 if !(sample["body"] is NSNull) {guard let data=request.httpBody else{throw ProbeError.mismatch};let actual=try JSONSerialization.jsonObject(with:data,options:[.fragmentsAllowed]);guard try canonical(actual)==canonical(sample["body"]!) else{throw ProbeError.mismatch}} else if request.httpBody != nil {throw ProbeError.mismatch}
 let body=sample["response"] is NSNull ? Data() : try JSONSerialization.data(withJSONObject:sample["response"]!,options:[.fragmentsAllowed])
 return(body,HTTPURLResponse(url:request.url!,statusCode:Int(sample["status"] as! String)!,httpVersion:nil,headerFields:["Content-Type":"application/json"])!)
 }
}
@main struct OperationTests {static func main()async throws {
__CASES__
}}
