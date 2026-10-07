<?php
declare(strict_types=1);
require $argv[2];
require $argv[1];
use Psr\Http\Client\ClientInterface;
use Psr\Http\Message\RequestInterface;
use Psr\Http\Message\ResponseInterface;
use Nyholm\Psr7\Response;
use Nyholm\Psr7\Factory\Psr17Factory;
final class Issuer implements ClientInterface {
 public int $calls = 0;
 public bool $bad = false;
 public function sendRequest(RequestInterface $request): ResponseInterface {
  $this->calls++;
  assert($request->getHeaderLine('Authorization') === 'Basic aWQ6c2VjcmV0');
  assert((string)$request->getBody() === 'grant_type=client_credentials');
  return new Response(200, [], $this->bad ? '{"client_secret":"secret"}' : '{"access_token":"fresh","token_type":"Bearer","expires_in":60}');
 }
}
final class Terminal implements ClientInterface {
 public int $calls = 0;
 public bool $reject = false;
 public function sendRequest(RequestInterface $request): ResponseInterface {
  $this->calls++;
  assert(in_array($request->getHeaderLine('Authorization'), ['Bearer fresh','Custom'], true));
  return new Response($this->reject ? 401 : 204);
 }
}
$issuer = new Issuer(); $terminal = new Terminal(); $factory = new Psr17Factory();
$client = new ProbeSdk\OAuthClient($terminal, $issuer, $factory, $factory, 'https://api.test', 'https://issuer.test/token', 'id', 'secret');
$request = $factory->createRequest('GET', 'https://api.test/path');
for ($i=0; $i<4; $i++) { assert($client->sendRequest($request)->getStatusCode() === 204); }
assert($issuer->calls === 1);
$client->sendRequest($request->withHeader('Authorization', 'Custom')); assert($issuer->calls === 1);
try { $client->sendRequest($factory->createRequest('GET','https://evil.test/path')); throw new Exception('missing origin guard'); } catch (RuntimeException $e) { assert($e->getMessage() === 'OAuth origin mismatch'); }
$terminal->reject = true; $before = $terminal->calls;
$client->sendRequest($request); assert($terminal->calls === $before + 2);
$client->sendRequest($factory->createRequest('POST','https://api.test/path')); assert($terminal->calls === $before + 3);
$client->invalidate(); $issuer->bad = true;
try { $client->sendRequest($request); throw new Exception('missing rejection'); } catch (RuntimeException $e) { assert($e->getMessage() === 'OAuth token request failed'); }
assert(!str_contains(print_r($client,true), 'secret'));
echo "oauth cache origin redaction explicit-auth passed\n";

require $argv[3];
$base = new ProbeSdk\Client($terminal, 'https://api.test', defaultHeaders:['X-Scope'=>'base']);
$scoped = $base->forCall(['x-scope'=>'call'], $terminal);
$headers = new ReflectionProperty(ProbeSdk\Client::class, 'defaultHeaders');
assert($headers->getValue($base) === ['X-Scope'=>'base']);
assert($headers->getValue($scoped) === ['x-scope'=>'call']);
try { $base->forCall(["X-Bad"=>"bad\r\n"]); throw new Exception('missing header validation'); } catch (InvalidArgumentException) {}

$root = $argv[4];
spl_autoload_register(function(string $class) use ($root): void {
 if (str_starts_with($class, 'Probescoped\\')) { require $root.'/src/'.str_replace('\\','/',substr($class,strlen('Probescoped\\'))).'.php'; }
});
final class Capturing implements ClientInterface {
 public array $headers = [];
 public function sendRequest(RequestInterface $request): ResponseInterface { $this->headers[] = $request->getHeaderLine('X-Scope'); return new Response(204); }
}
$originalDriver = new Capturing(); $scopedDriver = new Capturing();
$original = new Probescoped\Client($originalDriver, 'https://api.test', defaultHeaders:['X-Scope'=>'base']);
$cached = $original->pets();
$call = $original->forCall(['x-scope'=>'call'], $scopedDriver);
assert($call->pets() !== $cached);
$call->pets()->get(); $original->pets()->get();
assert($scopedDriver->headers === ['call']); assert($originalDriver->headers === ['base']);
