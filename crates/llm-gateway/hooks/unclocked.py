"""Takes the wait off a request naming none on an entry that has no wait of its own.

An entry of a card service's engine names no wait: a request on the deployment's own engine is waited for
however long the engine takes (docs/own-llm.md, 「The gateway」). LiteLLM has no value that means no wait —
an entry naming none falls to the proxy-wide default, and an infinite one fails the connection — so such
an entry is marked unclocked in its model information, and after the entry a request goes to is chosen and
before the request is sent, this hook hands the request a wait with no limit. A request that names its
caller's wait in the wait headers keeps that wait.
"""

import httpx

from litellm.integrations.custom_logger import CustomLogger

# The headers a caller names its wait in, read case-insensitively.
WAIT_HEADERS = ("x-litellm-timeout", "x-litellm-stream-timeout")


def headers_of(kwargs: dict) -> dict:
    """The request's headers, wherever the router left them beside the deployment's call."""
    places = [
        kwargs.get("proxy_server_request"),
        (kwargs.get("litellm_params") or {}).get("proxy_server_request"),
    ]
    for request in places:
        if isinstance(request, dict) and isinstance(request.get("headers"), dict):
            return request["headers"]
    for name in ("metadata", "litellm_metadata"):
        metadata = kwargs.get(name)
        if isinstance(metadata, dict) and isinstance(metadata.get("headers"), dict):
            return metadata["headers"]
    return {}


def unclocked(kwargs: dict) -> bool:
    info = kwargs.get("model_info")
    return isinstance(info, dict) and info.get("unclocked") is True


def names_a_wait(kwargs: dict) -> bool:
    return any(isinstance(name, str) and name.lower() in WAIT_HEADERS for name in headers_of(kwargs))


def take_the_wait_off(kwargs: dict):
    """The request with no limit on its wait, where its entry has none and it names none; else None."""
    if not unclocked(kwargs) or names_a_wait(kwargs):
        return None
    kwargs["timeout"] = httpx.Timeout(None)
    kwargs.pop("stream_timeout", None)
    return kwargs


class Unclocked(CustomLogger):
    async def async_pre_call_deployment_hook(self, kwargs, call_type):
        return take_the_wait_off(kwargs)


handler = Unclocked()
