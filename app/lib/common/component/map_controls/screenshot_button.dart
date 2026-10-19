import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/theme/app_colors.dart';

/// Uses the same glass circle as Record controls, with a quiet idle state.
class MapScreenshotButton extends StatefulWidget {
  const MapScreenshotButton({super.key, required this.onPressed});

  final VoidCallback? onPressed;

  @override
  State<MapScreenshotButton> createState() => _MapScreenshotButtonState();
}

class _MapScreenshotButtonState extends State<MapScreenshotButton> {
  static const _idleOpacity = 0.22;
  static const _revealHoldDuration = Duration(seconds: 2);

  final _pointers = <int>{};
  Timer? _fadeTimer;
  bool _revealed = false;
  bool _armed = false;
  bool _hovered = false;
  bool _focused = false;

  void _reveal() {
    _fadeTimer?.cancel();
    if (!_revealed) setState(() => _revealed = true);
  }

  void _scheduleFade() {
    _fadeTimer?.cancel();
    if (_pointers.isNotEmpty || _hovered || _focused) return;
    _fadeTimer = Timer(_revealHoldDuration, () {
      if (mounted) {
        setState(() {
          _revealed = false;
          _armed = false;
        });
      }
    });
  }

  void _handleTap() {
    // Contact/hover may reveal the button, but only a completed first tap
    // arms it. Dragging across it or cancelling a gesture cannot arm it.
    final shouldCapture = _armed;
    _armed = !shouldCapture;
    _reveal();
    _scheduleFade();
    if (shouldCapture) widget.onPressed?.call();
  }

  void _releasePointer(int pointer) {
    _pointers.remove(pointer);
    _scheduleFade();
  }

  @override
  void dispose() {
    _fadeTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final color = widget.onPressed == null
        ? context.appColors.mutedInkColor
        : context.appColors.deepGreen;

    return Listener(
      behavior: HitTestBehavior.opaque,
      // Reveal on contact, without waiting for the tap recognizer to win.
      onPointerDown: (event) {
        _pointers.add(event.pointer);
        _reveal();
      },
      onPointerUp: (event) => _releasePointer(event.pointer),
      onPointerCancel: (event) => _releasePointer(event.pointer),
      child: AnimatedOpacity(
        opacity: _revealed ? 1 : _idleOpacity,
        duration: Duration(milliseconds: _revealed ? 100 : 350),
        curve: Curves.easeOut,
        child: LiquidGlassSurface(
          circular: true,
          child: Tooltip(
            message: context.tr('screenshot.button_label'),
            child: Material(
              color: Colors.transparent,
              child: InkWell(
                onTap: widget.onPressed == null ? null : _handleTap,
                customBorder: const CircleBorder(),
                onHover: (hovered) {
                  _hovered = hovered;
                  hovered ? _reveal() : _scheduleFade();
                },
                onFocusChange: (focused) {
                  _focused = focused;
                  focused ? _reveal() : _scheduleFade();
                },
                child: SizedBox.square(
                  dimension: 44,
                  child: Icon(Icons.crop_free_rounded, size: 20, color: color),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
