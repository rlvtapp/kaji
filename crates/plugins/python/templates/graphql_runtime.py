"""GraphQL HTTP envelopes preserve partial results separately from transport failures."""
from dataclasses import dataclass
from typing import Any, Generic, Optional, TypeVar
import json
from urllib.request import Request, urlopen
from urllib.error import HTTPError

T = TypeVar("T")

class GraphqlErrors(Exception):
    def __init__(self, errors, data=None):
        super().__init__("GraphQL response contains errors")
        self.errors = errors
        self.data = data

@dataclass(frozen=True)
class GraphqlResponse(Generic[T]):
    data: Optional[T]
    errors: list[dict[str, Any]]
    extensions: Optional[dict[str, Any]] = None
    data_present: bool = False

    @property
    def status(self):
        return "partial" if self.errors and self.data is not None else "error" if self.errors else "success"

    def require_data(self) -> T:
        if self.errors:
            raise GraphqlErrors(self.errors, self.data)
        if self.data is None:
            raise ValueError("GraphQL response has no data")
        return self.data

class Transport:
    def __init__(self, endpoint: str, *, headers=None, timeout: float = 30):
        self.endpoint = endpoint
        self.headers = {"Content-Type": "application/json", "Accept": "application/graphql-response+json, application/json", **(headers or {})}
        self.timeout = timeout

    def execute(self, document: str, operation: str, variables) -> GraphqlResponse:
        body = json.dumps({"query": document, "operationName": operation, "variables": variables}).encode()
        request = Request(self.endpoint, data=body, headers=self.headers, method="POST")
        try:
            with urlopen(request, timeout=self.timeout) as response:
                value = json.load(response)
        except HTTPError as error:
            # GraphQL-over-HTTP may return a valid error envelope with a 4xx status.
            try:
                value = json.load(error)
            except (ValueError, UnicodeDecodeError):
                raise error
            if not isinstance(value, dict) or not value.get("errors"):
                raise error
        if not isinstance(value, dict) or not ("data" in value or "errors" in value):
            raise ValueError("Invalid GraphQL response envelope")
        errors = value.get("errors", [])
        if not isinstance(errors, list) or any(not isinstance(error, dict) or not isinstance(error.get("message"), str) for error in errors):
            raise ValueError("Invalid GraphQL errors")
        data = value.get("data")
        if data is not None and not isinstance(data, dict):
            raise ValueError("Invalid GraphQL data")
        extensions = value.get("extensions")
        if extensions is not None and not isinstance(extensions, dict):
            raise ValueError("Invalid GraphQL extensions")
        if "data" not in value and not errors:
            raise ValueError("GraphQL response has neither data nor errors")
        return GraphqlResponse(data, errors, extensions, "data" in value)
