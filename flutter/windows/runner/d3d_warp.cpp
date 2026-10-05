#include "d3d_warp.h"

#include <windows.h>

#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <vector>

namespace {

// Must match src/platform/virtual_gpu.rs ENV_FLUTTER_D3D_WARP.
constexpr const char kEnvFlutterD3dWarp[] = "RUSTDESK_FLUTTER_D3D_WARP";

// EGL / ANGLE attribute tokens used by Flutter 3.24.5 egl::Manager.
constexpr int kEglNone = 0x3038;
constexpr int kEglPlatformAngleDeviceType = 0x3209;
constexpr int kEglPlatformAngleDeviceTypeD3dWarp = 0x320B;
constexpr int kEglExperimentalPresentPath = 0x33A4;
constexpr int kEglExperimentalPresentPathFast = 0x33A9;
constexpr int kEglExperimentalPresentPathCopy = 0x33AA;

using EglGetPlatformDisplayEXTFn = void* (*)(unsigned int platform,
                                             void* native_display,
                                             const int* attrib_list);

EglGetPlatformDisplayEXTFn g_original_get_platform_display_ext = nullptr;

#if defined(_M_X64)
constexpr size_t kJumpSize = 14;
#elif defined(_M_ARM64)
constexpr size_t kJumpSize = 16;
#else
constexpr size_t kJumpSize = 0;
#endif

void* g_trampoline = nullptr;
DWORD g_old_protect = 0;

bool EnvForced() {
  char* value = nullptr;
  size_t size = 0;
  if (_dupenv_s(&value, &size, kEnvFlutterD3dWarp) != 0 || value == nullptr) {
    return false;
  }
  const bool on = strcmp(value, "1") == 0;
  free(value);
  return on;
}

std::vector<int> RewriteAttribs(const int* attrib_list) {
  std::vector<int> out;
  bool has_device_type = false;
  if (attrib_list) {
    for (size_t i = 0; attrib_list[i] != kEglNone; i += 2) {
      const int key = attrib_list[i];
      int val = attrib_list[i + 1];
      if (key == kEglPlatformAngleDeviceType) {
        has_device_type = true;
        val = kEglPlatformAngleDeviceTypeD3dWarp;
      } else if (key == kEglExperimentalPresentPath &&
                 val == kEglExperimentalPresentPathFast) {
        // FAST present talks to a D3D swapchain directly; COPY is the safe
        // path on virtual adapters / no physical monitor.
        val = kEglExperimentalPresentPathCopy;
      }
      out.push_back(key);
      out.push_back(val);
    }
  }
  if (!has_device_type) {
    out.push_back(kEglPlatformAngleDeviceType);
    out.push_back(kEglPlatformAngleDeviceTypeD3dWarp);
  }
  out.push_back(kEglNone);
  return out;
}

void* HookedGetPlatformDisplayEXT(unsigned int platform,
                                  void* native_display,
                                  const int* attrib_list) {
  const std::vector<int> attribs = RewriteAttribs(attrib_list);
  return g_original_get_platform_display_ext(platform, native_display,
                                             attribs.data());
}

void* FollowThunks(void* p, int depth = 0) {
  if (!p || depth > 4) {
    return p;
  }
#if defined(_M_X64)
  auto* b = static_cast<uint8_t*>(p);
  // jmp rel32
  if (b[0] == 0xE9) {
    int32_t rel = 0;
    memcpy(&rel, b + 1, 4);
    return FollowThunks(b + 5 + rel, depth + 1);
  }
  // jmp qword ptr [rip+disp32]
  if (b[0] == 0xFF && b[1] == 0x25) {
    int32_t disp = 0;
    memcpy(&disp, b + 2, 4);
    void* next = *reinterpret_cast<void**>(b + 6 + disp);
    return FollowThunks(next, depth + 1);
  }
#endif
  return p;
}

void WriteAbsJump(void* from, void* to) {
  auto* p = static_cast<uint8_t*>(from);
#if defined(_M_X64)
  // jmp qword ptr [rip+0]; dq dest
  p[0] = 0xFF;
  p[1] = 0x25;
  p[2] = 0x00;
  p[3] = 0x00;
  p[4] = 0x00;
  p[5] = 0x00;
  memcpy(p + 6, &to, sizeof(to));
#elif defined(_M_ARM64)
  // ldr x16, #8; br x16; .quad dest
  const uint32_t ldr = 0x58000050u;
  const uint32_t br = 0xD61F0200u;
  memcpy(p, &ldr, 4);
  memcpy(p + 4, &br, 4);
  memcpy(p + 8, &to, sizeof(to));
#else
  (void)p;
  (void)to;
#endif
}

bool InstallHook(void* target) {
  if (kJumpSize == 0 || !target) {
    return false;
  }
  g_trampoline = VirtualAlloc(nullptr, 64, MEM_COMMIT | MEM_RESERVE,
                              PAGE_EXECUTE_READWRITE);
  if (!g_trampoline) {
    return false;
  }
  memcpy(g_trampoline, target, kJumpSize);
  WriteAbsJump(static_cast<uint8_t*>(g_trampoline) + kJumpSize,
               static_cast<uint8_t*>(target) + kJumpSize);
  if (!VirtualProtect(target, kJumpSize, PAGE_EXECUTE_READWRITE,
                      &g_old_protect)) {
    VirtualFree(g_trampoline, 0, MEM_RELEASE);
    g_trampoline = nullptr;
    return false;
  }
  WriteAbsJump(target, reinterpret_cast<void*>(&HookedGetPlatformDisplayEXT));
  FlushInstructionCache(GetCurrentProcess(), target, kJumpSize);
  DWORD restored = 0;
  VirtualProtect(target, kJumpSize, g_old_protect, &restored);
  g_original_get_platform_display_ext =
      reinterpret_cast<EglGetPlatformDisplayEXTFn>(g_trampoline);
  return true;
}

}  // namespace

void MaybeForceFlutterAngleWarp() {
  if (!EnvForced()) {
    return;
  }
#if !defined(_M_X64) && !defined(_M_ARM64)
  OutputDebugStringA(
      "rustdesk: RUSTDESK_FLUTTER_D3D_WARP is set but this arch cannot hook "
      "eglGetPlatformDisplayEXT.\n");
  return;
#endif
  HMODULE egl = LoadLibraryW(L"libEGL.dll");
  if (!egl) {
    OutputDebugStringA(
        "rustdesk: RUSTDESK_FLUTTER_D3D_WARP=1 but libEGL.dll was not found.\n");
    return;
  }
  void* fn = FollowThunks(reinterpret_cast<void*>(
      GetProcAddress(egl, "eglGetPlatformDisplayEXT")));
  if (!fn) {
    OutputDebugStringA(
        "rustdesk: libEGL.dll has no eglGetPlatformDisplayEXT; WARP hook "
        "skipped.\n");
    return;
  }
  if (!InstallHook(fn)) {
    OutputDebugStringA(
        "rustdesk: failed to install ANGLE WARP hook on "
        "eglGetPlatformDisplayEXT.\n");
    return;
  }
  OutputDebugStringA(
      "rustdesk: forcing Flutter ANGLE onto D3D11 WARP "
      "(eglGetPlatformDisplayEXT hook).\n");
}
