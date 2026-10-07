<?php
require 'src/Webhooks.php';
use Security\Sdk\Webhooks;
$body = '{ "data": "h\u00e9llo", "value": 1 }';
$secret = 'whsec_MDEyMzQ1Njc4OWFiY2RlZmdoaWprbG1u';
$headers = ['webhook-id'=>'msg_test','webhook-timestamp'=>'1700000000','webhook-signature'=>'v1,ngcuVH7fbp2oRZsP2NHR5ipUCWyM9B2osQVfxhlUeAk='];
$check = static function(bool $condition): void { if (!$condition) { throw new RuntimeException('Assertion failed'); } };
$check(Webhooks::verify($body,$headers,[$secret],1700000000,0) === $body);
$check(Webhooks::verifyAndDecode($body,$headers,[$secret],fn($raw)=>json_decode($raw,true,512,JSON_THROW_ON_ERROR),1700000000)['value'] === 1);
$rotated = $headers; $rotated['webhook-signature']='v2,ignored v1,bad '.$headers['webhook-signature'];
$check(Webhooks::verify($body,$rotated,['whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4',$secret],1700000300) === $body);
$cases = [[$body.' ',$headers,[$secret],1700000000],[$body,$headers,[$secret],1700000301],[$body,$headers,[$secret],1699999699],[$body,$headers,[],1700000000],[$body,$headers,['whsec_bad'],1700000000],[$body,$headers+['WEBHOOK-ID'=>'duplicate'],[$secret],1700000000],[$body,array_replace($headers,['webhook-signature'=>str_replace('v1,','v1a,',$headers['webhook-signature'])]),[$secret],1700000000],[$body,array_replace($headers,['webhook-id'=>['msg_test']]),[$secret],1700000000]];
foreach ($cases as [$raw,$metadata,$keys,$now]) {
    try { Webhooks::verify($raw,$metadata,$keys,$now); throw new RuntimeException('Invalid webhook accepted'); }
    catch (InvalidArgumentException $error) { $check($error->getMessage() === 'Invalid webhook'); }
}
foreach ([[INF,300],[1700000000,NAN],[1700000000,-1]] as [$now,$tolerance]) {
    try { Webhooks::verify($body,$headers,[$secret],$now,$tolerance); throw new RuntimeException('Invalid clock accepted'); }
    catch (InvalidArgumentException $error) { $check($error->getMessage() === 'Invalid webhook'); }
}
$pairs = array_map(fn($name,$value)=>[$name,$value],array_keys($headers),array_values($headers));
$check(Webhooks::verify($body,$pairs,[$secret],1700000000) === $body);
$pairs[]=['webhook-id','duplicate'];
try { Webhooks::verify($body,$pairs,[$secret],1700000000); throw new RuntimeException('Duplicate accepted'); }
catch (InvalidArgumentException $error) { $check($error->getMessage() === 'Invalid webhook'); }

$called=false;
try { Webhooks::verifyAndDecode($body.' ', $headers, [$secret], function($raw) use (&$called) { $called=true; },1700000000); throw new RuntimeException('Tamper accepted'); }
catch (InvalidArgumentException $error) { $check(!$called); }
