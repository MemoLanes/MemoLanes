import 'dart:io';

import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

enum AppNavigationContentMode { fullBleed, inset }

typedef AppNavigationBodyBuilder = Widget Function({
  required double topSafeArea,
  required double bottomSafeArea,
});

typedef AppNavigationBuilder = Widget Function(
  BuildContext context, {
  required Axis axis,
});

/// Places the app's existing content and navigation in one stable layout tree.
/// Navigation actions, page selection and visual styling belong to its callers.
class AppNavigationShell extends StatelessWidget {
  const AppNavigationShell({
    super.key,
    required this.bodyBuilder,
    required this.navigationBuilder,
    this.navigationVisible = true,
    this.contentMode = AppNavigationContentMode.fullBleed,
    this.bottomTrailingOverlay,
  });

  /// Scroll-content offsets resolved by the shared layout on every device.
  final AppNavigationBodyBuilder bodyBuilder;
  final AppNavigationBuilder navigationBuilder;
  final bool navigationVisible;
  final AppNavigationContentMode contentMode;

  /// Small map overlay aligned above the navigation, hidden with app chrome.
  final Widget? bottomTrailingOverlay;

  @override
  Widget build(BuildContext context) {
    // Keep these readings outside Scaffold, which removes the keyboard inset
    // from its body's MediaQuery and supplies the reduced layout constraints.
    final mediaQuery = MediaQuery.of(context);
    final insetContent = contentMode == AppNavigationContentMode.inset;

    return Scaffold(
      // Keep the native map viewport stable while its overlay clears the
      // keyboard. Scrolling menu pages retain Scaffold's normal resizing.
      resizeToAvoidBottomInset: insetContent,
      body: AppLayoutBuilder(
        navigationVisible: navigationVisible,
        navigationMedia: mediaQuery,
        builder: (context, layout) => Stack(
          children: [
            // Menu pages keep vertical insets inside their scroll content so
            // the viewport extends beneath system chrome and the nav bar.
            PageContentInsets.menu(
              layout: layout,
              enabled: insetContent,
              child: bodyBuilder(
                topSafeArea: insetContent ? layout.menuScrollInsets.top : 0,
                bottomSafeArea: layout.menuScrollInsets.bottom,
              ),
            ),
            if (navigationVisible && layout.chromeReady)
              Positioned.fromRect(
                rect: layout.navigationBounds,
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  child: SizedBox.fromSize(
                    size: layout.navigationContentSize,
                    // TODO: Remove this iOS PlatformView composition workaround
                    // once Flutter #190003 is included in the stable SDK:
                    // https://github.com/flutter/flutter/pull/190003
                    child: PointerInterceptor(
                      intercepting: Platform.isIOS,
                      child: navigationBuilder(
                        context,
                        axis: layout.navigationAxis,
                      ),
                    ),
                  ),
                ),
              ),
            if (navigationVisible &&
                layout.chromeReady &&
                bottomTrailingOverlay != null)
              Positioned(
                right: layout.trailingOverlayInsets.dx,
                bottom: layout.trailingOverlayInsets.dy,
                child: bottomTrailingOverlay!,
              ),
          ],
        ),
      ),
    );
  }
}
