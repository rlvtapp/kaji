<?php
declare(strict_types=1);
require getenv('POOLSTER_SYMFONY_VENDOR').'/autoload.php';
require __DIR__.'/vendor/autoload.php';
use Acme\Graphql\Client;
use Acme\Graphql\ReadUserVariables;
use Acme\Graphql\RenameVariables;
use Acme\Graphql\Symfony\AcmeGraphqlBundle;
use Acme\Graphql\Symfony\DependencyInjection\AcmeGraphqlExtension;
use Symfony\Component\DependencyInjection\ContainerBuilder;
use Symfony\Component\DependencyInjection\Definition;
use Symfony\Component\DependencyInjection\Reference;
use Symfony\Component\HttpClient\MockHttpClient;
use Symfony\Component\HttpClient\Response\MockResponse;
function check(bool $ok, string $message): void { if (!$ok) throw new RuntimeException($message); }
$bundle = new AcmeGraphqlBundle();
check($bundle->getContainerExtension() instanceof AcmeGraphqlExtension, 'bundle extension discovery');
foreach ([['endpoint'=>'https://example.test/graphql','timeout'=>0], ['headers'=>[]]] as $bad) {
    try { (new AcmeGraphqlExtension())->load([$bad], new ContainerBuilder()); throw new RuntimeException('accepted invalid config'); }
    catch (Symfony\Component\Config\Definition\Exception\InvalidConfigurationException $expected) {}
}
$requests=[];
$mock = new MockHttpClient(function($method,$url,$options) use (&$requests) {
    $payload=json_decode($options['body'],true,512,JSON_THROW_ON_ERROR);
    $requests[]=$payload;
    check($method==='POST' && $url==='https://example.test/graphql', 'request URL/method');
    check($options['max_redirects']===0 && $options['timeout']===2.0, 'request policy');
    check(in_array('authorization: bearer secret',array_map('strtolower',$options['headers']),true), 'auth header');
    $id=$payload['variables']['id'];
    if($id==='status') return new MockResponse('unavailable',['http_code'=>503]);
    if($id==='malformed') return new MockResponse('{bad');
    $field=$payload['operationName']==='Rename'?'rename':'user';
    $body=['data'=>[$field=>['id'=>$id,'name'=>$payload['variables']['name']??'Ada','when'=>'2026-10-10']]];
    if($id==='partial') $body['errors']=[['message'=>'partial']];
    return new MockResponse(json_encode($body,JSON_THROW_ON_ERROR));
});
$container = new ContainerBuilder();
$container->set('graphql.scoped_client',$mock);
(new AcmeGraphqlExtension())->load([['endpoint'=>'https://example.test/graphql','headers'=>['Authorization'=>'Bearer secret'],'timeout'=>2,'http_client'=>'graphql.scoped_client']],$container);
$container->getDefinition(Client::class)->setArgument('$scalarCodecs',['DateTime'=>['decode'=>fn($v)=>'decoded:'.$v]]);
// A public consumer exercises normal private-service autowiring after compilation.
class Consumer { public function __construct(public Client $client) {} }
$container->setDefinition(Consumer::class,(new Definition(Consumer::class))->setAutowired(true)->setPublic(true));
$container->compile();
$client=$container->get(Consumer::class)->client;
function read(Client $client, string $id): mixed {
    $variables=new ReadUserVariables($id);
    return match($GLOBALS['argv'][1]) {
        'raw'=>Acme\Graphql\readUser($client,$variables),
        'flat'=>$client->readUser($variables),
        'custom'=>$client->users()->read($variables),
        default=>$client->query()->readUser($variables),
    };
}
$data=read($client,'42')->requireData();
check($data->user->name==='Ada' && $data->user->when==='decoded:2026-10-10','model and scalar decoding');
$v=new RenameVariables('42','Grace');
$mutation=match($argv[1]) { 'raw'=>Acme\Graphql\rename($client,$v), 'flat'=>$client->rename($v), 'custom'=>$client->users()->rename($v), default=>$client->mutation()->rename($v) };
check($mutation->requireData()->rename->name==='Grace','mutation');
$partial=read($client,'partial');check($partial->errors!==[] && $partial->data->present,'partial results');
try { $partial->requireData(); throw new RuntimeException('accepted partial'); } catch(Acme\Graphql\GraphqlException $expected) {}
try { read($client,'status'); throw new LogicException('accepted status'); } catch(RuntimeException $expected) {check(str_contains($expected->getMessage(),'503'),'status');}
try { read($client,'malformed'); throw new RuntimeException('accepted malformed'); } catch(JsonException $expected) {}
check(count($requests)===5,'mutation not retried');
echo "Symfony container and transport verified\n";

$native=Symfony\Component\HttpClient\HttpClient::create();
$live=new ContainerBuilder(); $live->set('http_client',$native);
(new AcmeGraphqlExtension())->load([['endpoint'=>getenv('POOLSTER_SYMFONY_ENDPOINT'),'headers'=>['Authorization'=>'Bearer secret']]],$live);
$live->getDefinition(Client::class)->setPublic(true); $live->compile();
$liveClient=$live->get(Client::class);
check(read($liveClient,'42')->requireData()->user->name==='Ada','live GraphQL query');
$v=new RenameVariables('42','Grace');
$r=match($argv[1]) { 'raw'=>Acme\Graphql\rename($liveClient,$v), 'flat'=>$liveClient->rename($v), 'custom'=>$liveClient->users()->rename($v), default=>$liveClient->mutation()->rename($v) };
check($r->requireData()->rename->name==='Grace','live GraphQL mutation');
echo "Local GraphQL server verified\n";
