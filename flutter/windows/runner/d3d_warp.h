#ifndef RUNNER_D3D_WARP_H_
#define RUNNER_D3D_WARP_H_

// When RUSTDESK_FLUTTER_D3D_WARP=1, force Flutter's bundled ANGLE onto
// D3D11 WARP (and the COPY present path) before the engine is created.
void MaybeForceFlutterAngleWarp();

#endif  // RUNNER_D3D_WARP_H_
