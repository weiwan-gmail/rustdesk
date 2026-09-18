import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  DIRECT_PORT,
  isDirectTarget,
  isHostnameTarget,
  isIpTarget,
  normalizeDirectTarget,
  resolveWebDirectRemoteId,
  usefulDirectConnectHost,
} from "./direct_target.ts";

describe("isIpTarget", () => {
  it("accepts IPv4 with optional port", () => {
    assert.equal(isIpTarget("192.168.1.50"), true);
    assert.equal(isIpTarget("192.168.1.50:21118"), true);
    assert.equal(isIpTarget(" 10.0.0.1 "), true);
  });
  it("accepts localhost so /direct still opens", () => {
    assert.equal(isIpTarget("localhost"), true);
    assert.equal(isIpTarget("LOCALHOST"), true);
    assert.equal(isIpTarget("localhost:21118"), true);
    assert.equal(isIpTarget("127.0.0.1"), true);
    assert.equal(isIpTarget("::1"), true);
    assert.equal(isIpTarget("[::1]"), true);
    assert.equal(isIpTarget("[::1]:21118"), true);
  });
  it("rejects numeric IDs and does not treat LAN names as IP literals", () => {
    assert.equal(isIpTarget("123456789"), false);
    assert.equal(isIpTarget("my-nas.local"), false);
    assert.equal(isIpTarget("localhost.example"), false);
  });
});

describe("isDirectTarget hostnames", () => {
  it("accepts split-horizon FQDN with and without port", () => {
    assert.equal(isHostnameTarget("xxx.yy.com"), true);
    assert.equal(isDirectTarget("xxx.yy.com"), true);
    assert.equal(isDirectTarget("xxx.yy.com:21118"), true);
  });
  it("accepts LAN names and .local", () => {
    assert.equal(isDirectTarget("pc2023"), true);
    assert.equal(isDirectTarget("pc2023:21118"), true);
    assert.equal(isDirectTarget("office-pc.local"), true);
    assert.equal(isDirectTarget("my-nas.local"), true);
  });
  it("does not treat digit-only RustDesk IDs as hostnames", () => {
    assert.equal(isHostnameTarget("123456789"), false);
    assert.equal(isDirectTarget("123456789"), false);
    assert.equal(isDirectTarget("123456789:21118"), false);
  });
  it("rejects IDs with @server and garbage", () => {
    assert.equal(isDirectTarget("abc@public"), false);
    assert.equal(isDirectTarget("under_score"), false);
    assert.equal(isDirectTarget("host:99999"), false);
  });
});

describe("normalizeDirectTarget", () => {
  it("appends the default direct port to a bare IP or hostname", () => {
    assert.equal(
      normalizeDirectTarget("192.168.1.50"),
      `192.168.1.50:${DIRECT_PORT}`
    );
    assert.equal(
      normalizeDirectTarget("192.168.1.50:21118"),
      "192.168.1.50:21118"
    );
    assert.equal(normalizeDirectTarget("xxx.yy.com"), `xxx.yy.com:${DIRECT_PORT}`);
    assert.equal(normalizeDirectTarget("pc2023"), `pc2023:${DIRECT_PORT}`);
    assert.equal(normalizeDirectTarget("xxx.yy.com:21118"), "xxx.yy.com:21118");
  });
  it("maps loopback names to 127.0.0.1", () => {
    assert.equal(normalizeDirectTarget("localhost"), `127.0.0.1:${DIRECT_PORT}`);
    assert.equal(
      normalizeDirectTarget("localhost:21118"),
      "127.0.0.1:21118"
    );
    assert.equal(normalizeDirectTarget("::1"), `127.0.0.1:${DIRECT_PORT}`);
    assert.equal(normalizeDirectTarget("[::1]"), `127.0.0.1:${DIRECT_PORT}`);
    assert.equal(
      normalizeDirectTarget("[::1]:21118"),
      "127.0.0.1:21118"
    );
    assert.equal(
      normalizeDirectTarget("127.0.0.1"),
      `127.0.0.1:${DIRECT_PORT}`
    );
  });
});

describe("resolveWebDirectRemoteId", () => {
  it("lets saved last_remote_id win over config and page host", () => {
    assert.equal(
      resolveWebDirectRemoteId({
        isWeb: true,
        direct: true,
        lastRemoteId: "10.0.0.9",
        defaultTarget: "192.168.1.50",
        locationHost: "192.168.1.10",
      }),
      "10.0.0.9"
    );
  });
  it("lets defaultTarget win over location host when last id is empty", () => {
    assert.equal(
      resolveWebDirectRemoteId({
        isWeb: true,
        direct: true,
        lastRemoteId: "",
        defaultTarget: "192.168.1.50",
        locationHost: "192.168.1.10",
      }),
      "192.168.1.50"
    );
  });
  it("prefills a LAN page host and maps localhost to 127.0.0.1", () => {
    assert.equal(
      resolveWebDirectRemoteId({
        isWeb: true,
        direct: true,
        lastRemoteId: "  ",
        defaultTarget: "",
        locationHost: "192.168.1.10",
      }),
      "192.168.1.10"
    );
    assert.equal(usefulDirectConnectHost("localhost"), "127.0.0.1");
    assert.equal(usefulDirectConnectHost("LOCALHOST"), "127.0.0.1");
    assert.equal(usefulDirectConnectHost("127.0.0.1"), "127.0.0.1");
    assert.equal(usefulDirectConnectHost("::1"), "127.0.0.1");
    assert.equal(usefulDirectConnectHost("[::1]"), "127.0.0.1");
  });
  it("does not prefill when not web, not direct, or hostname-only", () => {
    assert.equal(
      resolveWebDirectRemoteId({
        isWeb: false,
        direct: true,
        lastRemoteId: "",
        defaultTarget: "",
        locationHost: "192.168.1.10",
      }),
      ""
    );
    assert.equal(
      resolveWebDirectRemoteId({
        isWeb: true,
        direct: false,
        lastRemoteId: "",
        defaultTarget: "192.168.1.50",
        locationHost: "192.168.1.10",
      }),
      ""
    );
    assert.equal(usefulDirectConnectHost("my-nas.local"), null);
    assert.equal(usefulDirectConnectHost("localhost.example"), null);
  });
});
