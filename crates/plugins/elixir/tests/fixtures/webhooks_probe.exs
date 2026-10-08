body = ~S({ "data": "h\u00e9llo", "value": 1 })
secret = "whsec_MDEyMzQ1Njc4OWFiY2RlZmdoaWprbG1u"
headers = %{"webhook-id" => "msg_test", "webhook-timestamp" => "1700000000", "webhook-signature" => "v1,ngcuVH7fbp2oRZsP2NHR5ipUCWyM9B2osQVfxhlUeAk="}
{:ok, ^body} = SecuritySdk.Webhooks.verify(body, headers, [secret], now: 1700000000, tolerance: 0)
{:decoded, ^body} = SecuritySdk.Webhooks.verify_and_decode(body, headers, [secret], &{:decoded, &1}, now: 1700000000)
rotated = Map.update!(headers, "webhook-signature", &("v2,ignored v1,bad " <> &1))
{:ok, ^body} = SecuritySdk.Webhooks.verify(body, rotated, ["whsec_YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4", secret], now: 1700000300)
cases = [{body <> " ", headers, [secret], 1700000000}, {body, headers, [secret], 1700000301}, {body, headers, [secret], 1699999699}, {body, headers, [], 1700000000}, {body, headers, ["whsec_bad"], 1700000000}, {body, Map.put(headers, "WEBHOOK-ID", "duplicate"), [secret], 1700000000}, {body, Map.update!(headers, "webhook-signature", &String.replace(&1, "v1,", "v1a,")), [secret], 1700000000}, {body, Map.put(headers, "webhook-id", ["msg_test"]), [secret], 1700000000}]
Enum.each(cases, fn {raw, metadata, keys, now} ->
  {:error, :invalid_webhook} = SecuritySdk.Webhooks.verify(raw, metadata, keys, now: now)
end)
{:error, :invalid_webhook} = SecuritySdk.Webhooks.verify(body, headers, [secret], now: 1700000000, tolerance: -1)
{:error, :invalid_webhook} = SecuritySdk.Webhooks.verify(body, headers, [secret], now: 1700000000.0)
{:ok, ^body} = SecuritySdk.Webhooks.verify(body, Map.to_list(headers), [secret], now: 1700000000)
{:error, :invalid_webhook} = SecuritySdk.Webhooks.verify(body, Map.to_list(headers) ++ [{"webhook-id", "duplicate"}], [secret], now: 1700000000)

{:error, :invalid_webhook} = SecuritySdk.Webhooks.verify_and_decode(body <> " ", headers, [secret], fn _ -> raise "Decoder must not run" end, now: 1700000000)
