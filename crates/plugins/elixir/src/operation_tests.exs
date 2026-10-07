# Generated bounded smoke cases. Run: mix deps.get && mix run test/operations_test.exs
# The injected transport never contacts an API.
ExUnit.start()
defmodule __MODULE__.OperationSmokeTests do
  use ExUnit.Case
  test "bounded public operations and malformed JSON" do
    fixtures = Jason.decode!(File.read!("tests/operation-fixtures.json"))
    for fixture <- fixtures["cases"] do
      transport = fn request, _ ->
        uri = URI.parse(request.url)
        path = Enum.reduce(fixture["parameters"], fixture["path"], fn parameter, path ->
          if parameter["location"] == "path", do: String.replace(path, "{" <> parameter["name"] <> "}", URI.encode(to_string(parameter["value"]), &URI.char_unreserved?/1)), else: path
        end)
        assert uri.path == path
        assert String.upcase(to_string(request.method)) == fixture["method"]
        query = fixture["parameters"] |> Enum.filter(&(&1["location"] == "query")) |> Map.new(&{&1["name"], to_string(&1["value"])})
        assert URI.decode_query(uri.query || "") == query
        for parameter <- fixture["parameters"], parameter["location"] == "header" do
          assert Enum.any?(request.headers, fn {key,value} -> String.downcase(key) == String.downcase(parameter["name"]) and value == to_string(parameter["value"]) end)
        end
        if fixture["body"] == nil, do: assert(request.body in [nil, ""]), else: assert(Jason.decode!(request.body) == fixture["body"])
        send(self(), :operation_request)
        body = if Process.get(:operation_malformed), do: "{broken", else: Jason.encode!(fixture["result"])
        {:ok, %Finch.Response{status: fixture["status"], headers: [{"content-type", fixture["content_type"]}], body: body}}
      end
      {:ok, client} = __MODULE__.Client.new(base_url: "https://operation-tests.invalid", transport: transport, max_retries: 0)
      options = Enum.map(fixture["parameters"], &{String.to_existing_atom(&1["argument"]), &1["value"]})
      options = if fixture["body"] == nil, do: options, else: Keyword.put(options, :body, fixture["body"])
      {:ok, result} = apply(__MODULE__.API, String.to_existing_atom(fixture["method_name"]), [client, options])
      assert __MODULE__.JSON.to_wire(result) == fixture["result"]
      assert_receive :operation_request
      refute_receive :operation_request
      Process.put(:operation_malformed,true)
      assert {:error, %Jason.DecodeError{}} = apply(__MODULE__.API, String.to_existing_atom(fixture["method_name"]), [client, options])
      assert_receive :operation_request
      refute_receive :operation_request
      Process.delete(:operation_malformed)
    end
  end
end
