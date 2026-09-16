import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  USB_HID_PAUSE,
  flutterHidToMapKey,
  flutterKeyModifiers,
  mapUsbHidToPeerCode,
} from "./keyboard_map.ts";

describe("mapUsbHidToPeerCode Windows", () => {
  it("maps letter A to scan code 0x1E (not HID 0x04 or ASCII 97)", () => {
    assert.equal(mapUsbHidToPeerCode(0x04, "Windows"), 0x1e);
    assert.deepEqual(flutterHidToMapKey(0x04, "Windows"), { kind: "map", chr: 0x1e });
  });
  it("maps Enter, Space, digits, and Shift to scan codes Winlogon accepts", () => {
    assert.equal(mapUsbHidToPeerCode(0x28, "Windows"), 0x1c);
    assert.equal(mapUsbHidToPeerCode(0x2c, "Windows"), 0x39);
    assert.equal(mapUsbHidToPeerCode(0x1e, "Windows"), 0x02);
    assert.equal(mapUsbHidToPeerCode(0xe1, "Windows"), 0x2a);
  });
  it("masks the usage page like Flutter usbHidUsage & 0xFFFF", () => {
    assert.equal(mapUsbHidToPeerCode(0x00070004, "Windows"), 0x1e);
  });
  it("sends Pause as Legacy, not a zero scan code", () => {
    assert.deepEqual(flutterHidToMapKey(USB_HID_PAUSE, "Windows"), { kind: "pause" });
    assert.equal(mapUsbHidToPeerCode(USB_HID_PAUSE, "Windows"), undefined);
  });
});

describe("mapUsbHidToPeerCode other platforms", () => {
  it("maps A to Xorg keycode 38 on Linux and kVK 0 on macOS", () => {
    assert.equal(mapUsbHidToPeerCode(0x04, "Linux"), 0x26);
    assert.equal(mapUsbHidToPeerCode(0x04, "Mac OS"), 0);
    assert.deepEqual(flutterHidToMapKey(0x04, "Mac OS"), { kind: "map", chr: 0 });
  });
});

describe("flutterKeyModifiers", () => {
  it("adds CapsLock for letters when Flutter lock bit 1 is set", () => {
    assert.deepEqual(flutterKeyModifiers(0x04, 1 << 1), ["CapsLock"]);
    assert.deepEqual(flutterKeyModifiers(0x28, 1 << 1), []);
  });
  it("adds NumLock for numpad when Flutter lock bit 2 is set", () => {
    assert.deepEqual(flutterKeyModifiers(0x59, 1 << 2), ["NumLock"]);
    assert.deepEqual(flutterKeyModifiers(0x04, 1 << 2), []);
  });
});
