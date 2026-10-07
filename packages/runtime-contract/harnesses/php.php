<?php
// A real loopback wire adapter; decorators retain the SDK's PSR-18 boundary.
declare(strict_types=1);
require getcwd() . '/vendor/autoload.php';
use Psr\Http\Client\ClientInterface;
use Psr\Http\Message\RequestInterface;
use Psr\Http\Message\ResponseInterface;
use Nyholm\Psr7\Response;
final class NativeWire implements ClientInterface {
    public function sendRequest(RequestInterface $request): ResponseInterface {
        $headers = [];
        foreach ($request->getHeaders() as $name => $values) foreach ($values as $value) $headers[] = $name . ': ' . $value;
        $context = stream_context_create(['http' => ['method' => $request->getMethod(), 'header' => implode("\r\n", $headers), 'ignore_errors' => true, 'timeout' => 10]]);
        $body = file_get_contents((string)$request->getUri(), false, $context);
        if ($body === false) throw new RuntimeException('loopback transport failed');
        if (!preg_match('/^HTTP\/\S+ (\d+)/', $http_response_header[0], $status)) throw new RuntimeException('invalid loopback response');
        return new Response((int)$status[1], ['Content-Type' => 'application/json'], $body);
    }
}
final class Policy implements ClientInterface {
    public function __construct(private readonly ClientInterface $next) {}
    public function sendRequest(RequestInterface $request): ResponseInterface {
        return $this->next->sendRequest($request->withHeader('X-Contract-Middleware','yes'));
    }
}
$metadata = json_decode(file_get_contents('composer.json'), true, flags: JSON_THROW_ON_ERROR);
$prefix = array_key_first($metadata['autoload']['psr-4']);
$type = $prefix . 'Client';
$client = new $type(new Policy(new NativeWire()), getenv('KAJI_CONTRACT_URL'), getenv('KAJI_CONTRACT_CASE'));
try { $model=$client->getContact(); echo json_encode(['outcome'=>'success','id'=>$model->id],JSON_THROW_ON_ERROR)."\n"; }
catch (Throwable) { echo "{\"outcome\":\"error\"}\n"; }
