import Foundation
struct Vector:Decodable {let secret:String;let raw_body:String;let headers:[String:String];let now:Double}
struct Payload:Decodable {let data:String;let value:Int}
@main struct Probe {
    static func main() throws {
        let vector = try JSONDecoder().decode(Vector.self,from:Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[1])))
        let body = Data(vector.raw_body.utf8), headers = vector.headers.mapValues { [$0] }
        let wrong = "whsec_" + Data(repeating:65,count:24).base64EncodedString()
        func reject(_ bytes:Data = body, _ metadata:[String:[String]] = headers, _ keys:[String] = [vector.secret], _ now:Double = vector.now, _ tolerance:Double = 300) {
            do {_ = try StandardWebhooks.verify(bytes,headers:metadata,secrets:keys,now:now,tolerance:tolerance);fatalError("accepted invalid webhook")}
            catch {precondition(!String(describing:error).contains(vector.secret));precondition(!String(describing:error).contains(vector.raw_body))}
        }
        let valid = try StandardWebhooks.verify(body,headers:headers,secrets:[wrong,vector.secret],now:vector.now,tolerance:0)
        precondition(valid == body)
        let payload = try StandardWebhooks.verifyAndDecode(body,headers:headers,secrets:[vector.secret],as:Payload.self,now:vector.now)
        precondition(payload.value == 1)
        reject(body+Data(" ".utf8)); reject(body,headers,[wrong]);reject(body,headers,[vector.secret],vector.now+301)
        reject(body,headers,[vector.secret],vector.now-301);reject(body,headers,[])
        reject(body,headers,["whsec_bad"]);reject(body,headers,[vector.secret],vector.now,-1)
        var duplicate=headers;duplicate["Webhook-Id"]=["other"];reject(body,duplicate)
        var multi=headers;multi["webhook-id"]=["first","second"];reject(body,multi)
        var rotated=headers;rotated["webhook-signature"]=["v1a,ignored v1,bad "+vector.headers["webhook-signature"]!]
        let rotatedValid = try StandardWebhooks.verify(body,headers:rotated,secrets:[vector.secret],now:vector.now)
        precondition(rotatedValid == body)
        var unsupported=headers;unsupported["webhook-signature"]=["v1a,ignored"];reject(body,unsupported)
        do {_ = try StandardWebhooks.verifyAndDecode(body,headers:headers,secrets:[vector.secret],as:[String:Int].self,now:vector.now);fatalError()}
        catch {precondition(String(describing:error) == "verified webhook payload does not match destination type")}
    }
}
