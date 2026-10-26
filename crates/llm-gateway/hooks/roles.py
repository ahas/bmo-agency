"""Starts a request from its role's saved prefix on an instance that keeps role files.

An engine instance that saves and restores its conversation state keeps one file per role, the state of
that role's system prompt read once, named `role-<role>.bin`; restoring it costs a fraction of a second
where reading the prompt again costs many (docs/own-llm.md, 「Role files」). A run whose work key names a
role says so in a header on every request. After the instance a request goes to is chosen and before the
request is sent, this hook asks that instance to restore the role's file where the instance is not on that
role already, then lets the request go on. Which files stand, and whether one is still valid, are the
engine deployment's own; this hook never saves one.

For each instance the hook keeps the role it last restored or served: a request naming no role leaves the
instance on none, since what it reads replaces the role's state. A restore that is refused or not answered
leaves the request to go as it stands, with a line in the log, and that role is not asked of that instance
again for ten minutes. What it keeps is this process's: a new gateway child starts with every instance on
no role.
"""

import asyncio
import json
import re
import sys
import time
import urllib.error
import urllib.request

from litellm.integrations.custom_logger import CustomLogger

# The header a run names its role in, as the program's custom headers send it.
ROLE_HEADER = "x-bmo-role"
# What a role's name looks like; anything else is no role.
ROLE_NAME = re.compile(r"^[a-z][a-z0-9-]{0,39}$")
# How long a role an instance could not restore is not asked of it again, in seconds.
RETRY_AFTER = 600
# How long a restore is waited for, in seconds.
RESTORE_WAIT = 30

# Each instance's role, by its root address: the one it last restored or served, None for none.
on_role: dict = {}
# When each role an instance could not restore may be asked of it again, by (root, role).
refused_until: dict = {}
# One check and restore at a time for each instance, so two requests do not restore it twice.
locks: dict = {}


def log(text: str) -> None:
    print(f"llm-gateway roles: {text}", file=sys.stderr, flush=True)


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


def role_of(kwargs: dict):
    """The role the request names, or None."""
    for key, value in headers_of(kwargs).items():
        if isinstance(key, str) and key.lower() == ROLE_HEADER and isinstance(value, str):
            role = value.strip()
            return role if ROLE_NAME.match(role) else None
    return None


def keeps_role_files(kwargs: dict) -> bool:
    info = kwargs.get("model_info")
    return isinstance(info, dict) and info.get("role_files") is True


def instance_root(api_base) -> str:
    """The instance's own address: its chat address without the path the OpenAI client asks under."""
    if not isinstance(api_base, str):
        return ""
    root = api_base.rstrip("/")
    for tail in ("/chat/completions", "/v1"):
        if root.endswith(tail):
            root = root[: -len(tail)]
    return root


def restore(root: str, key, role: str):
    """Whether the instance restored the role's file, and why not where it did not."""
    body = json.dumps({"filename": f"role-{role}.bin"}).encode()
    headers = {"Content-Type": "application/json"}
    if isinstance(key, str) and key and key != "no-key":
        headers["Authorization"] = f"Bearer {key}"
    asked = urllib.request.Request(f"{root}/slots/0?action=restore", data=body, method="POST", headers=headers)
    try:
        with urllib.request.urlopen(asked, timeout=RESTORE_WAIT) as answer:
            answer.read()
            return True, None
    except urllib.error.HTTPError as refusal:
        return False, f"the instance answered {refusal.code}"
    except Exception as failure:  # no answer, a refused connection or a wait run out
        return False, f"no answer: {failure}"


async def start_from_role(kwargs: dict) -> None:
    """The instance the request goes to put on the request's role first, where it keeps role files."""
    if not keeps_role_files(kwargs):
        return
    root = instance_root(kwargs.get("api_base"))
    if not root:
        return
    role = role_of(kwargs)
    lock = locks.setdefault(root, asyncio.Lock())
    async with lock:
        if role is None:
            on_role[root] = None
            return
        if on_role.get(root) == role:
            return
        if refused_until.get((root, role), 0) > time.monotonic():
            on_role[root] = None
            return
        began = time.monotonic()
        restored, why = await asyncio.to_thread(restore, root, kwargs.get("api_key"), role)
        took = time.monotonic() - began
        if restored:
            on_role[root] = role
            log(f"{root} restored role-{role}.bin in {took:.2f} s")
        else:
            on_role[root] = None
            refused_until[(root, role)] = time.monotonic() + RETRY_AFTER
            log(f"{root} did not restore role-{role}.bin ({why}); the request goes as it stands, and the role is not asked of it again for {RETRY_AFTER // 60} minutes")


class RoleStarter(CustomLogger):
    async def async_pre_call_deployment_hook(self, kwargs, call_type):
        await start_from_role(kwargs)
        return None


handler = RoleStarter()
