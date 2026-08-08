"""Tests for outbound proxy configuration."""

from microsandbox import OutboundProxy, Sandbox


def test_socks5_proxy_serializes_as_structured_config() -> None:
    proxy = OutboundProxy.socks5("127.0.0.1:1080")

    assert proxy._to_dict() == {
        "protocol": "socks5",
        "address": "127.0.0.1:1080",
    }


def _native_create_error(**kwargs: object) -> Exception:
    try:
        Sandbox.create("proxy-parse-probe", image="alpine", **kwargs)
    except Exception as exc:
        return exc
    raise AssertionError("expected Sandbox.create to raise outside an event loop")


def test_native_create_accepts_top_level_proxy() -> None:
    baseline = _native_create_error()
    error = _native_create_error(proxy=OutboundProxy.socks5("127.0.0.1:1080"))

    assert type(error) is type(baseline), f"top-level proxy rejected: {error!r}"
