"""GraphQL HTTP envelopes preserve partial results separately from transport failures."""

from dataclasses import dataclass
from typing import Any, Generic, Optional, TypeVar
import json
from urllib.request import Request, urlopen
from urllib.error import HTTPError
from .codecs import scalar_walk

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
        return (
            "partial"
            if self.errors and self.data is not None
            else "error"
            if self.errors
            else "success"
        )

    def require_data(self) -> T:
        if self.errors:
            raise GraphqlErrors(self.errors, self.data)
        if self.data is None:
            raise ValueError("GraphQL response has no data")
        return self.data


class Transport:
    def __init__(
        self,
        endpoint: str,
        *,
        headers=None,
        timeout: float = 30,
        scalar_codecs=None,
        max_frame_bytes=1024 * 1024,
    ):
        self.endpoint = endpoint
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/graphql-response+json, application/json",
            **(headers or {}),
        }
        self.timeout = timeout
        self.scalar_codecs = scalar_codecs or {}
        self.max_frame_bytes = max_frame_bytes
        if not isinstance(max_frame_bytes, int) or max_frame_bytes <= 0:
            raise ValueError("Positive max_frame_bytes required")

    def execute(
        self,
        document: str,
        operation: str,
        variables,
        variable_shape=None,
        result_shape=None,
        input_shapes=None,
    ) -> GraphqlResponse:
        variables = scalar_walk(
            variables, variable_shape, input_shapes or {}, self.scalar_codecs, "encode"
        )
        body = json.dumps(
            {"query": document, "operationName": operation, "variables": variables}
        ).encode()
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
        return envelope(value, result_shape, input_shapes, self.scalar_codecs)

    def subscribe(
        self,
        document,
        operation,
        variables,
        variable_shape=None,
        result_shape=None,
        input_shapes=None,
    ):
        from .streaming import sse_events

        variables = scalar_walk(
            variables, variable_shape, input_shapes or {}, self.scalar_codecs, "encode"
        )
        headers = {**self.headers, "Accept": "text/event-stream"}
        body = json.dumps(
            {"query": document, "operationName": operation, "variables": variables}
        ).encode()
        with urlopen(
            Request(self.endpoint, data=body, headers=headers, method="POST"), timeout=self.timeout
        ) as response:
            if response.headers.get_content_type() != "text/event-stream":
                raise ValueError("Subscription requires text/event-stream")
            for value in sse_events(response, self.max_frame_bytes):
                if "hasNext" in value or "incremental" in value:
                    raise ValueError("Incremental results require separate transport")
                yield envelope(value, result_shape, input_shapes, self.scalar_codecs)

    def incremental(
        self,
        document,
        operation,
        variables,
        variable_shape=None,
        result_shape=None,
        input_shapes=None,
    ):
        from .streaming import multipart_frames, apply_incremental

        variables = scalar_walk(
            variables, variable_shape, input_shapes or {}, self.scalar_codecs, "encode"
        )
        headers = {**self.headers, "Accept": "multipart/mixed; deferSpec=20220824"}
        body = json.dumps(
            {"query": document, "operationName": operation, "variables": variables}
        ).encode()
        snapshot, errors, extensions = {}, [], None
        with urlopen(
            Request(self.endpoint, data=body, headers=headers, method="POST"), timeout=self.timeout
        ) as response:
            for frame in multipart_frames(response, self.max_frame_bytes):
                if "data" in frame:
                    snapshot = frame["data"]
                snapshot = apply_incremental(snapshot, frame)
                errors.extend(frame.get("errors", []))
                if "extensions" in frame:
                    extensions = frame["extensions"]
                for patch in frame.get("incremental", []):
                    errors.extend(patch.get("errors", []))
                final = (
                    None
                    if frame["hasNext"]
                    else envelope(
                        {"data": snapshot, "errors": errors, "extensions": extensions},
                        result_shape,
                        input_shapes,
                        self.scalar_codecs,
                    )
                )
                yield IncrementalFrame(frame, snapshot, final)


@dataclass(frozen=True)
class IncrementalFrame:
    frame: dict[str, Any]
    data: Any
    final: Optional[GraphqlResponse]


def envelope(value, result_shape=None, input_shapes=None, scalar_codecs=None):
    if not isinstance(value, dict) or not ("data" in value or "errors" in value):
        raise ValueError("Invalid GraphQL response envelope")
    errors = value.get("errors", [])
    if not isinstance(errors, list) or any(
        not isinstance(error, dict) or not isinstance(error.get("message"), str) for error in errors
    ):
        raise ValueError("Invalid GraphQL errors")
    data = value.get("data")
    if data is not None and not isinstance(data, dict):
        raise ValueError("Invalid GraphQL data")
    extensions = value.get("extensions")
    if extensions is not None and not isinstance(extensions, dict):
        raise ValueError("Invalid GraphQL extensions")
    if "data" not in value and not errors:
        raise ValueError("GraphQL response has neither data nor errors")
    data = scalar_walk(data, result_shape, input_shapes or {}, scalar_codecs, "decode")
    return GraphqlResponse(data, errors, extensions, "data" in value)
