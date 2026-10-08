<?php
// Generated bounded smoke cases. Run: composer install && php tests/operations.php
// No live API, credentials, or network transport is used.
declare(strict_types=1);
require dirname(__DIR__) . '/vendor/autoload.php';

function operationWireEqual(mixed $expected, mixed $actual): bool {
    if (is_object($expected)) {
        if (!is_object($actual)) return false;
        $expected=get_object_vars($expected); $actual=get_object_vars($actual);
        ksort($expected); ksort($actual);
        if (array_keys($expected)!==array_keys($actual)) return false;
        foreach ($expected as $key=>$value) if (!operationWireEqual($value,$actual[$key])) return false;
        return true;
    }
    if (is_array($expected)) {
        if (!is_array($actual) || array_keys($expected)!==array_keys($actual)) return false;
        foreach ($expected as $key=>$value) if (!operationWireEqual($value,$actual[$key])) return false;
        return true;
    }
    if ((is_int($expected)||is_float($expected)) && (is_int($actual)||is_float($actual))) return $expected==$actual;
    return $expected===$actual;
}

$fixtures = json_decode(file_get_contents(__DIR__.'/operation-fixtures.json'), true, 512, JSON_THROW_ON_ERROR);
foreach ($fixtures['cases'] as $case) {
    $transport = new class($case) implements \Psr\Http\Client\ClientInterface {
        public int $calls=0;
        public bool $malformed=false;
        public function __construct(private array $case) {}
        public function sendRequest(\Psr\Http\Message\RequestInterface $request): \Psr\Http\Message\ResponseInterface {
            $this->calls++;
            $path=$this->case['path']; $query=[];
            foreach ($this->case['parameters'] as $parameter) {
                $value=is_bool($parameter['value'])?($parameter['value']?'true':'false'):(string)$parameter['value'];
                if ($parameter['location']==='path') $path=str_replace('{'.$parameter['name'].'}',rawurlencode($value),$path);
                if ($parameter['location']==='query') $query[$parameter['name']]=$value;
                if ($parameter['location']==='header' && $request->getHeaderLine($parameter['name'])!==$value) throw new \RuntimeException('Header mismatch');
            }
            parse_str($request->getUri()->getQuery(),$actual);
            if ($request->getMethod()!==$this->case['method'] || $request->getUri()->getPath()!==$path || $actual!==$query) throw new \RuntimeException('Request mismatch');
            if ($this->case['body']!==null) { if (json_decode((string)$request->getBody(),true,512,JSON_THROW_ON_ERROR)!==$this->case['body']) throw new \RuntimeException('Request body mismatch'); } elseif ((string)$request->getBody()!=='') throw new \RuntimeException('Unexpected request body');
            return new \Nyholm\Psr7\Response($this->case['status'],['Content-Type'=>$this->case['content_type']],$this->malformed?'{broken':$this->case['result_json']);
        }
    };
    $client = new \__MODULE__\Client($transport,'https://operation-tests.invalid',maxRetries:0);
    $arguments=array_column($case['parameters'],'value');
    if ($case['body']!==null) {
        $body=$case['body'];
        if ($case['body_model']!==null) { $model='__MODULE__\\Models\\'.$case['body_model']; $body=$model::fromArray($body); }
        array_splice($arguments,$case['body_required']?$case['first_optional']:count($arguments),0,[$body]);
    }
    $result=$client->{$case['method_name']}(...$arguments);
    $wire=json_decode(json_encode($result,JSON_THROW_ON_ERROR),false,512,JSON_THROW_ON_ERROR);
    $expected=json_decode($case['result_json'],false,512,JSON_THROW_ON_ERROR);
    if (!operationWireEqual($expected,$wire) || $transport->calls!==1) throw new \RuntimeException('Decoded result mismatch: '.$case['operation']);
    $transport->malformed=true;
    $failed=false;
    try { $client->{$case['method_name']}(...$arguments); } catch (\JsonException $error) { $failed=true; }
    if (!$failed || $transport->calls!==2) throw new \RuntimeException('Malformed JSON was accepted: '.$case['operation']);
}
echo count($fixtures['cases'])." operation smoke cases passed\n";
