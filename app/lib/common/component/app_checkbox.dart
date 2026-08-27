import 'package:flutter/material.dart';

/// The shared compact checkbox used throughout the app.
///
/// Colors, border, and shape come from the app-level [CheckboxThemeData].
class AppCheckbox extends StatelessWidget {
  const AppCheckbox({
    super.key,
    required this.value,
    required this.onChanged,
  }) : _indicator = false;

  /// A non-interactive checkbox whose state and semantics are owned by its
  /// parent control, such as a selectable settings tile.
  const AppCheckbox.indicator({
    super.key,
    required this.value,
  })  : onChanged = null,
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
    );

    if (!_indicator) return checkbox;
    return ExcludeSemantics(
      child: IgnorePointer(child: checkbox),
    );
  }
}
