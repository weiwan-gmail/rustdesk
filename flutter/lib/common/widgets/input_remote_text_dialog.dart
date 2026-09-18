import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/input_remote_text_panel.dart';
import 'package:flutter_hbb/common/widgets/remote_text_submit.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:get/get.dart';

/// Local multiline dialog: edit on the client, then inject text on the remote
/// via [bind.sessionInputString] after the user confirms.
void showInputRemoteTextDialog({
  required SessionID sessionId,
  required OverlayDialogManager dialogManager,
  required InputModel inputModel,
}) {
  final controller = TextEditingController();
  dialogManager.show((setState, close, context) {
    void submit({required bool sendEnter}) {
      submitRemoteTextInput(
        text: controller.text,
        sendEnter: sendEnter,
        keyboardInputAllowed: inputModel.keyboardInputAllowed,
        inputString: (value) =>
            bind.sessionInputString(sessionId: sessionId, value: value),
        inputKey: (name) => inputModel.inputKey(name),
      );
      close();
    }

    if (isMobile) {
      return InputRemoteTextFloatingPanel(
        controller: controller,
        title: translate('Input text'),
        hint: translate('Input text'),
        cancelLabel: translate('Cancel'),
        sendLabel: translate('Send'),
        enterLabel: translate('Enter'),
        accentColor: MyTheme.accent,
        onCancel: close,
        onSend: () => submit(sendEnter: false),
        onEnter: () => submit(sendEnter: true),
      );
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
          "Send",
          icon: Icon(Icons.send_rounded),
          onPressed: () => submit(sendEnter: false),
          isOutline: true,
        ),
        dialogButton(
          "Enter",
          icon: Icon(Icons.keyboard_return),
          onPressed: () => submit(sendEnter: true),
        ),
      ],
      onCancel: close,
    );
  });
}
