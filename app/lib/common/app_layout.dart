import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    show DuoHardware, DuoHingeStatus, DuoRegion;
import 'package:memolanes/common/duo_device.dart';
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
    final region = _nearest(
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
  EdgeInsets get mapFitPadding => _mapFitPaddingAround(panelBounds);

  EdgeInsets _mapFitPaddingAround(Rect occupied) {
    var visible =
        [
              for (final region in regions)
                ..._subtract(region, occupied.inflate(16)),
            ]
            .where((region) => region.width >= 120 && region.height >= 120)
            .fold(
              Rect.zero,
              (a, b) => b.width * b.height > a.width * a.height ? b : a,
            );
    // A keyboard or short window can leave no useful rectangle beside the
    // floating card. Fit safely within its usable region until space returns.
    if (visible.isEmpty) {
      visible = regions.reduce(
        (a, b) => a.width * a.height > b.width * b.height ? a : b,
      );
    }
    return fitPaddingFor(visible);
  }

  /// Keeps MapLibre's fit area positive even when a short window is covered.
  EdgeInsets fitPaddingFor(Rect visible) {
    final left = math.max(28.0, visible.left + 16);
    final top = math.max(28.0, visible.top + 16);
    final right = math.max(28.0, size.width - visible.right + 16);
    final bottom = math.max(28.0, size.height - visible.bottom + 16);
    final horizontalScale = math.min(
      1.0,
      math.max(0.0, size.width - math.min(120.0, size.width)) / (left + right),
    );
    final verticalScale = math.min(
      1.0,
      math.max(0.0, size.height - math.min(120.0, size.height)) /
          (top + bottom),
    );
    return EdgeInsets.fromLTRB(
      left * horizontalScale,
      top * verticalScale,
      right * horizontalScale,
      bottom * verticalScale,
    );
  }

  // Keep auxiliary map tools opposite the physical vertical navigation bar,
  // selecting a clear pane without moving the recording controls' region.
  Rect get _mapControlsRegion {
    if (navigationAxis == Axis.horizontal) return controlRegion;
    final fitting = regions
        .where(
          (r) => r.height >= 126 && r.width >= (r.height < 260 ? 148 : 44) + 32,
        )
        .toList();
    return _nearest(
      fitting.isEmpty ? [controlRegion] : fitting,
      _barEdge == DuoBarEdge.right
          ? contentBounds.bottomLeft
          : contentBounds.bottomRight,
    );
  }

  Axis get mapControlsAxis => _usesRegions && _mapControlsRegion.height < 260
      ? Axis.horizontal
      : Axis.vertical;

  Rect get mapControlsBounds {
    if (!_usesRegions) {
      final landscape = _media.orientation == Orientation.landscape;
      return Rect.fromLTWH(
        size.width - _media.viewPadding.right - (landscape ? 32 : 18) - 44,
        size.height - primaryControlBottomInset - (landscape ? 0 : 82) - 148,
        44,
        148,
      );
    }
    final region = _mapControlsRegion;
    final horizontal = mapControlsAxis == Axis.horizontal;
    final width = math.min(
      horizontal ? 148.0 : 44.0,
      math.max(0.0, region.width - 32),
    );
    final height = math.min(
      horizontal ? 44.0 : 148.0,
      math.max(0.0, region.height - 24),
    );
    return Rect.fromLTWH(
      navigationAxis == Axis.vertical && _barEdge == DuoBarEdge.right
          ? region.left + 16
          : region.right - width - 16,
      math.max(region.top + 12, region.bottom - height - 82),
      width,
      height,
    );
  }

  /// Popups stay in the region containing their anchor; unanchored modals use
  /// the same interaction region as the primary controls.
  Rect regionFor(Offset anchor) => _nearest(regions, anchor);

  /// Center the primary recording capsule on the window unless its footprint
  /// would cross real reserved space. A fold then uses the nearest clear pane.
  Rect recordingControlsBoundsFor(
    Size footprint, {
    required double minimumWidth,
  }) {
    if (!_usesRegions) {
      final inset =
          math.max(_media.viewPadding.left, _media.viewPadding.right) + 24;
      return Rect.fromLTWH(
        inset,
        size.height - primaryControlBottomInset - footprint.height,
        math.max(0.0, size.width - 2 * inset),
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
    var halfWidth = math.max(0.0, region.width - 48) / 2;
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

  Size get navigationContentSize => _usesRegions
      ? navigationBounds.size
      : Size(
          math.max(0.0, _media.size.width - 48),
          StyleConstants.navBarHeight,
        );
  bool get navigationAllowsOverflow => _usesRegions;
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

  /// Menu viewports consume physical insets; ordinary scrolling pages retain
  /// their original top/bottom scroll padding and 8pt design margin.
  EdgeInsets get menuInsets => _usesRegions
      ? insetsFor(contentBounds)
      : EdgeInsets.only(
          left: _media.viewPadding.left + 8,
          right: _media.viewPadding.right + 8,
        );
  EdgeInsets get menuScrollInsets => _usesRegions
      ? EdgeInsets.zero
      : EdgeInsets.only(
          top: _navigationMedia.padding.top,
          bottom: StyleConstants.navBarHeight + _navigationBottomInset,
        );
  // Null leaves the caller's SafeArea in charge. Zero still claims the sides.
  EdgeInsets? contentInsets(
    MediaQueryData media, {
    required bool maintainViewPadding,
  }) => !_usesRegions
      ? null
      : maintainViewPadding
      ? insetsFor(safeBounds)
      : media.padding;
  double get dashboardContentWidth =>
      _usesRegions ? StyleConstants.pageContentMaxWidth : double.infinity;
  double get dashboardTopPadding => _usesRegions ? 24 : 16;

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
    return _nearest(
      _clearRegions(bounds, [navigationBounds ?? this.navigationBounds]),
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

  /// A card uses its clear pane, with room beside it for the map when both fit.
  /// Width depends on this pane's dimensions, including ordinary windows.
  Rect get panelBounds {
    final available = _inset(_panelRegion, const EdgeInsets.all(16));
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
    final left =
        available.left + (available.width - budget.maxWidth - mapSpace) / 2;
    return Rect.fromLTWH(
      left,
      available.top,
      budget.maxWidth,
      budget.maxHeight,
    );
  }

  /// A browsing card leaves the map visible instead of filling its size limit.
  /// The clear pane's shape determines its reading width, including folds.
  Rect get pickerBounds {
    final available = _inset(_panelRegion, const EdgeInsets.all(16));
    // The keyboard can shorten a portrait pane without rotating its content.
    final wide =
        _media.orientation == Orientation.landscape &&
        _panelRegion.width > _panelRegion.height;
    final budget = contentConstraints(
      available.size,
      maxWidth: wide
          ? StyleConstants.mapPickerWideMaxWidth
          : StyleConstants.mapPickerMaxWidth,
    );
    final preferredHeight = (size.height * 0.62).clamp(
      StyleConstants.mapPickerMinHeight,
      StyleConstants.mapPickerMaxHeight,
    );
    final height = math.min(budget.maxHeight, preferredHeight);
    return Rect.fromLTWH(
      available.center.dx - budget.maxWidth / 2,
      available.bottom - height,
      budget.maxWidth,
      height,
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
    if (_usesRegions) {
      return EdgeInsets.fromLTRB(
        controlRegion.left + 24,
        0,
        size.width - controlRegion.right + 24,
        size.height - controlRegion.bottom + 22,
      );
    }
    final side =
        math.max(_media.viewPadding.left, _media.viewPadding.right) + 24;
    return EdgeInsets.fromLTRB(side, 0, side, primaryControlBottomInset);
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
  MediaQueryData headerMedia(MediaQueryData media) => _usesRegions
      ? media.removePadding(removeLeft: true, removeRight: true)
      : media;
  bool get backPrecedesEdgeGesture => detachedBackBounds == null;

  EdgeInsets get recordingIndicatorMargin => _usesRegions
      ? EdgeInsets.fromLTRB(
          controlRegion.left + 18 - _media.viewPadding.left,
          controlRegion.top + 8 - _media.viewPadding.top,
          0,
          0,
        )
      : const EdgeInsets.fromLTRB(18, 8, 0, 0);
  double recordingLeadingSpace(double totalWidth, double primaryWidth) =>
      _usesRegions ? (totalWidth - primaryWidth) / 2 : 0;

  EdgeInsets get toastInsets {
    final region = _usesRegions
        ? regionFor(safeBounds.topLeft)
        : Offset.zero & _media.size;
    return EdgeInsets.fromLTRB(
      region.left + 16,
      region.top + region.height / 6,
      size.width - region.right + 16,
      0,
    );
  }

  /// Fixed single-viewport metrics are a compatibility policy. Clear-region
  /// controls can grow for text, but must fit the available region.
  BoxConstraints controlConstraints({double? width, double? height}) {
    if (!_usesRegions) {
      return BoxConstraints.tightFor(width: width, height: height);
    }
    return BoxConstraints(
      minWidth: math.min(width ?? 0, controlRegion.width),
      maxWidth: controlRegion.width,
      minHeight: math.min(height ?? 0, controlRegion.height),
      maxHeight: controlRegion.height,
    );
  }

  /// Every map fits against the existing panel's measured footprint.
  EdgeInsets mapFitPaddingForOverlay(Rect occupied) =>
      _mapFitPaddingAround(occupied);

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
    MediaQueryData? navigationMedia,
  }) {
    navigationMedia ??= media;
    final viewport = Offset.zero & size;
    final keyboardInset = _remainingKeyboard(media, size);
    final availableBottom = size.height - keyboardInset;
    final enabled =
        platform == TargetPlatform.iOS &&
        hardware.hasHinge &&
        barEdge != DuoBarEdge.unknown;
    final padding = media.viewPadding;
    final safe = _inset(
      viewport,
      EdgeInsets.fromLTRB(
        padding.left,
        padding.top,
        padding.right,
        media.viewInsets.bottom > 0 ? keyboardInset : padding.bottom,
      ),
    );
    if (!enabled) {
      final bottom = StyleConstants.navBarBottomInsetForMedia(
        navigationMedia,
        platform,
      );
      final naturalWidth = math.max(0.0, media.size.width - 48);
      final availableWidth = math.max(
        0.0,
        size.width - 2 * math.max(padding.left, padding.right),
      );
      final scale = naturalWidth == 0
          ? 1.0
          : math.min(1.0, availableWidth / naturalWidth);
      final navigationSize = Size(
        naturalWidth * scale,
        StyleConstants.navBarHeight * scale,
      );
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
        navigationAxis: Axis.horizontal,
        regions: [safe],
        controlRegion: safe,
        panelRegion: Rect.fromLTRB(
          safe.left,
          safe.top,
          safe.right,
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
    final clear = _clearRegions(band, reserved);
    final fitting = clear
        .where((r) => r.width >= 68 && r.height >= 68)
        .toList();
    final free = _nearest(
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
        ? _clearRegions(primaryBand, reserved)
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
        final width = math.max(0.0, free.width - 48);
        final height = math.min(
          StyleConstants.navBarHeight,
          math.max(0.0, free.height - 24),
        );
        navigation = Rect.fromLTWH(
          free.left + 24,
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
    final regions = _clearRegions(content, reserved);
    final controls = _controlRegionFor(regions, content);
    Rect panelRegion = controls;
    final sideways = folds.any(
      (r) =>
          r.frame.height > r.frame.width &&
          r.frame.height >= content.height * 0.75,
    );
    if (sideways) {
      panelRegion = _nearest(
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
        : _nearest(fitting, content.bottomRight);
  }

  static Rect _inset(Rect rect, EdgeInsets padding) => Rect.fromLTRB(
    math.min(rect.right, rect.left + padding.left),
    math.min(rect.bottom, rect.top + padding.top),
    math.max(
      rect.left + math.min(rect.width, padding.left),
      rect.right - padding.right,
    ),
    math.max(
      rect.top + math.min(rect.height, padding.top),
      rect.bottom - padding.bottom,
    ),
  );

  static List<Rect> _clearRegions(Rect bounds, List<Rect> reserved) {
    var result = <Rect>[bounds];
    for (final obstacle in reserved) {
      result = [for (final rect in result) ..._subtract(rect, obstacle)];
    }
    return result.isEmpty
        ? [Rect.fromLTWH(bounds.left, bounds.top, 0, 0)]
        : result;
  }

  static Iterable<Rect> _subtract(Rect rect, Rect obstacle) sync* {
    if (!rect.overlaps(obstacle)) {
      yield rect;
      return;
    }
    final cut = rect.intersect(obstacle);
    final options = [
      Rect.fromLTRB(rect.left, rect.top, rect.right, cut.top),
      Rect.fromLTRB(rect.left, cut.bottom, rect.right, rect.bottom),
      Rect.fromLTRB(rect.left, cut.top, cut.left, cut.bottom),
      Rect.fromLTRB(cut.right, cut.top, rect.right, cut.bottom),
    ];
    yield* options.where((r) => r.width > 0 && r.height > 0);
  }

  static Rect _nearest(List<Rect> regions, Offset anchor) =>
      regions.reduce((a, b) {
        double distance(Rect r) =>
            (Offset(
                      anchor.dx.clamp(r.left, r.right),
                      anchor.dy.clamp(r.top, r.bottom),
                    ) -
                    anchor)
                .distanceSquared;
        final da = distance(a), db = distance(b);
        return db < da || (db == da && b.width * b.height > a.width * a.height)
            ? b
            : a;
      });
}

class AppLayoutScope extends InheritedWidget {
  const AppLayoutScope({super.key, required this.layout, required super.child});

  final AppLayout layout;

  static AppLayout? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<AppLayoutScope>()?.layout;

  static AppLayout of(BuildContext context) =>
      maybeOf(context) ?? AppLayout.fromContext(context);

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

/// Places modal content inside one clear native region, consuming insets once.
class AppModalRegion extends StatelessWidget {
  const AppModalRegion({
    super.key,
    required this.child,
    this.useSafeArea = true,
  });

  final Widget child;

  /// Keep the caller's safe-area policy in a single viewport. Reserved native
  /// space is always excluded before content reaches the modal.
  final bool useSafeArea;

  @override
  Widget build(BuildContext context) {
    final layout = AppLayout.fromContext(context);
    // Own safe-area consumption so a late native reading can change geometry
    // without leaving a route-created legacy SafeArea around the dialog.
    return SafeArea(
      left: useSafeArea && !layout._usesRegions,
      right: useSafeArea && !layout._usesRegions,
      top: useSafeArea && !layout._usesRegions,
      bottom: useSafeArea && !layout._usesRegions,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final media = MediaQuery.of(context);
          final bounds = layout._usesRegions
              ? layout.controlRegion
              : Offset.zero & constraints.biggest;
          return Padding(
            padding: EdgeInsets.fromLTRB(
              bounds.left,
              bounds.top,
              constraints.maxWidth - bounds.right,
              constraints.maxHeight - bounds.bottom,
            ),
            child: MediaQuery(
              data: layout._usesRegions
                  ? media.copyWith(
                      size: bounds.size,
                      padding: EdgeInsets.zero,
                      viewPadding: EdgeInsets.zero,
                      viewInsets: EdgeInsets.zero,
                    )
                  : media,
              child: child,
            ),
          );
        },
      ),
    );
  }
}
