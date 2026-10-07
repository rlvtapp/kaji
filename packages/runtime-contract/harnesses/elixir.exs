policy = fn request, following -> following.(%{request | headers: [{"x-contract-middleware", "yes"} | request.headers]}) end
{:ok, client} = Contractsdk.Client.new(base_url: System.fetch_env!("KAJI_CONTRACT_URL"), api_key: System.fetch_env!("KAJI_CONTRACT_CASE"), middleware: [policy])
result = case Contractsdk.API.get_contact(client) do
  {:ok, model} -> %{outcome: "success", id: model.id}
  {:error, _reason} -> %{outcome: "error"}
end
IO.puts(Jason.encode!(result))
