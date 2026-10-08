import json, os
from contractsdk import Client

scenario = json.loads(os.environ.get('POOLSTER_CONTRACT_SCENARIO', '{}'))
def policy(request, following):
    request.add_header('X-Contract-Middleware', 'yes')
    return following(request)
client = Client(os.environ['POOLSTER_CONTRACT_URL'], api_key=os.environ['POOLSTER_CONTRACT_CASE'], middleware=(policy,), validate_responses=scenario.get('validate_responses', False), retry_initial_delay=0, retry_max_delay=0)
try:
    action = scenario.get('action', 'getContact')
    if action == 'echoWire':
        from contractsdk.models import WireInput
        result_id = client.echo_wire(key='café/雪', text='héllo 雪', flag=False, count=0, tags=['a', 'b'], x_label='caller', body=WireInput(enabled=False, count=0, note=None)).id
    elif action == 'listContactsPages':
        ids = [item.id for page in client.list_contacts_pages(page=scenario.get('page'), limit=scenario.get('limit')) for item in page.items]
        result_id = ','.join(ids)
    else:
        operations = {'getContact': client.get_contact, 'createContact': client.create_contact, 'patchContact': client.patch_contact, 'unsafeCreateContact': client.unsafe_create_contact, 'unsafePatchContact': client.unsafe_patch_contact}
        arguments = {'x_once': scenario['caller_key']} if 'caller_key' in scenario else {}
        for _ in range(scenario.get('repeats', 1)):
            model = operations[action](**arguments)
        result_id = model.id
except Exception:
    print(json.dumps({'outcome': 'error'}))
else:
    print(json.dumps({'outcome': 'success', 'id': result_id}))
