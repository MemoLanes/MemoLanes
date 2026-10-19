import 'package:flutter/widgets.dart';
import 'package:memolanes/common/app_layout.dart' show AppLayout;
import 'package:memolanes/common/layout_geometry.dart' as geometry;

/// Places modal content inside one clear native region, consuming insets once.
class AppModalRegion extends StatelessWidget {
  const AppModalRegion({
    super.key,
    required this.child,
    this.useSafeArea = true,
  }) : alignment = null;

  /// Fits an overlay notice using the same outer budget as a Wolt dialog.
  /// It remains an overlay, so the host route can keep handling navigation.
  const AppModalRegion.dialog({
    super.key,
    required this.child,
    this.alignment = Alignment.center,
  }) : useSafeArea = true;

  final Widget child;

  /// Keep the caller's safe-area policy in a single viewport. Reserved native
  /// space is always excluded before content reaches the modal.
  final bool useSafeArea;
  final Alignment? alignment;

  @override
  Widget build(BuildContext context) {
    final layout = AppLayout.fromContext(context);
    final modal = layout.modalGeometry;
    final useSystemSafeArea = useSafeArea && modal.region == null;
    // Own safe-area consumption so a late native reading can change geometry
    // without leaving a route-created legacy SafeArea around the dialog.
    final viewport = SafeArea(
      minimum: useSystemSafeArea ? modal.safeAreaMinimum : EdgeInsets.zero,
      left: useSystemSafeArea,
      right: useSystemSafeArea,
      top: useSystemSafeArea,
      bottom: useSystemSafeArea,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final media = MediaQuery.of(context);
          final region = modal.region ?? Offset.zero & constraints.biggest;
          final bounds = alignment != null && modal.region == null
              ? geometry.insetRect(region, media.viewInsets)
              : region;
          return Padding(
            padding: EdgeInsets.fromLTRB(
              bounds.left,
              bounds.top,
              constraints.maxWidth - bounds.right,
              constraints.maxHeight - bounds.bottom,
            ),
            child: MediaQuery(
              data: modal.region != null
                  ? media.copyWith(
                      size: bounds.size,
                      padding: EdgeInsets.zero,
                      viewPadding: EdgeInsets.zero,
                      viewInsets: EdgeInsets.zero,
                    )
                  : media,
              child: alignment == null
                  ? child
                  : Align(
                      alignment: alignment!,
                      child: ConstrainedBox(
                        constraints: AppLayout.dialogConstraints(bounds.size),
                        child: child,
                      ),
                    ),
            ),
          );
        },
      ),
    );
    return alignment == null
        ? viewport
        : DisplayFeatureSubScreen(child: viewport);
  }
}
