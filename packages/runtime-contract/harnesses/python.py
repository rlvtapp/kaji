import json, os
from contractsdk import Client

def policy(request, following):
    request.add_header("X-Contract-Middleware", "yes")
    return following(request)

client = Client(os.environ["KAJI_CONTRACT_URL"], api_key=os.environ["KAJI_CONTRACT_CASE"], middleware=(policy,))
try:
    model = client.get_contact()
except Exception:
    print(json.dumps({"outcome": "error"}))
else:
    print(json.dumps({"outcome": "success", "id": model.id}))
