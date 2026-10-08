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
        $context = stream_context_create(['http' => ['method' => $request->getMethod(), 'header' => implode("\r\n", $headers), 'ignore_errors' => true, 'timeout' => 10, 'follow_location' => 0, 'content' => (string)$request->getBody()]]);
        $body = file_get_contents((string)$request->getUri(), false, $context);
        if ($body === false) throw new RuntimeException('loopback transport failed');
        if (!preg_match('/^HTTP\/\S+ (\d+)/', $http_response_header[0], $status)) throw new RuntimeException('invalid loopback response');
        $responseHeaders = [];
        foreach (array_slice($http_response_header, 1) as $line) {
            if (strpos($line, ':') !== false) { [$name, $value] = explode(':', $line, 2); $responseHeaders[trim($name)][] = trim($value); }
        }
        return new Response((int)$status[1], $responseHeaders, $body);
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
$scenario = json_decode(getenv('KAJI_CONTRACT_SCENARIO') ?: '{}', true, flags: JSON_THROW_ON_ERROR);
$action = $scenario['action'] ?? 'getContact';
$callerKey = $scenario['caller_key'] ?? null;
try {
    $id = null;
    for ($call = 0; $call < ($scenario['repeats'] ?? 1); $call++) {
        switch ($action) {
            case 'getContact': $id = $client->getContact()->id; break;
            case 'createContact': $id = $client->createContact(xOnce: $callerKey)->id; break;
            case 'patchContact': $id = $client->patchContact(xOnce: $callerKey)->id; break;
            case 'unsafeCreateContact': $id = $client->unsafeCreateContact()->id; break;
            case 'unsafePatchContact': $id = $client->unsafePatchContact()->id; break;
            case 'echoWire':
                $bodyType = $prefix . 'Models\\WireInput';
                $id = $client->echoWire(key: 'café/雪', body: new $bodyType(false, 0, null), text: 'héllo 雪', flag: false, count: 0, tags: ['a', 'b'], xLabel: 'caller')->id; break;
            case 'listContactsPages':
                $ids = [];
                foreach ($client->listContactsPages(page: $scenario['page'] ?? null, limit: $scenario['limit'] ?? null) as $page)
                    foreach ($page->items as $contact) $ids[] = $contact->id;
                $id = implode(',', $ids); break;
            default: throw new RuntimeException('unsupported contract action');
        }
    }
    echo json_encode(['outcome'=>'success','id'=>$id],JSON_THROW_ON_ERROR)."\n";
} catch (Throwable) { echo "{\"outcome\":\"error\"}\n"; }
