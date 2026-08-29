import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  barCopy,
  controlBarEnabled,
  controlEnabled,
  esc,
  isForcedViewer,
} from "./control_room.ts";

describe("controlEnabled", () => {
  it("is off when config is missing or control is false", () => {
    globalThis.RUSTDESK_CONFIG = undefined;
    assert.equal(controlEnabled(), false);
    globalThis.RUSTDESK_CONFIG = { control: false };
    assert.equal(controlEnabled(), false);
    assert.equal(controlBarEnabled(), false);
  });
  it("follows control and controlBar", () => {
    globalThis.RUSTDESK_CONFIG = { control: true };
    assert.equal(controlEnabled(), true);
    assert.equal(controlBarEnabled(), true);
    globalThis.RUSTDESK_CONFIG = { control: true, controlBar: false };
    assert.equal(controlEnabled(), true);
    assert.equal(controlBarEnabled(), false);
  });
});

describe("isForcedViewer", () => {
  it("is empty until attach applies a viewer role", () => {
    const conn = { _id: "x", _closed: false, setViewOnly() {} };
    assert.equal(isForcedViewer(conn), false);
  });
});

describe("barCopy", () => {
  it("uses Chinese for zh and English otherwise", () => {
    assert.equal(barCopy("zh-CN").request, "申请控制");
    assert.equal(barCopy("en-US").request, "Request control");
    assert.equal(barCopy("zh-CN").approve, "批准");
    assert.equal(barCopy("en").autoNext, "Auto-approve next");
  });
});

describe("esc", () => {
  it("escapes HTML", () => {
    assert.equal(esc(`<img src="x">`), "&lt;img src=&quot;x&quot;&gt;");
  });
});
