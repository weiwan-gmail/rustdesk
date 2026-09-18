import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/input_model.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/control_pad_wheel.dart';

const double _kCellW = 74;
const double _kCellH = 62;
const double _kHandleH = 28;
const double _kPadW = _kCellW * 3;
const double _kPadH = _kHandleH + _kCellH * 3;
const double _kEdge = 12;
const int _kWheelIntervalMs = 100;
const int _kLongPressMs = 400;
const double _kMoveSlop = 8;
const double _kDragMoveScale = 2.0;
const double _kZoomPerPixel = 0.008;
final Color _kFill = Colors.black.withOpacity(0.45);
final Color _kBorder = Colors.white.withOpacity(0.7);
final Color _kActive = Colors.blue.withOpacity(0.7);

class FloatingControlPad extends StatefulWidget {
  final FFI ffi;
  final VoidCallback onOpenKeyboard;

  const FloatingControlPad({
    super.key,
    required this.ffi,
    required this.onOpenKeyboard,
  });

  @override
  State<FloatingControlPad> createState() => _FloatingControlPadState();
}

class _FloatingControlPadState extends State<FloatingControlPad> {
  Offset _position = Offset.zero;
  bool _isInitialized = false;
  bool _collapsed = false;
  Rect? _lastBlockedRect;
  Orientation? _previousOrientation;
  Offset _preSavedPos = Offset.zero;
  late final ControlPadMode _mode;

  InputModel get _inputModel => widget.ffi.inputModel;
  CursorModel get _cursorModel => widget.ffi.cursorModel;
  CanvasModel get _canvasModel => widget.ffi.canvasModel;

  @override
  void initState() {
    super.initState();
    _mode = widget.ffi.ffiModel.controlPadMode;
    _mode.addListener(_onModeChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      _previousOrientation = MediaQuery.of(context).orientation;
      _resetPosition(_previousOrientation!);
    });
  }

  void _onModeChanged() {
    if (!_mode.show) {
      _cursorModel.blockEvents = false;
      if (_lastBlockedRect != null) {
        _cursorModel.removeBlockedRect(_lastBlockedRect!);
        _lastBlockedRect = null;
      }
    }
    if (mounted) setState(() {});
    if (_mode.show) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _updateBlockedRect();
      });
    }
  }

  @override
  void dispose() {
    _mode.removeListener(_onModeChanged);
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    _cursorModel.blockEvents = false;
    _trySavePosition();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final ori = MediaQuery.of(context).orientation;
    if (_previousOrientation == null || _previousOrientation != ori) {
      _resetPosition(ori);
    }
    _previousOrientation = ori;
  }

  String _positionKey(Orientation ori) {
    final strOri = ori == Orientation.landscape ? 'l' : 'p';
    return 'control-pad-$strOri-pos';
  }

  static Offset? _loadPositionFromString(String s) {
    if (s.isEmpty) return null;
    try {
      final m = jsonDecode(s);
      return Offset(m['x'], m['y']);
    } catch (e) {
      return null;
    }
  }

  void _trySavePosition() {
    if (_previousOrientation == null) return;
    if ((_position - _preSavedPos).distanceSquared < 0.1) return;
    bind.setLocalFlutterOption(
        k: _positionKey(_previousOrientation!),
        v: jsonEncode({'x': _position.dx, 'y': _position.dy}));
    _preSavedPos = _position;
  }

  Offset _defaultPosition(Size size) {
    return Offset(_kEdge, size.height - _kPadH - _kEdge - 56);
  }

  void _resetPosition(Orientation ori) {
    final size = MediaQuery.of(context).size;
    final pos = _loadPositionFromString(
            bind.getLocalFlutterOption(k: _positionKey(ori))) ??
        _defaultPosition(size);
    setState(() {
      _position = _clamp(pos, size);
      _isInitialized = true;
    });
    _preSavedPos = _position;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  Offset _clamp(Offset pos, Size size) {
    final w = _collapsed ? _kHandleH + 8 : _kPadW;
    final h = _collapsed ? _kHandleH + 8 : _kPadH;
    return Offset(
      pos.dx.clamp(_kEdge, (size.width - w - _kEdge).clamp(_kEdge, size.width)),
      pos.dy
          .clamp(_kEdge, (size.height - h - _kEdge).clamp(_kEdge, size.height)),
    );
  }

  void _updateBlockedRect() {
    if (_lastBlockedRect != null) {
      _cursorModel.removeBlockedRect(_lastBlockedRect!);
    }
    if (!_mode.show) {
      _lastBlockedRect = null;
      return;
    }
    final size = _collapsed
        ? const Size(_kHandleH + 8, _kHandleH + 8)
        : const Size(_kPadW, _kPadH);
    final newRect =
        Rect.fromLTWH(_position.dx, _position.dy, size.width, size.height);
    _cursorModel.addBlockedRect(newRect);
    _lastBlockedRect = newRect;
  }

  void _onHandleMove(Offset delta) {
    final size = MediaQuery.of(context).size;
    final next = _clamp(_position + delta, size);
    setState(() {
      _position = next;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  @override
  Widget build(BuildContext context) {
    if (!_mode.show || !_isInitialized) {
      return const Offstage();
    }
    return Positioned(
      left: _position.dx,
      top: _position.dy,
      child: Listener(
        onPointerDown: (_) => _cursorModel.blockEvents = true,
        onPointerUp: (_) {
          _cursorModel.blockEvents = false;
          _trySavePosition();
        },
        onPointerCancel: (_) => _cursorModel.blockEvents = false,
        child: _collapsed ? _buildCollapsed() : _buildPad(),
      ),
    );
  }

  void _setCollapsed(bool collapsed) {
    setState(() {
      _collapsed = collapsed;
      final size = MediaQuery.of(context).size;
      _position = _clamp(_position, size);
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _updateBlockedRect();
    });
  }

  Widget _buildCollapsed() {
    return GestureDetector(
      onPanUpdate: (d) => _onHandleMove(d.delta),
      onTap: () => _setCollapsed(false),
      child: Container(
        width: _kHandleH + 8,
        height: _kHandleH + 8,
        decoration: BoxDecoration(
          color: _kFill,
          border: Border.all(color: _kBorder),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Icon(Icons.grid_view, color: _kBorder, size: 20),
      ),
    );
  }

  Widget _buildPad() {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _buildHandle(),
        SizedBox(
          width: _kPadW,
          height: _kCellH * 3,
          child: Column(
            children: [
              Row(children: [
                _TapCell(
                  icon: Icons.touch_app,
                  label: translate('Left click'),
                  onTap: () => _inputModel.tap(MouseButtons.left),
                ),
                _WheelCell(inputModel: _inputModel),
                _TapCell(
                  icon: Icons.mouse,
                  label: translate('Right click'),
                  onTap: () => _inputModel.tap(MouseButtons.right),
                ),
              ]),
              Row(children: [
                _DragCell(
                  inputModel: _inputModel,
                  cursorModel: _cursorModel,
                ),
                _TapCell(
                  icon: Icons.radio_button_checked,
                  label: translate('Middle click'),
                  onTap: () => _inputModel.tap(MouseButtons.wheel),
                ),
                _TapCell(
                  icon: Icons.looks_two,
                  label: translate('Double-click'),
                  onTap: () async {
                    await _inputModel.tap(MouseButtons.left);
                    await _inputModel.tap(MouseButtons.left);
                  },
                ),
              ]),
              Row(children: [
                _CanvasPanCell(canvasModel: _canvasModel),
                _CanvasZoomCell(canvasModel: _canvasModel),
                _TapCell(
                  icon: Icons.keyboard,
                  label: translate('Keyboard'),
                  onTap: widget.onOpenKeyboard,
                ),
              ]),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildHandle() {
    return GestureDetector(
      onPanUpdate: (d) => _onHandleMove(d.delta),
      child: Container(
        width: _kPadW,
        height: _kHandleH,
        decoration: BoxDecoration(
          color: _kFill,
          border: Border.all(color: _kBorder),
          borderRadius: const BorderRadius.vertical(top: Radius.circular(8)),
        ),
        child: Row(
          children: [
            const SizedBox(width: 8),
            Icon(Icons.drag_handle, color: _kBorder, size: 18),
            const Spacer(),
            InkWell(
              onTap: () => _setCollapsed(true),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Icon(Icons.expand_more, color: _kBorder, size: 20),
              ),
            ),
            InkWell(
              onTap: () => _mode.toggle(),
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8),
                child: Icon(Icons.close, color: _kBorder, size: 18),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CellFrame extends StatelessWidget {
  final Widget child;
  final bool active;

  const _CellFrame({required this.child, this.active = false});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: _kCellW,
      height: _kCellH,
      decoration: BoxDecoration(
        color: _kFill,
        border: Border.all(color: active ? _kActive : _kBorder, width: 1),
      ),
      child: child,
    );
  }
}

class _TapCell extends StatefulWidget {
  final IconData icon;
  final String label;
  final FutureOr<void> Function() onTap;

  const _TapCell({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  @override
  State<_TapCell> createState() => _TapCellState();
}

class _TapCellState extends State<_TapCell> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTapDown: (_) => setState(() => _down = true),
      onTapUp: (_) => setState(() => _down = false),
      onTapCancel: () => setState(() => _down = false),
      onTap: () => widget.onTap(),
      child: _CellFrame(
        active: _down,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(widget.icon, color: _kBorder, size: 20),
            const SizedBox(height: 2),
            Text(
              widget.label,
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: _kBorder, fontSize: 9, height: 1.1),
            ),
          ],
        ),
      ),
    );
  }
}

class _WheelCell extends StatefulWidget {
  final InputModel inputModel;
  const _WheelCell({required this.inputModel});

  @override
  State<_WheelCell> createState() => _WheelCellState();
}

class _WheelCellState extends State<_WheelCell> {
  final _acc = ControlPadWheelAccumulator();
  Timer? _longPressTimer;
  Timer? _repeatTimer;
  Offset _downLocal = Offset.zero;
  bool _dragging = false;
  bool _holding = false;
  bool _down = false;

  void _cancel() {
    _longPressTimer?.cancel();
    _longPressTimer = null;
    _repeatTimer?.cancel();
    _repeatTimer = null;
    _acc.reset();
    _dragging = false;
    _holding = false;
    if (mounted) setState(() => _down = false);
  }

  void _startHold(int y) {
    _holding = true;
    widget.inputModel.scroll(y);
    _repeatTimer?.cancel();
    _repeatTimer =
        Timer.periodic(const Duration(milliseconds: _kWheelIntervalMs), (_) {
      widget.inputModel.scroll(y);
    });
  }

  @override
  void dispose() {
    _cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (e) {
        _cancel();
        _downLocal = e.localPosition;
        setState(() => _down = true);
        _longPressTimer =
            Timer(const Duration(milliseconds: _kLongPressMs), () {
          if (!_dragging && mounted) {
            _startHold(controlPadWheelHoldY(_downLocal.dy, _kCellH));
          }
        });
      },
      onPointerMove: (e) {
        if (_holding) return;
        if ((e.localPosition - _downLocal).distance > _kMoveSlop) {
          _dragging = true;
          _longPressTimer?.cancel();
          _longPressTimer = null;
          final tick = _acc.add(e.delta.dx, e.delta.dy);
          if (tick != null) {
            widget.inputModel.scrollXY(tick.x, tick.y);
          }
        }
      },
      onPointerUp: (_) => _cancel(),
      onPointerCancel: (_) => _cancel(),
      child: _CellFrame(
        active: _down,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.swap_vert, color: _kBorder, size: 20),
            const SizedBox(height: 2),
            Text(
              translate('Simulated wheel'),
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: _kBorder, fontSize: 9, height: 1.1),
            ),
          ],
        ),
      ),
    );
  }
}

class _DragCell extends StatefulWidget {
  final InputModel inputModel;
  final CursorModel cursorModel;
  const _DragCell({required this.inputModel, required this.cursorModel});

  @override
  State<_DragCell> createState() => _DragCellState();
}

class _DragCellState extends State<_DragCell> {
  bool _held = false;
  Future<void>? _downFuture;

  Future<void> _release() async {
    if (!_held) return;
    _held = false;
    final down = _downFuture;
    _downFuture = null;
    if (down != null) {
      await down;
    }
    await widget.inputModel.tapUp(MouseButtons.left);
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    if (_held) {
      _held = false;
      final down = _downFuture;
      _downFuture = null;
      if (down != null) {
        down.then((_) => widget.inputModel.tapUp(MouseButtons.left));
      } else {
        widget.inputModel.tapUp(MouseButtons.left);
      }
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) {
        if (_held) return;
        _held = true;
        setState(() {});
        _downFuture = widget.inputModel.tapDown(MouseButtons.left);
      },
      onPointerMove: (e) {
        if (!_held) return;
        final delta = e.delta * _kDragMoveScale;
        if (widget.inputModel.relativeMouseMode.value) {
          widget.inputModel
              .sendMobileRelativeMouseMove(delta.dx, delta.dy);
        } else {
          widget.cursorModel.updatePan(delta, Offset.zero, false);
        }
      },
      onPointerUp: (_) => _release(),
      onPointerCancel: (_) => _release(),
      child: _CellFrame(
        active: _held,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.open_with, color: _kBorder, size: 20),
            const SizedBox(height: 2),
            Text(
              translate('Drag'),
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: _kBorder, fontSize: 9, height: 1.1),
            ),
          ],
        ),
      ),
    );
  }
}

class _CanvasPanCell extends StatefulWidget {
  final CanvasModel canvasModel;
  const _CanvasPanCell({required this.canvasModel});

  @override
  State<_CanvasPanCell> createState() => _CanvasPanCellState();
}

class _CanvasPanCellState extends State<_CanvasPanCell> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) => setState(() => _down = true),
      onPointerMove: (e) {
        if (widget.canvasModel.locked) return;
        widget.canvasModel.panX(e.delta.dx);
        widget.canvasModel.panY(e.delta.dy);
      },
      onPointerUp: (_) => setState(() => _down = false),
      onPointerCancel: (_) => setState(() => _down = false),
      child: _CellFrame(
        active: _down,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.pan_tool, color: _kBorder, size: 20),
            const SizedBox(height: 2),
            Text(
              translate('Canvas pan'),
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: _kBorder, fontSize: 9, height: 1.1),
            ),
          ],
        ),
      ),
    );
  }
}

class _CanvasZoomCell extends StatefulWidget {
  final CanvasModel canvasModel;
  const _CanvasZoomCell({required this.canvasModel});

  @override
  State<_CanvasZoomCell> createState() => _CanvasZoomCellState();
}

class _CanvasZoomCellState extends State<_CanvasZoomCell> {
  bool _down = false;

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) => setState(() => _down = true),
      onPointerMove: (e) {
        if (widget.canvasModel.locked) return;
        final v = 1.0 - e.delta.dy * _kZoomPerPixel;
        if (v == 1.0) return;
        final size = MediaQuery.of(context).size;
        widget.canvasModel.updateScale(v, Offset(size.width / 2, size.height / 2));
      },
      onPointerUp: (_) => setState(() => _down = false),
      onPointerCancel: (_) => setState(() => _down = false),
      child: _CellFrame(
        active: _down,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.zoom_in, color: _kBorder, size: 20),
            const SizedBox(height: 2),
            Text(
              translate('Canvas zoom'),
              textAlign: TextAlign.center,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(color: _kBorder, fontSize: 9, height: 1.1),
            ),
          ],
        ),
      ),
    );
  }
}
