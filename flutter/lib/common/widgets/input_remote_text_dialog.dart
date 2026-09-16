import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/platform_model.dart';

/// Local multiline dialog: edit on the client, then inject text on the remote
/// via [bind.sessionInputString] after the user confirms.
void showInputRemoteTextDialog({
  required SessionID sessionId,
  required OverlayDialogManager dialogManager,
}) {
  final controller = TextEditingController();
  dialogManager.show((setState, close, context) {
    submit() {
      final text = controller.text;
      if (text.isNotEmpty) {
        bind.sessionInputString(sessionId: sessionId, value: text);
      }
      close();
    }

    return CustomAlertDialog(
      title: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.text_fields, color: MyTheme.accent),
          Text(translate('Input text')).paddingOnly(left: 10),
        ],
      ),
      content: TextField(
        controller: controller,
        autofocus: true,
        minLines: 6,
        maxLines: null,
        keyboardType: TextInputType.multiline,
        textInputAction: TextInputAction.newline,
        decoration: InputDecoration(
          border: const OutlineInputBorder(),
          hintText: translate('Input text'),
        ),
      ).workaroundFreezeLinuxMint(),
      actions: [
        dialogButton(
          "Cancel",
          icon: Icon(Icons.close_rounded),
          onPressed: close,
          isOutline: true,
        ),
        dialogButton(
          "OK",
          icon: Icon(Icons.done_rounded),
          onPressed: submit,
        ),
      ],
      onCancel: close,
    );
  });
}
