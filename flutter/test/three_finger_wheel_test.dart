import 'package:flutter_hbb/utils/three_finger_wheel.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('ThreeFingerWheelAccumulator', () {
    test('vertical downward movement emits a positive wheel tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(0, 5), 1);
    });

    test('vertical upward movement emits a negative wheel tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(0, -5), -1);
    });

    test('horizontal right movement emits a positive wheel tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(5, 0), 1);
    });

    test('horizontal left movement emits a negative wheel tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(-5, 0), -1);
    });

    test('small movement does not emit a tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(0, 4), isNull);
      expect(acc.integral, 1.0);
    });

    test('accumulated small horizontal moves eventually emit a tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(2, 0), isNull);
      expect(acc.add(2, 0), isNull);
      expect(acc.add(2, 0), 1);
    });

    test('dominant horizontal axis is used when |dx| > |dy|', () {
      final acc = ThreeFingerWheelAccumulator();
      // dx=5 would tick; dy=-1 would not reverse it because dx dominates.
      expect(acc.add(5, -1), 1);
    });

    test('equal axes keep vertical behavior', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(5, 5), 1);
      acc.reset();
      expect(acc.add(-5, 5), 1);
    });

    test('vertical movement ignores smaller horizontal jitter', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(1, 5), 1);
    });

    test('accumulator resets after emitting a tick', () {
      final acc = ThreeFingerWheelAccumulator();
      expect(acc.add(0, 5), 1);
      expect(acc.add(0, 2), isNull);
    });

    test('repeated 1px horizontal moves emit a tick every five events', () {
      final acc = ThreeFingerWheelAccumulator();
      var ticks = 0;
      for (var i = 0; i < 20; i++) {
        if (acc.add(1.0, 0) != null) {
          ticks++;
        }
      }
      expect(ticks, 4);
    });
  });
}
