import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/input_remote_text_panel.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('floating panel keeps Send and Enter as distinct actions',
      (tester) async {
    tester.view.physicalSize = const Size(800, 1400);
    tester.view.devicePixelRatio = 1.0;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    var send = 0;
    var enter = 0;
    var cancel = 0;
    final controller = TextEditingController(text: 'hello');
    addTearDown(controller.dispose);

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        backgroundColor: const Color(0xFF4A6B8A),
        body: InputRemoteTextFloatingPanel(
          controller: controller,
          title: 'Input text',
          hint: 'Input text',
          cancelLabel: 'Cancel',
          sendLabel: 'Send',
          enterLabel: 'Enter',
          accentColor: const Color(0xFF0071FF),
          onCancel: () => cancel++,
          onSend: () => send++,
          onEnter: () => enter++,
        ),
      ),
    ));

    expect(find.text('Send'), findsOneWidget);
    expect(find.text('Enter'), findsOneWidget);
    expect(find.text('Cancel'), findsOneWidget);
    expect(find.byIcon(Icons.keyboard_return), findsOneWidget);

    await tester.tap(find.text('Send'));
    await tester.pump();
    expect(send, 1);
    expect(enter, 0);

    await tester.tap(find.text('Enter'));
    await tester.pump();
    expect(send, 1);
    expect(enter, 1);

    await tester.tap(find.text('Cancel'));
    await tester.pump();
    expect(cancel, 1);
    expect(send, 1);
    expect(enter, 1);
  });
}
