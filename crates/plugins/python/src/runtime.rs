//! Generated Python transport runtime.
use super::*;
mod client;

pub(super) fn render_runtime(api: &Api) -> String {
    let errors = if partition_python_errors(api) {
        "from .errors import *\n".into()
    } else {
        render_declared_error_classes(api)
    };
    let mut output = format!(
        r#"{NOTICE}
from __future__ import annotations

import json
import time
import re
from dataclasses import fields, is_dataclass, replace
from typing import Any, Callable, Iterator, cast
from urllib.error import HTTPError, URLError
from urllib.parse import quote, urlencode, urljoin, urlsplit
from urllib.request import Request, urlopen

from .models import *
from .models import _to_wire as to_wire
from .multipart import MultipartBody, FilePart, JsonPart, RawJsonPart, encode_multipart
from .response_validation import ResponseDecodeError


class ApiError(Exception):
    """An unsuccessful HTTP response returned by the API.

    ``body`` is decoded using the OpenAPI error response schema, when one was
    declared. ``headers`` retains request IDs and rate-limit information.
    """

    def __init__(self, status_code: int, headers: dict[str, str], body: Any) -> None:
        self.status_code = status_code
        self.headers = headers
        self.body = body
        super().__init__(f"API request failed with status {{status_code}}")


{errors}
def _poolster_json_path(value: Any, path: str) -> Any:
    """Supported paths start at ``$`` or use RFC 6901 JSON pointers; never evaluate code."""
    current = to_wire(value)
    if path.startswith("/"):
        segments = []
        for item in path[1:].split("/"):
            if re.search(r"~(?![01])", item):
                return None
            segments.append(item.replace("~1", "/").replace("~0", "~"))
    elif path.startswith("$"):
        segments = []
        rest = path[1:]
        while rest:
            match = re.match(r"(?:\.([^.[\]]+)|\[(-?\d+)\])", rest)
            if match is None:
                return None
            segments.append(match.group(1) if match.group(1) is not None else int(match.group(2)))
            rest = rest[match.end():]
    else:
        return None
    for segment in segments:
        if isinstance(current, list):
            if path.startswith("/") and (not isinstance(segment, str) or re.fullmatch(r"0|[1-9][0-9]*", segment) is None):
                return None
            try:
                index = int(segment)
            except (ValueError, TypeError):
                return None
            if index < -len(current) or index >= len(current):
                return None
            current = current[index]
        elif isinstance(current, dict) and segment in current:
            current = current[segment]
        else:
            return None
    return current


def _poolster_with_body_value(body: Any, wire_name: str, value: Any) -> Any:
    """Copy a generated JSON request body with one declared field replaced."""
    if is_dataclass(body) and not isinstance(body, type):
        for item in fields(body):
            if item.metadata.get("wire_name", item.name) == wire_name:
                result = replace(body, **{{item.name: value}})
                for internal in fields(body):
                    if internal.metadata.get("present_fields"):
                        previous = getattr(body, internal.name)
                        if previous is not None:
                            setattr(result, internal.name, previous | {{wire_name}})
                return result
        raise TypeError(f"request body has no declared {{wire_name!r}} field")
    if isinstance(body, dict):
        return {{**body, wire_name: value}}
    raise TypeError("body pagination requires a generated dataclass or dictionary JSON body")


"#
    );
    output.push_str(&client::render_runtime_client(api));
    output
}
