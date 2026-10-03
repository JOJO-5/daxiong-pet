"""Persistent local WebDriver transport shared by native regression scenarios."""
import atexit
import http.client
import json
import os

_transport = http.client.HTTPConnection("127.0.0.1", int(os.environ.get("E2E_PORT", "4444")), timeout=40)
atexit.register(_transport.close)


def request(method, path, data=None):
    # urllib forces Connection: close. WebKit omits that header in its response,
    # so tauri-driver can pool a closing native socket and reset the next command.
    # Keep HTTP/1.1 alive and drain each response. Input commands are never retried.
    _transport.request(method, path, body=None if data is None else json.dumps(data).encode(),
                       headers={"Content-Type": "application/json"})
    response = _transport.getresponse()
    value = json.loads(response.read()).get("value")
    if response.status >= 400 or isinstance(value, dict) and "error" in value:
        raise AssertionError(value)
    return value
