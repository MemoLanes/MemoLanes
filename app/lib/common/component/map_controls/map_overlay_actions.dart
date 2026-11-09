import 'package:flutter/widgets.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Places Back and actions in a map Stack. Only their cells intercept taps.
class MapOverlayActions extends StatelessWidget {
  const MapOverlayActions({super.key, this.back, this.actions = const []});

  final Widget? back;
  final List<Widget> actions;

  static const _extent = StyleConstants.mapControlSize;
  static const _spacing = StyleConstants.mapControlSpacing;

  Size _footprint(int count) =>
      Size(_extent, count * _extent + (count - 1) * _spacing);

  // The interceptor's Stack gives its child loose constraints on iOS.
  Widget _cell(Widget child, Size size) => PointerInterceptor(
    child: SizedBox.fromSize(size: size, child: child),
  );

  Widget _column(Rect bounds, List<Widget> children) => Positioned.fromRect(
    rect: bounds,
    child: Column(
      spacing: _spacing,
      children: [
        for (final child in children) _cell(child, const Size.square(_extent)),
      ],
    ),
  );

  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    final back = this.back;
    if (!layout.chromeReady || (back == null && actions.isEmpty)) {
      return const SizedBox.shrink();
    }
    final children = [?back, ...actions];
    final rail = layout.duoActionBoundsFor(_footprint(children.length));
    if (rail != null) return _column(rail, children);

    final backBounds = back == null ? null : layout.primaryNavigationBounds;
    final actionBounds = actions.isEmpty
        ? Rect.zero
        : layout.mapActionBoundsFor(
            _footprint(actions.length),
            back: backBounds,
          );
    return Stack(
      fit: StackFit.expand,
      children: [
        if (back != null && backBounds != null)
          Positioned.fromRect(
            rect: backBounds,
            child: _cell(back, backBounds.size),
          ),
        if (!actionBounds.isEmpty) _column(actionBounds, actions),
      ],
    );
  }
}
