import 'package:flutter/material.dart';

final Color _kFloatingFill = Colors.black.withOpacity(0.45);
final Color _kFloatingBorder = Colors.white.withOpacity(0.7);
final Color _kFloatingFieldFill = Colors.black.withOpacity(0.25);

/// Semi-transparent floating panel for mobile remote text input.
class InputRemoteTextFloatingPanel extends StatelessWidget {
  final TextEditingController controller;
  final String title;
  final String hint;
  final String cancelLabel;
  final String sendLabel;
  final String enterLabel;
  final Color accentColor;
  final VoidCallback onCancel;
  final VoidCallback onSend;
  final VoidCallback onEnter;

  const InputRemoteTextFloatingPanel({
    super.key,
    required this.controller,
    required this.title,
    required this.hint,
    required this.cancelLabel,
    required this.sendLabel,
    required this.enterLabel,
    required this.accentColor,
    required this.onCancel,
    required this.onSend,
    required this.onEnter,
  });

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Material(
        color: Colors.transparent,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 18),
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: _kFloatingFill,
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: _kFloatingBorder),
              ),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(12, 10, 12, 8),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Row(
                      children: [
                        Icon(Icons.text_fields,
                            color: _kFloatingBorder, size: 18),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            title,
                            style: const TextStyle(
                              color: Colors.white,
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 8),
                    TextField(
                      controller: controller,
                      autofocus: true,
                      minLines: 4,
                      maxLines: null,
                      keyboardType: TextInputType.multiline,
                      textInputAction: TextInputAction.newline,
                      cursorColor: accentColor,
                      style: const TextStyle(color: Colors.white, fontSize: 14),
                      decoration: InputDecoration(
                        isDense: true,
                        filled: true,
                        fillColor: _kFloatingFieldFill,
                        hintText: hint,
                        hintStyle: const TextStyle(color: Colors.white70),
                        contentPadding: const EdgeInsets.all(10),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(8),
                          borderSide: BorderSide(color: _kFloatingBorder),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(8),
                          borderSide: BorderSide(color: _kFloatingBorder),
                        ),
                        focusedBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(8),
                          borderSide: BorderSide(color: accentColor),
                        ),
                      ),
                    ),
                    const SizedBox(height: 8),
                    Row(
                      children: [
                        Flexible(
                          child: Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: [
                              _chromeButton(
                                label: cancelLabel,
                                onPressed: onCancel,
                                outlined: true,
                              ),
                              _chromeButton(
                                label: sendLabel,
                                onPressed: onSend,
                                outlined: true,
                              ),
                            ],
                          ),
                        ),
                        _chromeButton(
                          label: enterLabel,
                          onPressed: onEnter,
                          outlined: false,
                          icon: Icons.keyboard_return,
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _chromeButton({
    required String label,
    required VoidCallback onPressed,
    required bool outlined,
    IconData? icon,
  }) {
    final Color fg =
        outlined ? _kFloatingBorder : Colors.white;
    final child = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (icon != null) ...[
          Icon(icon, size: 14, color: fg),
          const SizedBox(width: 4),
        ],
        Text(
          label,
          style: TextStyle(
            color: fg,
            fontSize: 13,
            fontWeight: outlined ? FontWeight.w500 : FontWeight.w600,
          ),
        ),
      ],
    );
    return TextButton(
      style: TextButton.styleFrom(
        minimumSize: Size.zero,
        tapTargetSize: MaterialTapTargetSize.shrinkWrap,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
        backgroundColor: outlined ? Colors.transparent : accentColor,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
          side: BorderSide(color: outlined ? fg : accentColor),
        ),
      ),
      onPressed: onPressed,
      child: child,
    );
  }
}
