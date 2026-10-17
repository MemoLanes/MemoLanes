import 'package:material_ui/material_ui.dart';

/// Insets explicitly consumed by a page container, inherited by its children.
/// Design margins are separate from the platform's safe-area padding.
class PageInsetsScope extends InheritedWidget {
  const PageInsetsScope({
    super.key,
    required this.horizontal,
    required this.vertical,
    required this.ownsChildMargins,
    required super.child,
  });

  final bool horizontal;
  final bool vertical;
  final bool ownsChildMargins;

  static PageInsetsScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<PageInsetsScope>();

  @override
  bool updateShouldNotify(PageInsetsScope oldWidget) =>
      horizontal != oldWidget.horizontal ||
      vertical != oldWidget.vertical ||
      ownsChildMargins != oldWidget.ownsChildMargins;
}

class SafeAreaWrapper extends StatelessWidget {
  final Widget child;
  final bool useSafeArea;

  const SafeAreaWrapper({
    super.key,
    required this.child,
    this.useSafeArea = true,
  });

  @override
  Widget build(BuildContext context) {
    final owner = PageInsetsScope.maybeOf(context);
    final horizontal = useSafeArea && owner?.horizontal != true;
    final vertical = useSafeArea && owner?.vertical != true;
    return SafeArea(
      left: horizontal,
      top: vertical,
      right: horizontal,
      bottom: vertical,
      child: Padding(
        padding: useSafeArea && owner?.ownsChildMargins != true
            ? const EdgeInsets.symmetric(horizontal: 8.0)
            : EdgeInsets.zero,
        child: child,
      ),
    );
  }
}
