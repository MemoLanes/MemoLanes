import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';

/// The shared compact checkbox used throughout the app.
///
/// Styling lives with the component while UI v2 remains dark-only. The final
/// theme PR can move these roles into the app-level [CheckboxThemeData].
class AppCheckbox extends StatelessWidget {
  const AppCheckbox({super.key, required this.value, required this.onChanged})
    : _indicator = false;

  /// A non-interactive checkbox whose state and semantics are owned by its
  /// parent control, such as a selectable settings tile.
  const AppCheckbox.indicator({super.key, required this.value})
    : onChanged = null,
      _indicator = true;

  final bool value;
  final ValueChanged<bool>? onChanged;
  final bool _indicator;

  @override
  Widget build(BuildContext context) {
    final changeCallback = onChanged;

    final checkbox = Checkbox(
      value: value,
      onChanged: _indicator
          ? (_) {}
          : changeCallback == null
          ? null
          : (next) {
              if (next != null) changeCallback(next);
            },
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      visualDensity: const VisualDensity(horizontal: -4, vertical: -4),
      fillColor: WidgetStateProperty.resolveWith(
        (states) => states.contains(WidgetState.selected)
            ? StyleConstants.primaryActionColor
            : Colors.transparent,
      ),
      checkColor: StyleConstants.onPrimaryActionColor,
      side: const BorderSide(color: StyleConstants.strongLineColor),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(5)),
    );

    if (!_indicator) return checkbox;
    return ExcludeSemantics(child: IgnorePointer(child: checkbox));
  }
}
