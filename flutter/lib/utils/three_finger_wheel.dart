/// Accumulates three-finger drag movement into mouse-wheel ticks.
///
/// Vertical and horizontal movement both drive the same vertical wheel so that
/// devices whose OS consumes three-finger vertical swipes can still scroll
/// with a three-finger horizontal swipe. Each update uses the dominant axis.
class ThreeFingerWheelAccumulator {
  /// Matches the original three-finger vertical wheel scale.
  static const double scale = 4.0;

  double _integral = 0;

  /// Adds a drag delta and returns a wheel tick when the accumulator crosses
  /// a step. Returns `1` (down), `-1` (up), or `null` if no tick yet.
  int? add(double dx, double dy) {
    final axisDelta = dx.abs() > dy.abs() ? dx : dy;
    _integral += axisDelta / scale;
    if (_integral > 1) {
      _integral = 0;
      return 1;
    }
    if (_integral < -1) {
      _integral = 0;
      return -1;
    }
    return null;
  }

  void reset() {
    _integral = 0;
  }

  double get integral => _integral;
}
