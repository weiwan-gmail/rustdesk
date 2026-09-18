import 'package:flutter_hbb/utils/control_pad_wheel.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('ControlPadWheelAccumulator', () {
    test('vertical downward drag emits a down tick (y = -1)', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(0, 5);
      expect(tick, isNotNull);
      expect(tick!.x, 0);
      expect(tick.y, -1);
    });

    test('vertical upward drag emits an up tick (y = 1)', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(0, -5);
      expect(tick, isNotNull);
      expect(tick!.x, 0);
      expect(tick.y, 1);
    });

    test('horizontal right drag emits x = 1', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(5, 0);
      expect(tick, isNotNull);
      expect(tick!.x, 1);
      expect(tick.y, 0);
    });

    test('horizontal left drag emits x = -1', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(-5, 0);
      expect(tick, isNotNull);
      expect(tick!.x, -1);
      expect(tick.y, 0);
    });

    test('small movement does not emit a tick', () {
      final acc = ControlPadWheelAccumulator();
      expect(acc.add(0, 4), isNull);
      expect(acc.integral, 1.0);
    });

    test('accumulated small vertical moves eventually emit a tick', () {
      final acc = ControlPadWheelAccumulator();
      expect(acc.add(0, 2), isNull);
      expect(acc.add(0, 2), isNull);
      final tick = acc.add(0, 2);
      expect(tick, isNotNull);
      expect(tick!.y, -1);
    });

    test('dominant horizontal axis is used when |dx| > |dy|', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(5, -1);
      expect(tick, isNotNull);
      expect(tick!.x, 1);
      expect(tick.y, 0);
    });

    test('equal axes keep vertical behavior', () {
      final acc = ControlPadWheelAccumulator();
      final tick = acc.add(5, 5);
      expect(tick, isNotNull);
      expect(tick!.x, 0);
      expect(tick.y, -1);
    });

    test('axis switch resets the accumulator', () {
      final acc = ControlPadWheelAccumulator();
      expect(acc.add(0, 4), isNull);
      expect(acc.add(5, 0), isNotNull);
    });

    test('accumulator resets after emitting a tick', () {
      final acc = ControlPadWheelAccumulator();
      expect(acc.add(0, 5), isNotNull);
      expect(acc.add(0, 2), isNull);
    });
  });

  group('controlPadWheelHoldY', () {
    test('upper half is scroll up', () {
      expect(controlPadWheelHoldY(0, 100), 1);
      expect(controlPadWheelHoldY(49, 100), 1);
    });

    test('lower half is scroll down', () {
      expect(controlPadWheelHoldY(50, 100), -1);
      expect(controlPadWheelHoldY(99, 100), -1);
    });
  });
}
