/// Accumulates control-pad wheel-cell drag into mouse-wheel ticks.
///
/// Vertical drag drives the vertical wheel; horizontal drag drives the
/// horizontal wheel. The dominant axis of each update is used. Sign matches
/// [FloatingWheel]: drag up / long-press upper half → y = 1 (scroll up).
class ControlPadWheelTick {
  final int x;
  final int y;
  const ControlPadWheelTick({this.x = 0, this.y = 0});
}

class ControlPadWheelAccumulator {
  /// Matches [ThreeFingerWheelAccumulator.scale].
  static const double scale = 4.0;

  double _integral = 0;
  bool? _horizontal;

  ControlPadWheelTick? add(double dx, double dy) {
    final horizontal = dx.abs() > dy.abs();
    if (_horizontal != null && _horizontal != horizontal) {
      _integral = 0;
    }
    _horizontal = horizontal;
    final delta = horizontal ? dx : dy;
    _integral += delta / scale;
    if (_integral > 1) {
      _integral = 0;
      return horizontal
          ? const ControlPadWheelTick(x: 1)
          : const ControlPadWheelTick(y: -1);
    }
    if (_integral < -1) {
      _integral = 0;
      return horizontal
          ? const ControlPadWheelTick(x: -1)
          : const ControlPadWheelTick(y: 1);
    }
    return null;
  }

  void reset() {
    _integral = 0;
    _horizontal = null;
  }

  double get integral => _integral;
}

/// Long-press in the upper half of the wheel cell scrolls up (`y = 1`).
int controlPadWheelHoldY(double localY, double height) {
  return localY < height / 2 ? 1 : -1;
}
