defmodule Finch.Request do
  defstruct [:method, :url, :body, headers: []]
  @type t :: %__MODULE__{}
end
defmodule Finch.Response do
  defstruct status: 200, headers: [], body: ""
end
defmodule Finch do
  def build(method,url,headers,body), do: struct(Finch.Request,method: method,url: url,headers: headers,body: body)
  def request(_,_,_), do: raise("Unexpected default transport")
  def stream(_,_,_,_,_), do: raise("Unexpected default stream")
end
defmodule Probe.ApiError do
  defexception [:status,:body,:headers]
end
defmodule Probe.JSON do
  def decode(value), do: {:ok,value}
  def to_wire(value), do: value
end
Code.compile_file("client.ex")
Code.compile_file("operations.ex")
defmodule OffsetProbe do
  alias Probe.Client
  def list_pets(_,options) do
    offset=Keyword.fetch!(options,:offset)
    if options[:tenant] != "preserved", do: raise("Lost caller option")
    Process.put(:offsets,Process.get(:offsets,[])++[offset])
    {:ok, if(offset == 0, do: ["a","b"], else: ["c"])}
  end
__OFFSET_HELPER__
end
stream=OffsetProbe.list_pets_pages(nil,limit: 2,tenant: "preserved")
if Process.get(:offsets) != nil, do: raise("Eager offset request")
if Enum.to_list(stream) != [{:ok,["a","b"]},{:ok,["c"]}], do: raise("Offset pages")
if Process.get(:offsets) != [0,2], do: raise("Offset advance")
Process.delete(:offsets)
if Enum.to_list(OffsetProbe.list_pets_pages(nil,offset: -1)) != [{:error,:invalid_pagination_offset}], do: raise("Negative offset accepted")
if Process.get(:offsets) != nil, do: raise("Invalid offset request executed")
transport=fn request,_ ->
  if {"authorization","Bearer test-token"} not in request.headers, do: raise("Lost same-origin auth")
  Process.put(:urls,Process.get(:urls,[])++[request.url])
  next=if String.contains?(request.url,"page=2"), do: nil, else: "https://api.example.test/pets?page=2"
  {:ok,struct(Finch.Response,body: %{"next"=>next})}
end
{:ok,client}=Probe.Client.new(base_url: "https://api.example.test",api_key: "test-token",transport: transport,max_retries: 0)
stream=Probe.API.Operations0000.list_pets_pages(client)
if Process.get(:urls) != nil, do: raise("Eager URL request")
if length(Enum.to_list(stream)) != 2, do: raise("URL pages")
if Process.get(:urls) != ["https://api.example.test/pets","https://api.example.test/pets?page=2"], do: raise("URL query was changed")
for target <- ["https://evil.example/pets","http://api.example.test/pets","https://api.example.test:444/pets","https://user@api.example.test/pets","https://api.example.test/pets#fragment","/relative","?page=2"] do
  Process.delete(:urls)
  terminal=fn request,_ ->
    Process.put(:urls,[request.url|Process.get(:urls,[])])
    {:ok,struct(Finch.Response,body: %{"next"=>target})}
  end
  {:ok,client}=Probe.Client.new(base_url: "https://api.example.test",api_key: "test-token",transport: terminal,max_retries: 0)
  [first,{:error,:unsafe_pagination_url}]=Enum.to_list(Probe.API.Operations0000.list_pets_pages(client))
  {:ok,_}=first
  if Process.get(:urls) != ["https://api.example.test/pets"], do: raise("Unsafe URL reached transport")
end
Process.delete(:urls)
loop=fn request,_ -> Process.put(:urls,[request.url|Process.get(:urls,[])]); {:ok,struct(Finch.Response,body: %{"next"=>"https://api.example.test/pets"})} end
{:ok,client}=Probe.Client.new(base_url: "https://api.example.test",transport: loop,max_retries: 0)
[{:ok,_},{:ok,_},{:error,:pagination_loop}]=Enum.to_list(Probe.API.Operations0000.list_pets_pages(client))
if length(Process.get(:urls)) != 2, do: raise("URL loop was unbounded")
