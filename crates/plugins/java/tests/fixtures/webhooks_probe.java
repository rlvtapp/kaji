package probe;
import java.util.*;
import java.nio.charset.StandardCharsets;
class Probe {
    static final byte[] BODY = __BODY__.getBytes(StandardCharsets.UTF_8);
    static final String SECRET = __SECRET__;
    static final Map<String,List<String>> HEADERS = Map.of("webhook-id",List.of("msg_test"),"webhook-timestamp",List.of("1700000000"),"webhook-signature",List.of(__SIGNATURE__));
    static void reject(byte[] body,Map<String,List<String>> headers,List<String> secrets,double now,double tolerance) {
        try {StandardWebhooks.verify(body,headers,secrets,now,tolerance);throw new AssertionError();}
        catch(StandardWebhooks.VerificationException error) {if(error.getMessage().contains(SECRET)||error.getMessage().contains("ngcu"))throw new AssertionError();}
    }
    public static void main(String[] args) {
        var wrong="whsec_"+Base64.getEncoder().encodeToString(new byte[24]);
        if(!Arrays.equals(BODY,StandardWebhooks.verify(BODY,HEADERS,List.of(wrong,SECRET),1700000000,0)))throw new AssertionError();
        reject(Arrays.copyOf(BODY,BODY.length+1),HEADERS,List.of(SECRET),1700000000,300);
        reject(BODY,HEADERS,List.of(wrong),1700000000,300);
        reject(BODY,HEADERS,List.of(SECRET),1700000301,300);
        reject(BODY,HEADERS,List.of(SECRET),1699999699,300);
        reject(BODY,HEADERS,List.of(),1700000000,300);
        reject(BODY,HEADERS,List.of("whsec_bad"),1700000000,300);
        var duplicate=new HashMap<>(HEADERS);duplicate.put("Webhook-Id",List.of("other"));reject(BODY,duplicate,List.of(SECRET),1700000000,300);
        var multiple=new HashMap<>(HEADERS);multiple.put("webhook-id",List.of("one","two"));reject(BODY,multiple,List.of(SECRET),1700000000,300);
        var rotation=new HashMap<>(HEADERS);rotation.put("webhook-signature",List.of("v1a,ignored v1,bad "+HEADERS.get("webhook-signature").get(0)));
        StandardWebhooks.verify(BODY,rotation,List.of(SECRET),1700000000,300);
        var unsupported=new HashMap<>(HEADERS);unsupported.put("webhook-signature",List.of("v1a,ignored"));reject(BODY,unsupported,List.of(SECRET),1700000000,300);
        if(StandardWebhooks.verifyAndDecode(BODY,HEADERS,List.of(SECRET),1700000000,300,bytes -> bytes.length)!=BODY.length)throw new AssertionError();
        try {StandardWebhooks.verifyAndDecode(BODY,HEADERS,List.of(SECRET),1700000000,300,bytes -> {throw new IllegalArgumentException(SECRET);});throw new AssertionError();}
        catch(StandardWebhooks.VerificationException error){if(!error.getMessage().equals("verified webhook payload does not match destination type"))throw new AssertionError();}
    }
}
