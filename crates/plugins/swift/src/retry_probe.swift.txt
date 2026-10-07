final class RetryProbe:KajiTransport,@unchecked Sendable {
 var calls=0;var keys:[String?]=[];var status=503;var delay="0"
 func execute(_ request:URLRequest)async throws->(Data,URLResponse){calls+=1;keys.append(request.value(forHTTPHeaderField:"X-Once"));return(Data(),HTTPURLResponse(url:request.url!,statusCode:calls==1 ? status : 204,httpVersion:nil,headerFields:["retry-after-ms":delay])!)}
}
