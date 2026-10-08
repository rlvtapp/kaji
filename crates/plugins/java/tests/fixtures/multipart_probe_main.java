 var driver=new Driver(MAPPER.readTree("{\"status\":204,\"response\":null}"));
 var client=new Client(new ClientConfig("https://unused.example",null,"Authorization","Bearer",Map.of(),driver,Duration.ofSeconds(1),null,null));
 byte[] bytes=new byte[]{0,(byte)255,13,10,34,92,127};var file=new MultipartBody.FilePart("blob.bin","application/octet-stream",bytes);bytes[0]=42;
 var body=new UploadThingMultipartBody("café雪\r\n",false,0L,file,null);
 client.uploadThing(new Client.UploadThingRequest(body));check(driver.calls==1,"one request");
 try{client.uploadThing(new Client.UploadThingRequest(null));throw new AssertionError("required body");}catch(NullPointerException expected){}
 check(driver.calls==1,"invalid body executed");
 try{new MultipartBody.FilePart("bad\r\nname","application/octet-stream",bytes);throw new AssertionError("header injection");}catch(IllegalArgumentException expected){}
