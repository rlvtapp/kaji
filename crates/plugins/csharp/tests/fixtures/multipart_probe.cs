using System;
using System.Net;
using System.Net.Http;
using System.Text;
using System.Text.RegularExpressions;
using Poolster.PoolsterMultipart;
class Driver:HttpMessageHandler {
 public int Calls;
 protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken cancellationToken){
  Calls++;if(request.Method!=HttpMethod.Post||request.RequestUri!.AbsolutePath!="/upload")throw new Exception("method/path");
  var type=request.Content!.Headers.ContentType!;if(type.MediaType!="multipart/form-data")throw new Exception("media");var boundary=type.Parameters.Single(p=>p.Name=="boundary").Value!.Trim('"');
  var raw=Encoding.Latin1.GetString(await request.Content.ReadAsByteArrayAsync(cancellationToken));var chunks=raw.Split("--"+boundary,StringSplitOptions.None);if(chunks.Length!=6||chunks[0]!=""||chunks[5]!="--\r\n")throw new Exception("boundaries");var parsed=new Dictionary<string,byte[]>();
  for(int i=1;i<5;i++){var part=chunks[i];if(!part.StartsWith("\r\n")||!part.EndsWith("\r\n"))throw new Exception("CRLF");part=part[2..^2];int split=part.IndexOf("\r\n\r\n",StringComparison.Ordinal);if(split<0)throw new Exception("separator");var header=part[..split];var match=Regex.Match(header,"name=\"?([^\";\\r\\n]+)\"?");if(!match.Success)throw new Exception("name");var name=match.Groups[1].Value.Trim();parsed.Add(name,Encoding.Latin1.GetBytes(part[(split+4)..]));if(name=="file"&&(!header.Contains("blob.bin")||!header.Contains("application/octet-stream")))throw new Exception("file metadata");}
  if(!parsed["title"].SequenceEqual(Encoding.UTF8.GetBytes("café雪\r\n"))||!parsed["flag"].SequenceEqual(Encoding.UTF8.GetBytes("false"))||!parsed["count"].SequenceEqual(Encoding.UTF8.GetBytes("0"))||!parsed["file"].SequenceEqual(new byte[]{0,255,13,10,34,92,127})||parsed.ContainsKey("missing"))throw new Exception("wire values");
  return new HttpResponseMessage(HttpStatusCode.NoContent){Content=new ByteArrayContent(Array.Empty<byte>())};
 }
}
class Probe {static async Task Main(){var driver=new Driver();var client=new PoolsterClient(new HttpClient(driver),new PoolsterClientOptions{BaseUrl="https://unused.example"});byte[] data={0,255,13,10,34,92,127};var file=new MultipartFile(data,"blob.bin");data[0]=42;var body=new UploadThingMultipartBody{Title="café雪\r\n",Flag=false,Count=0,File=file};await client.UploadThingAsync(body);if(driver.Calls!=1)throw new Exception("attempts");try{await client.UploadThingAsync(null!);throw new Exception("required body");}catch(ArgumentNullException){}if(driver.Calls!=1)throw new Exception("invalid execution");try{new MultipartFile(data,"bad\r\nname");throw new Exception("injection accepted");}catch(ArgumentException){}}}
