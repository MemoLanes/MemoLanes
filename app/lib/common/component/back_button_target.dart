import 'package:flutter/material.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Gives an existing Back visual one accessible, 44pt touch target.
/// Placement, visual styling and the navigation action belong to the caller.
class BackButtonTarget extends StatelessWidget {
  const BackButtonTarget({
    super.key,
    required this.onPressed,
    required this.child,
  }) : _positioned = false;

  /// Reuses the same touch target and the app's resolved map Back placement.
  const BackButtonTarget.positioned({
    super.key,
    required this.onPressed,
    required this.child,
  }) : _positioned = true;

  final bool _positioned;
  final VoidCallback onPressed;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final bounds = _positioned
        ? AppLayoutScope.of(context).primaryNavigationBounds
        : null;
    final target = SizedBox.square(
      dimension: bounds?.width ?? 44,
      child: Semantics(
        button: true,
        label: MaterialLocalizations.of(context).backButtonTooltip,
        onTap: onPressed,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          excludeFromSemantics: true,
          onTap: onPressed,
          child: Center(child: ExcludeSemantics(child: child)),
        ),
      ),
    );
    return bounds != null
        ? Positioned.fromRect(
            rect: bounds,
            child: PointerInterceptor(child: target),
          )
        : target;
  }
}
