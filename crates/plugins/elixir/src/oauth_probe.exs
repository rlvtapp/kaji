{:ok, calls} = Agent.start_link(fn -> 0 end)
fetch = fn _url, headers, body ->
  true = {"authorization", "Basic aWQ6c2VjcmV0"} in headers
  true = body == "grant_type=client_credentials"
  Agent.update(calls, &(&1 + 1))
  Process.sleep(20)
  {:ok, 200, ~s({"access_token":"fresh","token_type":"Bearer","expires_in":60})}
end
{:ok, provider} = ProbeSdk.OAuthClientCredentials.start_link(token_url: "https://issuer.test/token", client_id: "id", client_secret: "secret", fetch: fetch)
results = 1..16 |> Enum.map(fn _ -> Task.async(fn -> ProbeSdk.OAuthClientCredentials.token(provider) end) end) |> Enum.map(&Task.await/1)
true = Enum.all?(results, &(&1 == {:ok, "fresh"}))
1 = Agent.get(calls, & &1)
:ok = ProbeSdk.OAuthClientCredentials.invalidate(provider)
{:ok, "fresh"} = ProbeSdk.OAuthClientCredentials.token(provider)
2 = Agent.get(calls, & &1)
layer = ProbeSdk.OAuthClientCredentials.middleware(provider, "https://api.test")
request = %{scheme: :https, host: "api.test", port: 443, headers: [], method: "GET", body: nil}
{:ok, "Bearer fresh"} = layer.(request, fn request -> {:ok, Keyword.get(request.headers, :authorization) || Map.new(request.headers)["authorization"]} end)
{:error, :oauth_origin_mismatch} = layer.(%{request | host: "evil.test"}, fn _ -> raise "must not execute" end)
{:ok, attempts} = Agent.start_link(fn -> 0 end)
{:ok, %{status: 401}} = layer.(request, fn _ -> Agent.update(attempts, &(&1 + 1)); {:ok, %{status: 401}} end)
2 = Agent.get(attempts, & &1)
{:ok, %{status: 401}} = layer.(%{request | method: "POST"}, fn _ -> Agent.update(attempts, &(&1 + 1)); {:ok, %{status: 401}} end)
3 = Agent.get(attempts, & &1)
IO.puts("oauth singleflight origin passed")

{:ok, base} = ProbeSdk.Client.new(base_url: "https://api.test", headers: [{"X-Scope", "base"}])
scoped = ProbeSdk.Client.for_call(base, headers: [{"x-scope", "call"}], timeout: 100)
[{"x-scope", "call"}] = scoped.headers
100 = scoped.timeout
[{"X-Scope", "base"}] = base.headers
