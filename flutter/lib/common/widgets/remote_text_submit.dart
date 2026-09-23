/// Same name as the mobile remote-page Enter keycap (`VK_RETURN` is the
/// soft-keyboard newline path).
const kRemoteTextEnterKeyName = 'VK_ENTER';

void submitRemoteTextInput({
  required String text,
  required bool sendEnter,
  required bool keyboardInputAllowed,
  required void Function(String value) inputString,
  required void Function(String name) inputKey,
}) {
  if (text.isNotEmpty) {
    inputString(text);
  }
  if (sendEnter && keyboardInputAllowed) {
    inputKey(kRemoteTextEnterKeyName);
  }
}
