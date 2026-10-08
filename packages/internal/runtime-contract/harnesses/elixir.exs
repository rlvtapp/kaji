policy = fn request, following -> following.(%{request | headers: [{"x-contract-middleware", "yes"} | request.headers]}) end
{:ok, client} = Contractsdk.Client.new(base_url: System.fetch_env!("KAJI_CONTRACT_URL"), api_key: System.fetch_env!("KAJI_CONTRACT_CASE"), middleware: [policy])
scenario = Jason.decode!(System.get_env("KAJI_CONTRACT_SCENARIO") || "{}")
action = Map.get(scenario, "action", "getContact")
options = if Map.has_key?(scenario, "caller_key"), do: [x_once: scenario["caller_key"]], else: []
invoke = fn ->
  case action do
    "getContact" -> Contractsdk.API.get_contact(client)
    "createContact" -> Contractsdk.API.create_contact(client, options)
    "patchContact" -> Contractsdk.API.patch_contact(client, options)
    "unsafeCreateContact" -> Contractsdk.API.unsafe_create_contact(client)
    "unsafePatchContact" -> Contractsdk.API.unsafe_patch_contact(client)
    "echoWire" -> Contractsdk.API.echo_wire(client, key: "café/雪", body: %Contractsdk.Models.WireInput{enabled: false, count: 0, note: nil}, text: "héllo 雪", flag: false, count: 0, tags: ["a", "b"], x_label: "caller")
    "listContactsPages" ->
      Contractsdk.API.list_contacts_pages(client, page: Map.get(scenario, "page", 1), limit: Map.get(scenario, "limit", 2))
      |> Enum.reduce_while({:ok, []}, fn
        {:ok, page}, {:ok, ids} -> {:cont, {:ok, ids ++ Enum.map(page.items, & &1.id)}}
        {:error, reason}, _ -> {:halt, {:error, reason}}
      end)
      |> case do
        {:ok, ids} -> {:ok, %{id: Enum.join(ids, ",")}}
        error -> error
      end
    _ -> raise "unsupported contract action"
  end
end
result = Enum.reduce_while(1..Map.get(scenario, "repeats", 1), nil, fn _, _ ->
  case invoke.() do
    {:ok, model} -> {:cont, %{outcome: "success", id: model.id}}
    {:error, _reason} -> {:halt, %{outcome: "error"}}
  end
end)
IO.puts(Jason.encode!(result))
