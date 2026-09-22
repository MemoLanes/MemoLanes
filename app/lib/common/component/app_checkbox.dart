import 'package:flutter/material.dart';

enum AppCheckboxShape { roundedSquare, circle }

/// The shared compact checkbox used throughout the app.
///
/// Colors, border, and shape come from the app-level [CheckboxThemeData].
/// Keeping only sizing and null handling here prevents feature pages from
/// introducing another checkbox style.
class AppCheckbox extends StatelessWidget {
  const AppCheckbox({
    super.key,
    required this.value,
    required this.onChanged,
    this.shape = AppCheckboxShape.roundedSquare,
  }) : _indicator = false;

  /// A non-interactive selection indicator whose semantics belong to its parent.
  const AppCheckbox.indicator({
    super.key,
    required this.value,
    this.shape = AppCheckboxShape.roundedSquare,
  }) : onChanged = null,
       _indicator = true;

  final bool value;
  final ValueChanged<bool>? onChanged;
  final AppCheckboxShape shape;
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
      shape: shape == AppCheckboxShape.circle ? const CircleBorder() : null,
    );

    if (!_indicator) return checkbox;
    return ExcludeSemantics(child: IgnorePointer(child: checkbox));
  }
}
