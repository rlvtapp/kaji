 var driver=new Driver(MAPPER.readTree("{\"status\":204,\"response\":null}"));
 var client=new Client(new ClientConfig("https://api.test","static","Authorization","Bearer",Map.of(),driver,Duration.ofSeconds(30),new RetryConfig(1,Duration.ZERO,Duration.ZERO),null));
 var headers=new HashMap<String,String>();headers.put("X-Scope","scope");headers.put("Authorization","Own scope");
 var options=new ClientCallOptions(headers,Duration.ofMillis(25));var scoped=client.forCall(options);headers.put("X-Scope","changed");
 driver.expectedScope="scope";driver.expectedTimeout=Duration.ofMillis(25);driver.expectedAuth="Own scope";scoped.getThing();
 driver.expectedScope=null;driver.expectedTimeout=Duration.ofSeconds(30);driver.expectedAuth="Bearer static";client.getThing();check(driver.calls==2,"scopes reused shared HTTP driver");
 try{new ClientCallOptions(null,Duration.ZERO);throw new AssertionError("zero timeout");}catch(IllegalArgumentException expected){}
