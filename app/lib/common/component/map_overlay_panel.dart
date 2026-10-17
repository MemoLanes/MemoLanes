import 'package:flutter/widgets.dart';
import 'package:memolanes/common/app_layout.dart';

enum _PanelKind { picker, detail, controls, toolbar }

/// Places persistent map content using the same resolved geometry as modals.
class MapOverlayPanel extends StatefulWidget {
  const MapOverlayPanel.picker({super.key, required this.child})
    : _kind = _PanelKind.picker,
      minimumHeight = 0,
      minimumPadding = 0;
  const MapOverlayPanel.detail({super.key, required this.child})
    : _kind = _PanelKind.detail,
      minimumHeight = 0,
      minimumPadding = 0;
  const MapOverlayPanel.controls({super.key, required this.child})
    : _kind = _PanelKind.controls,
      minimumHeight = 0,
      minimumPadding = 0;
  const MapOverlayPanel.toolbar({
    super.key,
    required this.child,
    required this.minimumHeight,
    this.minimumPadding = 16,
  }) : _kind = _PanelKind.toolbar;

  final Widget child;
  final _PanelKind _kind;

  final double minimumHeight;
  final double minimumPadding;

  @override
  State<MapOverlayPanel> createState() => MapOverlayPanelState();
}

class MapOverlayPanelState extends State<MapOverlayPanel> {
  final _contentKey = GlobalKey();

  /// Read the current natural card size when fitting or resizing the map.
  /// The camera stays still during card dragging and draft edits.
  EdgeInsets get mapFitPadding {
    assert(widget._kind == _PanelKind.detail);
    final layout = AppLayoutScope.of(context);
    final frame = layout.detailBounds;
    final box = _contentKey.currentContext?.findRenderObject() as RenderBox?;
    final height = box != null && box.hasSize ? box.size.height : 0.0;
    final occupied = Rect.fromLTRB(
      frame.left,
      frame.bottom - height,
      frame.right,
      frame.bottom,
    );
    return layout.mapFitPaddingForOverlay(occupied);
  }

  @override
  Widget build(BuildContext context) {
    final layout = AppLayoutScope.of(context);
    // The resolved panel already clears window chrome. Scrollbars, SafeArea
    // and nested content use this local viewport without consuming it again.
    final child = MediaQuery.removePadding(
      context: context,
      removeLeft: true,
      removeTop: true,
      removeRight: true,
      removeBottom: true,
      child: widget.child,
    );
    if (widget._kind == _PanelKind.controls) {
      final insets = layout.primaryControlsInsets;
      return Positioned(
        left: insets.left,
        right: insets.right,
        bottom: insets.bottom,
        child: child,
      );
    }
    final bounds = switch (widget._kind) {
      _PanelKind.picker => layout.pickerBounds,
      _PanelKind.detail => layout.detailBounds,
      _PanelKind.toolbar => layout.toolbarBoundsFor(widget.minimumHeight),
      _PanelKind.controls => throw StateError('Controls use natural height'),
    };
    return Positioned(
      left: bounds.left,
      right: layout.size.width - bounds.right,
      top: widget._kind == _PanelKind.detail ? bounds.top : null,
      bottom: layout.size.height - bounds.bottom,
      child: switch (widget._kind) {
        _PanelKind.picker => ConstrainedBox(
          constraints: BoxConstraints(maxHeight: bounds.height),
          child: child,
        ),
        _PanelKind.detail => Align(
          alignment: Alignment.bottomCenter,
          child: SizedBox(key: _contentKey, width: bounds.width, child: child),
        ),
        _PanelKind.toolbar => ConstrainedBox(
          constraints: BoxConstraints(maxHeight: bounds.height),
          child: Padding(
            padding: layout.toolbarPadding(widget.minimumPadding),
            child: child,
          ),
        ),
        _PanelKind.controls => child,
      },
    );
  }
}
