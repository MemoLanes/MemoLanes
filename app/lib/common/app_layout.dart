import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    show DuoHardware, DuoHingeStatus, DuoRegion;
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/layout_geometry.dart' as geometry;
import 'package:memolanes/common/map_viewport.dart' as map_viewport;
import 'package:memolanes/constants/style_constants.dart';

/// Local placement geometry; the map and the existing cards stay mounted.
/// Sizes below describe our components, never a particular device or pose.
@immutable
class AppLayout {
  const AppLayout._({
    required this._usesRegions,
    required this._media,
    required this._navigationMedia,
    required this._platform,
    required this.size,
    required this.safeBounds,
    required this.contentBounds,
    required this.navigationBounds,
    required this.navigationContentSize,
    required this.chromeReady,
    required this.navigationAxis,
    required this.regions,
    required this.controlRegion,
    required this._panelRegion,
    required this._primaryNavigationRegions,
    required this._reservedRegions,
    required this._barEdge,
  });

  // Only the resolver knows how native regions differ from a single viewport.
  // Consumers receive usable geometry on every platform.
  final bool _usesRegions;
  final MediaQueryData _media;
  // Scaffold removes keyboard insets from content, while floating navigation
  // retains the original window's system-area spacing.
  final MediaQueryData _navigationMedia;
  final TargetPlatform _platform;
  final Size size;
  final Rect safeBounds;
  final Rect contentBounds;
  final Rect navigationBounds;
  final Size navigationContentSize;

  /// Navigation and dependent overlays wait for the first hardware response.
  final bool chromeReady;
  final Axis navigationAxis;
  final List<Rect> regions;
  final Rect controlRegion;
  final Rect _panelRegion;
  final List<Rect> _primaryNavigationRegions;
  final List<Rect> _reservedRegions;
  final DuoBarEdge _barEdge;

  // A map Scaffold keeps its full viewport; a regular Scaffold may already
  // have consumed some or all of the keyboard height in its body constraints.
  static double _remainingKeyboard(MediaQueryData media, Size size) =>
      (media.viewInsets.bottom - math.max(0.0, media.size.height - size.height))
          .clamp(0.0, size.height);

  double get _keyboardInset => _remainingKeyboard(_media, size);

  /// Camera fitting follows window and hardware geometry, not keyboard/drafts.
  bool hasSameMapViewport(AppLayout other) =>
      size == other.size &&
      _usesRegions == other._usesRegions &&
      _barEdge == other._barEdge &&
      _media.viewPadding == other._media.viewPadding &&
      listEquals(_reservedRegions, other._reservedRegions);

  /// Primary navigation stays at UIKit's physical side and clears hardware.
  Rect get primaryNavigationBounds {
    if (!_usesRegions) {
      return Rect.fromLTWH(
        _media.viewPadding.left + 16,
        _media.viewPadding.top + 14,
        42,
        42,
      );
    }
    final fitting = _primaryNavigationRegions
        .where((r) => r.width >= 44 && r.height >= 44)
        .toList();
    final region = geometry.nearestRegion(
      fitting.isEmpty ? _primaryNavigationRegions : fitting,
      _barEdge == DuoBarEdge.right ? safeBounds.topRight : safeBounds.topLeft,
    );
    final extent = math.min(44.0, math.min(region.width, region.height));
    final horizontalGap = math.min(
      16.0,
      math.max(0.0, (region.width - extent) / 2),
    );
    final verticalGap = math.min(
      14.0,
      math.max(0.0, (region.height - extent) / 2),
    );
    return Rect.fromLTWH(
      navigationAxis == Axis.vertical
          ? region.center.dx - extent / 2
          : region.left + horizontalGap,
      region.top + verticalGap,
      extent,
      extent,
    );
  }

  /// Fits selected tracks in the space beside/above the occupied detail card.
  EdgeInsets get mapFitPadding => mapFitPaddingForOverlay(panelBounds);

  /// Keeps MapLibre's fit area positive even when a short window is covered.
  EdgeInsets fitPaddingFor(Rect visible) =>
      map_viewport.fitPaddingForRegion(size, visible);

  // Keep auxiliary map tools opposite the physical vertical navigation bar,
  // selecting a clear pane without moving the recording controls' region.
  Rect get _mapControlsRegion {
    if (navigationAxis == Axis.horizontal) return controlRegion;
    final fitting = regions
        .where(
          (r) =>
              r.height >= 126 &&
              r.width >=
                  (r.height < 260
                          ? StyleConstants.mapControlsExtent
                          : StyleConstants.mapControlSize) +
                      32,
        )
        .toList();
    return geometry.nearestRegion(
      fitting.isEmpty ? [controlRegion] : fitting,
      _barEdge == DuoBarEdge.right
          ? contentBounds.bottomLeft
          : contentBounds.bottomRight,
    );
  }

  Axis get mapControlsAxis =>
      _mapControlsRegion.height < 260 ? Axis.horizontal : Axis.vertical;

  Rect get mapControlsBounds {
    final region = _mapControlsRegion;
    final horizontal = mapControlsAxis == Axis.horizontal;
    final sideNavigation = navigationAxis == Axis.vertical;
    final landscape = _media.orientation == Orientation.landscape;
    final available = geometry.insetRect(
      region,
      EdgeInsets.symmetric(
        horizontal: sideNavigation ? 16 : (landscape ? 32 : 18),
        vertical: 12,
      ),
    );
    final width = math.min(
      horizontal
          ? StyleConstants.mapControlsExtent
          : StyleConstants.mapControlSize,
      available.width,
    );
    final height = math.min(
      horizontal
          ? StyleConstants.mapControlSize
          : StyleConstants.mapControlsExtent,
      available.height,
    );
    final bottom = sideNavigation
        ? region.bottom
        : navigationBounds.isEmpty
        ? size.height - primaryControlBottomInset
        : navigationBounds.top -
              StyleConstants.mapPrimaryControlNavBarSpacing -
              math.max(
                0.0,
                navigationContentSize.height - navigationBounds.height,
              );
    return Rect.fromLTWH(
      sideNavigation && _barEdge == DuoBarEdge.right
          ? available.left
          : available.right - width,
      (bottom - height - (sideNavigation || !landscape ? 82 : 0)).clamp(
        available.top,
        math.max(available.top, available.bottom - height),
      ),
      width,
      height,
    );
  }

  /// Popups stay in the region containing their anchor; unanchored modals use
  /// the same interaction region as the primary controls.
  Rect regionFor(Offset anchor) => geometry.nearestRegion(regions, anchor);

  /// Center the primary recording capsule on the window unless its footprint
  /// would cross real reserved space. A fold then uses the nearest clear pane.
  Rect recordingControlsBoundsFor(
    Size footprint, {
    required double minimumWidth,
  }) {
    if (!_usesRegions) {
      final inset = _horizontalSafeInset + 24;
      final width = contentConstraints(
        size,
        padding: EdgeInsets.symmetric(horizontal: inset),
        maxWidth: footprint.width,
      ).maxWidth;
      return Rect.fromLTWH(
        (size.width - width) / 2,
        size.height - primaryControlBottomInset - footprint.height,
        width,
        footprint.height,
      );
    }
    final center = Offset(
      size.width / 2,
      controlRegion.bottom - 22 - footprint.height / 2,
    );
    final target = Rect.fromCenter(
      center: center,
      width: math.min(footprint.width, math.max(0.0, size.width - 48)),
      height: footprint.height,
    );
    var region = Rect.fromLTRB(
      0,
      controlRegion.top,
      size.width,
      controlRegion.bottom,
    );
    if (_reservedRegions.any((r) => r.overlaps(target))) {
      final fitting = regions
          .where(
            (r) =>
                r.width >= minimumWidth + 48 &&
                r.height >= footprint.height + 44,
          )
          .toList();
      double distance(Rect r) =>
          (Offset(r.center.dx, r.bottom - 22 - footprint.height / 2) - center)
              .distanceSquared;
      region = (fitting.isEmpty ? regions : fitting).reduce(
        (a, b) => distance(b) < distance(a) ? b : a,
      );
    }
    var halfWidth =
        math.min(footprint.width, math.max(0.0, region.width - 48)) / 2;
    final regionCenter = region.center.dx;
    // Keep a centered container clear of the actual tab bar. Long labels can
    // use their existing ellipsis without shifting the primary capsule.
    if (navigationBounds.height > 0 &&
        navigationBounds.top < target.bottom &&
        navigationBounds.bottom > target.top) {
      final available = navigationBounds.center.dx > regionCenter
          ? navigationBounds.left - regionCenter - 12
          : regionCenter - navigationBounds.right - 12;
      halfWidth = math.min(halfWidth, math.max(0.0, available));
    }
    return Rect.fromLTWH(
      regionCenter - halfWidth,
      region.bottom - 22 - footprint.height,
      halfWidth * 2,
      footprint.height,
    );
  }

  /// Selects a clear region that can hold the existing control's natural size.
  /// If none fits, its caller can scroll within the largest available region.
  Rect controlRegionFor({
    double minimumWidth = 256,
    double minimumHeight = 126,
  }) => _controlRegionFor(
    regions,
    contentBounds,
    minimumWidth: minimumWidth,
    minimumHeight: minimumHeight,
  );

  double get _navigationBottomInset =>
      StyleConstants.navBarBottomInsetForMedia(_navigationMedia, _platform);
  double get _horizontalSafeInset =>
      math.max(_media.viewPadding.left, _media.viewPadding.right);
  double get primaryControlBottomInset =>
      StyleConstants.navBarHeight +
      StyleConstants.navBarBottomInsetForMedia(
        _keyboardInset > 0
            ? _media.removeViewInsets(removeBottom: true)
            : _media,
        _platform,
      ) +
      _keyboardInset +
      StyleConstants.mapPrimaryControlNavBarSpacing;

  Offset get trailingOverlayInsets => _usesRegions
      ? Offset(
          size.width - controlRegion.right + 8,
          size.height - controlRegion.bottom + 4,
        )
      : Offset(
          _media.viewPadding.right + 8,
          _navigationBottomInset +
              StyleConstants.navBarHeight +
              6 +
              _keyboardInset,
        );

  /// Side clearance belongs outside the viewport; vertical clearance scrolls
  /// with the content so system areas never create a clipped bottom strip.
  EdgeInsets get menuInsets => _usesRegions
      ? EdgeInsets.only(
          left: contentBounds.left,
          right: size.width - contentBounds.right,
        )
      : EdgeInsets.symmetric(horizontal: _horizontalSafeInset + 8);
  EdgeInsets get menuScrollInsets => _usesRegions
      ? EdgeInsets.only(
          top: contentBounds.top,
          bottom: size.height - contentBounds.bottom,
        )
      : EdgeInsets.only(
          top: _navigationMedia.padding.top,
          bottom: StyleConstants.navBarHeight + _navigationBottomInset,
        );
  // Frames consume physical sides once; unframed content can use SafeArea.
  // Zero still claims the sides, so descendants do not add their own insets.
  EdgeInsets? contentInsets(
    MediaQueryData media, {
    required bool maintainViewPadding,
  }) {
    if (!maintainViewPadding) return _usesRegions ? media.padding : null;
    return _usesRegions
        ? EdgeInsets.only(
            left: media.viewPadding.left,
            right: media.viewPadding.right,
          )
        : EdgeInsets.symmetric(
            horizontal: math.max(
              media.viewPadding.left,
              media.viewPadding.right,
            ),
          );
  }

  EdgeInsets insetsFor(Rect bounds) => EdgeInsets.fromLTRB(
    bounds.left,
    bounds.top,
    size.width - bounds.right,
    size.height - bounds.bottom,
  );

  /// Menus clear hardware, the keyboard and the host page's navigation bounds.
  /// The host supplies its actual navigation geometry when menus use the full
  /// Navigator overlay rather than that page's content viewport.
  Rect menuRegionFor(
    Offset anchor, {
    Size? overlaySize,
    Rect? navigationBounds,
  }) {
    final bounds = _usesRegions
        ? regionFor(anchor)
        : _media.padding.deflateRect(
            _media.viewInsets.deflateRect(
              Offset.zero & (overlaySize ?? _media.size),
            ),
          );
    return geometry.nearestRegion(
      geometry.clearRegions(bounds, [
        navigationBounds ?? this.navigationBounds,
      ]),
      anchor,
    );
  }

  /// One width budget for page content, map cards and Wolt dialogs.
  static BoxConstraints contentConstraints(
    Size availableSize, {
    EdgeInsets padding = EdgeInsets.zero,
    double maxWidth = StyleConstants.pageContentMaxWidth,
  }) => BoxConstraints(
    maxWidth: math.min(
      maxWidth,
      math.max(0.0, availableSize.width - padding.horizontal),
    ),
    maxHeight: math.max(0.0, availableSize.height - padding.vertical),
  );

  /// Resolved modals consume their region's insets; other modals use
  /// their local viewport and symmetric system safe-area minimum.
  ({Rect? region, EdgeInsets safeAreaMinimum}) get modalGeometry => (
    region: _usesRegions ? controlRegion : null,
    safeAreaMinimum: EdgeInsets.symmetric(horizontal: _horizontalSafeInset),
  );

  /// Shared outer budget for Wolt dialogs and notices without a modal route.
  static BoxConstraints dialogConstraints(Size availableSize) {
    final horizontalGap = math.min(
      24.0,
      math.max(8.0, (availableSize.width - StyleConstants.contentMinWidth) / 2),
    );
    final budget = contentConstraints(
      availableSize,
      padding: EdgeInsets.symmetric(horizontal: horizontalGap, vertical: 24),
      maxWidth: StyleConstants.dialogMaxWidth,
    );
    return budget.tighten(width: budget.maxWidth);
  }

  /// A card stays centered in its clear pane. Its width budget leaves map
  /// space as the pane widens without changing the card's alignment.
  Rect get panelBounds {
    final available = geometry.insetRect(
      _panelRegion,
      const EdgeInsets.all(16),
    );
    const mapGap = 16.0;
    // Give excess horizontal space to the map gradually. Increasing the pane
    // width must never make a card narrower at a layout breakpoint.
    final mapSpace =
        (available.width -
                math.max(StyleConstants.contentMinWidth, available.height))
            .clamp(0.0, StyleConstants.minimumMapExtent + mapGap);
    final budget = contentConstraints(
      Size(available.width - mapSpace, available.height),
    );
    return Alignment.bottomCenter.inscribe(budget.biggest, available);
  }

  /// Browsing cards share the page width while leaving map space vertically.
  Rect get pickerBounds {
    final available = geometry.insetRect(
      _panelRegion,
      const EdgeInsets.all(16),
    );
    final budget = contentConstraints(available.size);
    final preferredHeight = (size.height * 0.62).clamp(
      StyleConstants.mapPickerMinHeight,
      StyleConstants.mapPickerMaxHeight,
    );
    final height = math.min(budget.maxHeight, preferredHeight);
    return Alignment.bottomCenter.inscribe(
      Size(budget.maxWidth, height),
      available,
    );
  }

  Rect get detailBounds {
    final panel = panelBounds;
    final back = primaryNavigationBounds;
    return Rect.fromLTRB(
      panel.left,
      math.min(
        panel.bottom,
        math.max(
          panel.top,
          back.overlaps(panel) ? back.bottom + 12 : panel.top,
        ),
      ),
      panel.right,
      panel.bottom,
    );
  }

  EdgeInsets get primaryControlsInsets {
    final side = _usesRegions ? 24.0 : _horizontalSafeInset + 24;
    final region = _usesRegions ? controlRegion : Offset.zero & size;
    final width = contentConstraints(
      region.size,
      padding: EdgeInsets.symmetric(horizontal: side),
      maxWidth: StyleConstants.timelineControlsMaxWidth,
    ).maxWidth;
    final left = region.center.dx - width / 2;
    return EdgeInsets.fromLTRB(
      left,
      0,
      size.width - left - width,
      _usesRegions
          ? size.height - region.bottom + 22
          : primaryControlBottomInset,
    );
  }

  Rect toolbarBoundsFor(double minimumHeight) => _usesRegions
      ? controlRegionFor(minimumHeight: minimumHeight)
      : Offset.zero & size;
  EdgeInsets toolbarPadding(double minimum) => _usesRegions
      ? EdgeInsets.all(minimum)
      : EdgeInsets.fromLTRB(
          math.max(minimum, _media.padding.left),
          math.max(minimum, _media.padding.top),
          math.max(minimum, _media.padding.right),
          math.max(minimum, _media.padding.bottom),
        );

  Rect? get detachedBackBounds => _usesRegions ? primaryNavigationBounds : null;
  Rect get headerBounds =>
      _usesRegions ? regionFor(safeBounds.topLeft) : Offset.zero & size;
  double get headerTopInset => _usesRegions ? 0 : _media.padding.top * .8;
  EdgeInsets get headerContentInsets => EdgeInsets.fromLTRB(
    (_usesRegions ? 0 : _media.padding.left) +
        StyleConstants.overlayBarHorizontalInset,
    headerTopInset,
    (_usesRegions ? 0 : _media.padding.right) +
        StyleConstants.overlayBarHorizontalInset,
    StyleConstants.overlayBarBottomInset,
  );
  bool get backPrecedesEdgeGesture => detachedBackBounds == null;

  /// Follow the top-left header region independently of recording actions.
  /// Mirror a bottom safe inset when a side status bar leaves no top inset,
  /// keeping the badge clear of the rounded top corner.
  Offset get recordingIndicatorOffset => _usesRegions
      ? Offset(
          headerBounds.left + 18,
          math.max(headerBounds.top, _media.viewPadding.bottom) + 8,
        )
      : Offset(_media.padding.left + 18, _media.padding.top + 8);

  /// Every map fits against the existing panel's measured footprint.
  EdgeInsets mapFitPaddingForOverlay(Rect occupied) =>
      map_viewport.fitPaddingAroundOverlay(
        viewportSize: size,
        regions: regions,
        occupied: occupied,
      );

  EdgeInsets editorMapFitPadding({
    required double toolbarHeight,
    required double margin,
  }) {
    if (!_usesRegions) {
      final height =
          toolbarHeight + margin + math.max(_media.padding.bottom, margin);
      return mapFitPaddingForOverlay(
        Rect.fromLTWH(0, size.height - height, size.width, height),
      );
    }
    final region = regionFor(safeBounds.topLeft);
    return fitPaddingFor(
      Rect.fromLTRB(
        region.left,
        region.top + 64,
        region.right,
        math.max(region.top + 64, region.bottom - toolbarHeight - margin * 2),
      ),
    );
  }

  static AppLayout fromContext(
    BuildContext context, {
    Size? size,
    MediaQueryData? media,
    bool navigationVisible = false,
    MediaQueryData? navigationMedia,
  }) {
    // The hardware layout aspect excludes continuous hinge-angle updates.
    final hardware = DuoHardware.of(context);
    media ??= MediaQuery.of(context);
    return resolve(
      media: media,
      size:
          size ??
          Size(media.size.width, media.size.height - media.viewInsets.bottom),
      hardware: hardware,
      barEdge: DuoBarScope.of(context),
      chromeReady: DuoBarScope.isReadyOf(context),
      platform: defaultTargetPlatform,
      navigationVisible: navigationVisible,
      navigationMedia: navigationMedia,
    );
  }

  static AppLayout resolve({
    required MediaQueryData media,
    required Size size,
    required DuoHardware hardware,
    required DuoBarEdge barEdge,
    required TargetPlatform platform,
    bool navigationVisible = true,
    bool chromeReady = true,
    MediaQueryData? navigationMedia,
  }) {
    navigationMedia ??= media;
    final viewport = Offset.zero & size;
    final keyboardInset = _remainingKeyboard(media, size);
    final availableBottom = size.height - keyboardInset;
    final enabled =
        platform == TargetPlatform.iOS &&
        (hardware.hasHinge ||
            barEdge == DuoBarEdge.left ||
            barEdge == DuoBarEdge.right) &&
        barEdge != DuoBarEdge.unknown;
    final padding = media.viewPadding;
    final safe = geometry.insetRect(
      viewport,
      EdgeInsets.fromLTRB(
        padding.left,
        padding.top,
        padding.right,
        media.viewInsets.bottom > 0 ? keyboardInset : padding.bottom,
      ),
    );
    if (!enabled) {
      final horizontalInset = math.max(padding.left, padding.right);
      // Match navigation's symmetric safe area so a one-sided cutout cannot
      // shift cards away from the window center. Duo keeps its native panes.
      final panelRegion = geometry.insetRect(
        safe,
        EdgeInsets.only(
          left: horizontalInset - padding.left,
          right: horizontalInset - padding.right,
        ),
      );
      final bottom = StyleConstants.navBarBottomInsetForMedia(
        navigationMedia,
        platform,
      );
      final naturalWidth = math.max(0.0, media.size.width - 48);
      final availableWidth = math.max(0.0, size.width - 2 * horizontalInset);
      final scale = naturalWidth == 0
          ? 1.0
          : math.min(1.0, availableWidth / naturalWidth);
      // Preserve the existing safe-area scale before capping visible width;
      // a narrower bar must not change its height or icon sizes.
      final navigationSize = contentConstraints(
        Size(naturalWidth * scale, StyleConstants.navBarHeight * scale),
      ).biggest;
      return AppLayout._(
        usesRegions: false,
        media: media,
        navigationMedia: navigationMedia,
        platform: platform,
        size: size,
        safeBounds: safe,
        contentBounds: viewport,
        navigationBounds: navigationVisible
            ? Rect.fromLTWH(
                (size.width - navigationSize.width) / 2,
                availableBottom - bottom - navigationSize.height,
                navigationSize.width,
                navigationSize.height,
              )
            : Rect.zero,
        navigationContentSize: Size(
          scale == 0 ? naturalWidth : navigationSize.width / scale,
          StyleConstants.navBarHeight,
        ),
        chromeReady: chromeReady,
        navigationAxis: Axis.horizontal,
        regions: [safe],
        controlRegion: safe,
        panelRegion: Rect.fromLTRB(
          panelRegion.left,
          safe.top,
          panelRegion.right,
          navigationVisible
              ? math.max(
                  safe.top,
                  math.min(
                    safe.bottom,
                    availableBottom - bottom - navigationSize.height,
                  ),
                )
              : safe.bottom,
        ),
        primaryNavigationRegions: [viewport],
        reservedRegions: const [],
        barEdge: barEdge,
      );
    }
    // SDK frames already include margins; deflating them changes the safe area.
    bool inWindow(DuoRegion r) =>
        r.frame.width > 0 &&
        r.frame.height > 0 &&
        r.frame.left >= 0 &&
        r.frame.top >= 0 &&
        r.frame.right <= media.size.width + 1 &&
        r.frame.bottom <= media.size.height + 1;
    final folds = hardware.folds
        .where(
          (r) =>
              inWindow(r) &&
              // Activation can lag UIKit's hinge update in either direction.
              (hardware.status == DuoHingeStatus.partiallyOpen ||
                  (hardware.status == DuoHingeStatus.unknown && r.isActive)),
        )
        .toList();
    final cameras = hardware.cameras
        .where((r) => inWindow(r) && r.isActive)
        .toList();
    final reserved = [...folds, ...cameras].map((r) => r.frame).toList();
    final vertical = barEdge == DuoBarEdge.left || barEdge == DuoBarEdge.right;
    final axis = vertical ? Axis.vertical : Axis.horizontal;
    Rect band = safe;
    if (vertical) {
      final inset = barEdge == DuoBarEdge.left ? padding.left : padding.right;
      final strip = Rect.fromLTWH(
        barEdge == DuoBarEdge.left ? 0 : size.width - inset,
        safe.top,
        inset,
        safe.height,
      );
      // The same physical band serves bottom tabs and top Back, including
      // detail pages where the tabs are hidden. Never guess camera/status size.
      if (inset >= StyleConstants.navBarHeight + 24 &&
          cameras.any((r) => r.frame.overlaps(strip))) {
        band = strip;
      }
    }
    final clear = geometry.clearRegions(band, reserved);
    final fitting = clear
        .where((r) => r.width >= 68 && r.height >= 68)
        .toList();
    final free = geometry.nearestRegion(
      fitting.isEmpty ? clear : fitting,
      vertical
          ? (barEdge == DuoBarEdge.left ? band.bottomLeft : band.bottomRight)
          : band.bottomCenter,
    );
    final barWidth = math.min(
      StyleConstants.navBarHeight,
      math.max(0.0, free.width - 24),
    );
    final barLeft = band == safe
        ? (barEdge == DuoBarEdge.left
              ? free.left + 12
              : free.right - barWidth - 12)
        : free.center.dx - barWidth / 2;
    final primaryWidth = math.min(44.0, band.width);
    final primaryBand = vertical
        ? Rect.fromLTWH(
            (barLeft + barWidth / 2 - primaryWidth / 2).clamp(
              band.left,
              band.right - primaryWidth,
            ),
            band.top,
            primaryWidth,
            band.height,
          )
        : band;
    // Restrict Back to the tab column before subtraction so a fragment beside
    // an occlusion cannot put it next to the status cluster.
    final primaryNavigationRegions = vertical
        ? geometry.clearRegions(primaryBand, reserved)
        : clear;
    Rect navigation = Rect.zero;
    Rect content = safe;
    if (navigationVisible) {
      if (vertical) {
        final height = math.min(
          StyleConstants.navBarHeight * 5,
          math.max(0.0, free.height - 24),
        );
        navigation = Rect.fromLTWH(
          barLeft,
          free.bottom - height - 12,
          barWidth,
          height,
        );
        if (navigation.overlaps(safe)) {
          content = barEdge == DuoBarEdge.left
              ? Rect.fromLTRB(
                  math.min(safe.right, navigation.right + 14),
                  safe.top,
                  safe.right,
                  safe.bottom,
                )
              : Rect.fromLTRB(
                  safe.left,
                  safe.top,
                  math.max(safe.left, navigation.left - 14),
                  safe.bottom,
                );
        }
      } else {
        final width = contentConstraints(
          free.size,
          padding: const EdgeInsets.symmetric(horizontal: 24),
        ).maxWidth;
        final height = math.min(
          StyleConstants.navBarHeight,
          math.max(0.0, free.height - 24),
        );
        navigation = Rect.fromLTWH(
          free.center.dx - width / 2,
          free.bottom - height - 12,
          width,
          height,
        );
        content = Rect.fromLTRB(
          safe.left,
          safe.top,
          safe.right,
          math.max(safe.top, navigation.top - 14),
        );
      }
    }
    final regions = geometry.clearRegions(content, reserved);
    final controls = _controlRegionFor(regions, content);
    Rect panelRegion = controls;
    final sideways = folds.any(
      (r) =>
          r.frame.height > r.frame.width &&
          r.frame.height >= content.height * 0.75,
    );
    if (sideways) {
      panelRegion = geometry.nearestRegion(
        regions,
        barEdge == DuoBarEdge.left ? content.bottomRight : content.bottomLeft,
      );
    }
    return AppLayout._(
      usesRegions: true,
      media: media,
      navigationMedia: navigationMedia,
      platform: platform,
      size: size,
      safeBounds: safe,
      contentBounds: content,
      navigationBounds: navigation,
      navigationContentSize: navigation.size,
      chromeReady: chromeReady,
      navigationAxis: axis,
      regions: regions,
      controlRegion: controls,
      panelRegion: panelRegion,
      primaryNavigationRegions: primaryNavigationRegions,
      reservedRegions: reserved,
      barEdge: barEdge,
    );
  }

  static Rect _controlRegionFor(
    List<Rect> regions,
    Rect content, {
    double minimumWidth = 256,
    double minimumHeight = 126,
  }) {
    final fitting = regions
        .where((r) => r.width >= minimumWidth && r.height >= minimumHeight)
        .toList();
    return fitting.isEmpty
        ? regions.reduce(
            (a, b) => a.width * a.height > b.width * b.height ? a : b,
          )
        : geometry.nearestRegion(fitting, content.bottomRight);
  }
}

class AppLayoutScope extends InheritedWidget {
  const AppLayoutScope({super.key, required this.layout, required super.child});

  final AppLayout layout;

  static AppLayout? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<AppLayoutScope>()?.layout;

  static AppLayout of(BuildContext context) {
    final layout = maybeOf(context);
    assert(layout != null, 'Place this content inside AppLayoutBuilder.');
    return layout!;
  }

  @override
  bool updateShouldNotify(AppLayoutScope oldWidget) =>
      layout != oldWidget.layout;
}

class AppLayoutBuilder extends StatelessWidget {
  const AppLayoutBuilder({
    super.key,
    required this.builder,
    this.navigationVisible = false,
    this.navigationMedia,
  });

  final Widget Function(BuildContext, AppLayout) builder;
  final bool navigationVisible;

  /// Window metrics captured before Scaffold consumes the keyboard inset.
  final MediaQueryData? navigationMedia;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (context, constraints) {
      final layout = AppLayout.fromContext(
        context,
        size: constraints.biggest,
        navigationVisible: navigationVisible,
        navigationMedia: navigationMedia,
      );
      return AppLayoutScope(
        layout: layout,
        child: Builder(builder: (context) => builder(context, layout)),
      );
    },
  );
}
