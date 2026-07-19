"""Python client for talking to Mote robots over the network."""

from mote_link import _generated
from mote_link._generated import *  # noqa: F401,F403
from mote_link.link import (
    MoteClient,
    MoteConnectionError,
    MoteError,
    MoteProtocolError,
)

__all__ = _generated.__all__ + [
    "MoteClient",
    "MoteError",
    "MoteConnectionError",
    "MoteProtocolError",
]
