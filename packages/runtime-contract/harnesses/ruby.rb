require 'json'
require 'contractsdk'
scenario = JSON.parse(ENV.fetch('KAJI_CONTRACT_SCENARIO', '{}'))
policy = ->(request, following) { request['X-Contract-Middleware'] = 'yes'; following.call(request) }
client = Contractsdk::Client.new(base_url: ENV.fetch('KAJI_CONTRACT_URL'), bearer_token: ENV.fetch('KAJI_CONTRACT_CASE'), middleware: [policy], validate_responses: scenario.fetch('validate_responses', false))
begin
  action = scenario.fetch('action', 'getContact')
  if action == 'echoWire'
    id = client.echo_wire(key: 'café/雪', text: 'héllo 雪', flag: false, count: 0, tags: ['a','b'], x_label: 'caller', body: Contractsdk::Models::WireInput.new(enabled: false, count: 0, note: nil)).id
  elsif action == 'listContactsPages'
    id = client.list_contacts_pages(page: scenario['page'], limit: scenario['limit']).flat_map { |page| page.items.map(&:id) }.join(',')
  else
    methods = {'getContact'=>:get_contact,'createContact'=>:create_contact,'patchContact'=>:patch_contact,'unsafeCreateContact'=>:unsafe_create_contact,'unsafePatchContact'=>:unsafe_patch_contact}
    arguments = scenario.key?('caller_key') ? {x_once: scenario['caller_key']} : {}
    scenario.fetch('repeats', 1).times { @model = arguments.empty? ? client.public_send(methods.fetch(action)) : client.public_send(methods.fetch(action), **arguments) }
    id = @model.id
  end
  puts JSON.generate(outcome: 'success', id: id)
rescue StandardError
  puts JSON.generate(outcome: 'error')
end
