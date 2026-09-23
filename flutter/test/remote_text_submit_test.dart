import 'package:flutter_hbb/common/widgets/remote_text_submit.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('submitRemoteTextInput', () {
    test('Send injects text and does not send Enter', () {
      final strings = <String>[];
      final keys = <String>[];

      submitRemoteTextInput(
        text: 'hello',
        sendEnter: false,
        keyboardInputAllowed: true,
        inputString: strings.add,
        inputKey: keys.add,
      );

      expect(strings, ['hello']);
      expect(keys, isEmpty);
    });

    test('Enter injects text then VK_ENTER', () {
      final events = <String>[];

      submitRemoteTextInput(
        text: 'hello',
        sendEnter: true,
        keyboardInputAllowed: true,
        inputString: (value) => events.add('string:$value'),
        inputKey: (name) => events.add('key:$name'),
      );

      expect(events, ['string:hello', 'key:VK_ENTER']);
    });

    test('empty text + Enter sends only VK_ENTER', () {
      final strings = <String>[];
      final keys = <String>[];

      submitRemoteTextInput(
        text: '',
        sendEnter: true,
        keyboardInputAllowed: true,
        inputString: strings.add,
        inputKey: keys.add,
      );

      expect(strings, isEmpty);
      expect(keys, ['VK_ENTER']);
    });

    test('Send with empty text is a no-op', () {
      var stringCalls = 0;
      var keyCalls = 0;

      submitRemoteTextInput(
        text: '',
        sendEnter: false,
        keyboardInputAllowed: true,
        inputString: (_) => stringCalls++,
        inputKey: (_) => keyCalls++,
      );

      expect(stringCalls, 0);
      expect(keyCalls, 0);
    });

    test('Enter skips VK_ENTER when keyboard input is not allowed', () {
      final strings = <String>[];
      final keys = <String>[];

      submitRemoteTextInput(
        text: 'hello',
        sendEnter: true,
        keyboardInputAllowed: false,
        inputString: strings.add,
        inputKey: keys.add,
      );

      expect(strings, ['hello']);
      expect(keys, isEmpty);
    });

    test('uses the remote-page Enter key name', () {
      expect(kRemoteTextEnterKeyName, 'VK_ENTER');
    });
  });
}
