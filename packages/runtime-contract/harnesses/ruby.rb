require 'json'
require 'contractsdk'
policy = ->(request, following) { request['X-Contract-Middleware'] = 'yes'; following.call(request) }
client = Contractsdk::Client.new(base_url: ENV.fetch('KAJI_CONTRACT_URL'), bearer_token: ENV.fetch('KAJI_CONTRACT_CASE'), middleware: [policy])
begin
  model = client.get_contact
  puts JSON.generate(outcome: 'success', id: model.id)
rescue StandardError
  puts JSON.generate(outcome: 'error')
end
