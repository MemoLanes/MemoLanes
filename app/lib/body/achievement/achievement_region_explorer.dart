import 'dart:math' as math;

import 'package:flutter/rendering.dart' show OverflowBoxFit;

import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/body/achievement/achievement_region_explorer_controller.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/layout_geometry.dart' as geometry;
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart' show GeoEntityId;
import 'package:memolanes/theme/app_colors.dart';

const _panelHandleHeight = 56.0;
const _expandedGrabberTargetHeight = 48.0;
const _grabberStripHeight = 18.0;

typedef AchievementRegionMapState = ({
  String? featureGeojson,
  MapBounds? flyToBounds,
  MapView? flyToView,
  EdgeInsets boundsPadding,
  int cameraRevision,
});

class AchievementRegionExplorer extends StatefulWidget {
  const AchievementRegionExplorer({
    super.key,
    this.createController,
    required this.mapBuilder,
    this.controlsBuilder,
    required this.onClose,
  });

  final VoidCallback onClose;
  final Widget Function(BuildContext, Rect)? controlsBuilder;
  final AchievementRegionExplorerController Function()? createController;
  final Widget Function(BuildContext, AchievementRegionMapState) mapBuilder;

  @override
  State<AchievementRegionExplorer> createState() =>
      AchievementRegionExplorerState();
}

class AchievementRegionExplorerState extends State<AchievementRegionExplorer> {
  // Match the upstream map's unpositioned camera.
  static const MapView _initialView = (lng: 0, lat: 0, zoom: 2);
  // The sheet owns the only vertical scroll position, also in the wide pane.
  ScrollController? _listScroll;
  ScrollController get _scroll => _listScroll!;
  final _sheet = DraggableScrollableController();
  double _fraction = .55;
  bool _collapsed = false;
  ({bool beside, double minimum, double maximum, double initial})?
  _panelGeometry;
  List<double> _snapSizes = const [];
  double? _snapInitial;
  int _restorationRevision = 0;
  double? _pendingScrollOffset;
  double? _pendingSheetExtent;
  bool _restorationScheduled = false;
  bool _adjustingGeometry = false;
  final _parents = <GeoEntityId, RegionEntity>{};
  late final AchievementRegionExplorerController _controller;
  late final int _openedCameraRevision;
  late int _previousInitialCameraRevision;
  GeoEntityId? _previousParent;

  @override
  void initState() {
    super.initState();
    _controller =
        widget.createController?.call() ??
        AchievementRegionExplorerController(
          worldviewManager: WorldviewManager.instance,
        );
    _controller.openRoot();
    _openedCameraRevision = _controller.initialCameraRevision;
    _previousInitialCameraRevision = _openedCameraRevision;
    _controller.addListener(_onStateChanged);
  }

  @override
  void dispose() {
    _controller.removeListener(_onStateChanged);
    _controller.dispose();
    _sheet.dispose();
    super.dispose();
  }

  void _onStateChanged() {
    final parent = _controller.parent;
    final initialRevision = _controller.initialCameraRevision;
    if (parent != _previousParent ||
        initialRevision != _previousInitialCameraRevision) {
      _previousParent = parent;
      _previousInitialCameraRevision = initialRevision;
      if (parent == null) _parents.clear();
      // Navigation owns the target even if a geometry restoration is pending.
      _pendingScrollOffset = _controller.restoredScrollOffset ?? 0;
      _restorationRevision++;
      _scheduleRestoration();
    }
    setState(() {});
  }

  // One transaction spans all layout changes until the final viewport is ready.
  // Geometry keeps the first offset; navigation replaces it with its own target.
  void _scheduleRestoration() {
    if (_restorationScheduled) return;
    _restorationScheduled = true;
    final revision = _restorationRevision;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _restorationScheduled = false;
      if (!mounted) return;
      if (revision != _restorationRevision) {
        _scheduleRestoration();
        return;
      }
      final extent = _pendingSheetExtent;
      if (extent != null && _sheet.isAttached) {
        _pendingSheetExtent = null;
        _sheet.jumpTo(extent);
        // Offset clamping must use the viewport after this extent lays out.
        _scheduleRestoration();
        return;
      }
      if (_listScroll != null && _scroll.hasClients) {
        final offset = _pendingScrollOffset;
        if (offset != null) {
          _scroll.jumpTo(offset.clamp(0, _scroll.position.maxScrollExtent));
        }
      }
      _pendingScrollOffset = null;
      if (_adjustingGeometry) {
        setState(() {
          _adjustingGeometry = false;
        });
      }
    });
    WidgetsBinding.instance.scheduleFrame();
  }

  void _drillInto(GeoEntityId id, RegionEntity entity) {
    _parents[id] = entity;
    _controller.drillInto(
      id,
      _pendingScrollOffset ??
          (_listScroll != null && _scroll.hasClients ? _scroll.offset : 0),
    );
  }

  void requestBack() {
    if (_controller.canGoBack) {
      _controller.back();
    } else {
      widget.onClose();
    }
  }

  @override
  Widget build(BuildContext context) {
    final bottomInset = MediaQuery.paddingOf(context).bottom;
    final worldview = _controller.worldviewManager.currentWorldview.id;
    final parent = _parents[_controller.parent];
    final selection = _controller.selectedId;
    final selected =
        _controller.rows[selection] ??
        (selection == _controller.parent ? parent : null);
    final title =
        parent?.displayName(worldview) ??
        context.tr('achievement.region_list.country_title');
    final appBar = CapsuleStyleAppBar(title: title, onBack: requestBack);
    return _ExplorerLayout(
      appBar: PreferredSize(
        preferredSize: appBar.preferredSize,
        // Keep the single-line title inside the incumbent fixed-height bar.
        // The list and map notice still honor the full system text scale.
        child: MediaQuery.withClampedTextScaling(
          maxScaleFactor: 2,
          child: appBar,
        ),
      ),
      body: SafeArea(
        top: false,
        child: AppLayoutBuilder(
          builder: (context, layout) {
            final area = layout.controlRegion;
            final beside =
                area.width >=
                    StyleConstants.contentMinWidth +
                        StyleConstants.minimumMapExtent &&
                (area.width > area.height || area.width >= 640);
            final height = math.max(1.0, area.height);
            final handleMinimum = math.min(1.0, _panelHandleHeight / height);
            // On short stacked windows the map minimum is a soft target:
            // reserve real globe space but leave scrollable room below the handle.
            final constrained =
                height <
                _panelHandleHeight * 2 + StyleConstants.minimumMapExtent;
            final mapReserve = constrained
                ? math.max(
                    height * .25,
                    math.min(
                      StyleConstants.mapControlSize,
                      math.max(0.0, height - _panelHandleHeight * 2),
                    ),
                  )
                : StyleConstants.minimumMapExtent;
            final maximum = ((height - mapReserve) / height).clamp(
              handleMinimum,
              1.0,
            );
            final initial = constrained
                ? maximum
                : .55.clamp(handleMinimum, maximum);
            var collapsedHeight = _panelHandleHeight;
            AchievementRegionAreaSummary? summary;
            if (!beside && selection != null && selected != null) {
              final summaryHeight = AchievementRegionAreaSummary.heightFor(
                MediaQuery.textScalerOf(context),
              );
              final previewHeight = _grabberStripHeight + summaryHeight;
              // Keep a distinct expanded stop even on short viewports.
              if (initial * height - previewHeight >= _panelHandleHeight) {
                collapsedHeight = previewHeight;
                if (_collapsed) {
                  summary = AchievementRegionAreaSummary(
                    item: _regionItem(selection, selected, worldview),
                  );
                }
              }
            }
            final minimum = math.min(1.0, collapsedHeight / height);
            final snapInitial = initial > minimum && initial < maximum
                ? initial
                : null;
            if (_snapInitial != snapInitial) {
              _snapInitial = snapInitial;
              _snapSizes = [?snapInitial];
            }
            final nextGeometry = (
              beside: beside,
              minimum: minimum,
              maximum: maximum,
              initial: initial,
            );
            if (_panelGeometry != nextGeometry) {
              final previous = _panelGeometry;
              _panelGeometry = nextGeometry;
              _pendingScrollOffset ??= _listScroll != null && _scroll.hasClients
                  ? _scroll.offset
                  : 0.0;
              final target = beside
                  ? 1.0
                  : _collapsed
                  ? minimum
                  : previous?.beside != beside || _fraction <= minimum
                  ? initial
                  : math.min(_fraction, maximum);
              if (!beside) _fraction = target;
              _pendingSheetExtent = target;
              _restorationRevision++;
              _adjustingGeometry = true;
              _scheduleRestoration();
            }
            final fraction = (_collapsed ? minimum : _fraction).clamp(
              minimum,
              maximum,
            );
            final width = math.min(
              StyleConstants.pageContentMaxWidth,
              math.max(StyleConstants.contentMinWidth, area.width * .46),
            );
            final occupied = beside
                ? Rect.fromLTRB(
                    area.right - (_collapsed ? _panelHandleHeight : width),
                    area.top,
                    area.right,
                    area.bottom,
                  )
                : Rect.fromLTRB(
                    area.left,
                    area.bottom - area.height * fraction,
                    area.right,
                    area.bottom,
                  );
            void toggle() {
              setState(() {
                _collapsed = !_collapsed;
                if (!beside) {
                  _fraction = _collapsed ? minimum : initial;
                  _sheet.jumpTo(_fraction);
                }
              });
            }

            final tooltip = context.tr(
              'achievement.region_explorer.${_collapsed ? 'expand' : 'collapse'}',
            );
            final handle = SizedBox(
              key: const ValueKey('achievement-region-drag-handle'),
              width: 34,
              height: 4,
              child: DecoratedBox(
                decoration: ShapeDecoration(
                  color: context.appColors.mutedInkColor,
                  shape: const StadiumBorder(),
                ),
              ),
            );
            final headerHeight = summary != null
                ? collapsedHeight
                : !beside && !_collapsed
                ? _expandedGrabberTargetHeight
                : _panelHandleHeight;
            final header = beside
                ? Center(
                    child: IconButton(
                      tooltip: tooltip,
                      onPressed: toggle,
                      icon: Icon(
                        _collapsed ? Icons.chevron_left : Icons.chevron_right,
                      ),
                    ),
                  )
                : Tooltip(
                    message: tooltip,
                    child: Semantics(
                      button: true,
                      child: Material(
                        type: MaterialType.transparency,
                        child: InkWell(
                          onTap: toggle,
                          child: Column(
                            children: [
                              SizedBox(
                                height: _grabberStripHeight,
                                child: Center(child: handle),
                              ),
                              if (summary != null) Expanded(child: summary),
                            ],
                          ),
                        ),
                      ),
                    ),
                  );
            return Stack(
              clipBehavior: Clip.none,
              children: [
                Positioned.fill(child: _buildMap(context, layout, occupied)),
                // Continue only the surface past SafeArea, not usable content.
                // A pane ending above the viewport (e.g. a fold) stays clear.
                if (bottomInset > 0 && occupied.bottom == layout.size.height)
                  Positioned.fromRect(
                    rect: Rect.fromLTWH(
                      occupied.left,
                      occupied.bottom,
                      occupied.width,
                      bottomInset,
                    ),
                    child: IgnorePointer(
                      child: ColoredBox(color: context.appColors.canvasColor),
                    ),
                  ),
                if (widget.controlsBuilder != null)
                  Positioned.fill(
                    child: widget.controlsBuilder!(
                      context,
                      [
                        for (final region in layout.regions)
                          ...geometry.subtractRect(region, occupied),
                      ].fold<Rect>(
                        Rect.zero,
                        (a, b) =>
                            b.width * b.height > a.width * a.height ? b : a,
                      ),
                    ),
                  ),
                Positioned.fromRect(
                  rect: beside ? occupied : area,
                  child: NotificationListener<DraggableScrollableNotification>(
                    onNotification: (notification) {
                      if (!_adjustingGeometry &&
                          !beside &&
                          (_fraction - notification.extent).abs() > .0001) {
                        setState(() {
                          _fraction = notification.extent;
                          _collapsed = notification.extent <= minimum + .001;
                        });
                      }
                      return false;
                    },
                    child: DraggableScrollableSheet(
                      controller: _sheet,
                      initialChildSize: beside ? 1 : initial,
                      minChildSize: beside ? 1 : minimum,
                      maxChildSize: beside ? 1 : maximum,
                      snap: true,
                      snapSizes: beside ? null : _snapSizes,
                      shouldCloseOnMinExtent: false,
                      builder: (context, scroll) {
                        _listScroll = scroll;
                        return Semantics(
                          container: true,
                          explicitChildNodes: true,
                          label: context.tr(
                            'achievement.region_explorer.panel',
                          ),
                          child: PointerInterceptor(
                            child: ClipRect(
                              child: ColoredBox(
                                color: context.appColors.canvasColor,
                                child: CustomScrollView(
                                  key: const ValueKey(
                                    'achievement region panel',
                                  ),
                                  controller: scroll,
                                  slivers: [
                                    SliverPersistentHeader(
                                      pinned: true,
                                      delegate: _PanelHandle(
                                        child: header,
                                        height: headerHeight,
                                      ),
                                    ),
                                    SliverToBoxAdapter(
                                      child: ExcludeSemantics(
                                        excluding: _collapsed,
                                        child: IgnorePointer(
                                          ignoring: _collapsed,
                                          child: OverflowBox(
                                            fit: OverflowBoxFit.deferToChild,
                                            alignment: Alignment.topLeft,
                                            minWidth: beside ? width : null,
                                            maxWidth: beside ? width : null,
                                            child: Opacity(
                                              opacity: beside && _collapsed
                                                  ? 0
                                                  : 1,
                                              child: _buildList(
                                                context,
                                                worldview,
                                                topPadding: beside ? 12 : 0,
                                              ),
                                            ),
                                          ),
                                        ),
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        );
                      },
                    ),
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  Widget _buildMap(BuildContext context, AppLayout layout, Rect occupied) {
    final outline = _controller.outline;
    final map = (
      featureGeojson: outline?.featureGeojson,
      flyToBounds: outline?.bounds,
      flyToView:
          _controller.selectedId == null &&
              _controller.initialCameraRevision != _openedCameraRevision
          ? _initialView
          : null,
      boundsPadding: layout.mapFitPaddingForOverlay(occupied),
      cameraRevision:
          _controller.cameraRevision + _controller.initialCameraRevision,
    );
    return widget.mapBuilder(context, map);
  }

  AchievementRegionAreaListItem _regionItem(
    GeoEntityId id,
    RegionEntity entity,
    String worldview,
  ) => AchievementRegionAreaListItem.fromEntity(
    entity,
    worldview,
    showFlag: entity.kind == RegionKind.admin0,
    onTap: () => _controller.select(id),
    onDrill: entity.hasChildren ? () => _drillInto(id, entity) : null,
    selected: id == _controller.selectedId,
  );

  Widget _buildList(
    BuildContext context,
    String worldview, {
    required double topPadding,
  }) {
    final root = _controller.parent == null;
    final selection = _controller.selectedId;
    final missingOutline =
        selection != null &&
        _controller.outline == null &&
        !_controller.isGeometryLoading;
    return Column(
      children: [
        if (missingOutline)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
            child: _OutlineNotice(
              onRetry: _controller.geometryError == null
                  ? null
                  : _controller.retrySelectedGeometry,
            ),
          ),
        Padding(
          padding: EdgeInsets.fromLTRB(16, topPadding, 16, 24),
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(
                maxWidth: StyleConstants.pageContentMaxWidth,
              ),
              child: AchievementRegionAreaListContent(
                items: [
                  for (final MapEntry(:key, :value) in _controller.rows.entries)
                    _regionItem(key, value, worldview),
                ],
                emptyText: context.tr(
                  root
                      ? 'achievement.countries.empty'
                      : _controller.hasRegionStats
                      ? 'achievement.region_list.region_empty'
                      : 'achievement.region_list.region_unsupported',
                ),
                showIcons: root,
                skeletonShowsChevron: root,
                isLoading: _controller.isLoading,
                onRetry: _controller.listError == null
                    ? null
                    : _controller.retryList,
                selectOnMap: true,
                sortMode: _controller.sortMode,
                onSortChanged: _controller.changeSort,
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _ExplorerLayout extends StatelessWidget {
  const _ExplorerLayout({required this.appBar, required this.body});

  final Widget appBar;
  final Widget body;

  @override
  Widget build(BuildContext context) {
    // A Scaffold's canvas Material absorbs hits even with a transparent color.
    // Here the map is a sibling below us; only visible chrome should intercept.
    final media = MediaQuery.of(context);
    return Material(
      type: MaterialType.transparency,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          MediaQuery(
            data: media.removePadding(removeBottom: true),
            child: PointerInterceptor(child: appBar),
          ),
          Expanded(
            child: Padding(
              padding: EdgeInsets.only(bottom: media.viewInsets.bottom),
              child: MediaQuery(
                data: media
                    .removePadding(removeTop: true)
                    .removeViewInsets(removeBottom: true),
                child: body,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _PanelHandle extends SliverPersistentHeaderDelegate {
  _PanelHandle({required this.child, required this.height});

  final Widget child;
  final double height;

  @override
  double get minExtent => height;
  @override
  double get maxExtent => height;
  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) => ColoredBox(color: context.appColors.canvasColor, child: child);
  @override
  bool shouldRebuild(_PanelHandle oldDelegate) => true;
}

class _OutlineNotice extends StatelessWidget {
  const _OutlineNotice({this.onRetry});

  final VoidCallback? onRetry;

  @override
  Widget build(BuildContext context) => Semantics(
    liveRegion: true,
    child: Wrap(
      alignment: WrapAlignment.center,
      crossAxisAlignment: WrapCrossAlignment.center,
      spacing: 8,
      children: [
        Text(
          context.tr(
            onRetry == null
                ? 'achievement.region_explorer.unavailable'
                : 'achievement.region_explorer.error',
          ),
        ),
        if (onRetry != null)
          TextButton(
            onPressed: onRetry,
            child: Text(context.tr('achievement.region_list.retry')),
          ),
      ],
    ),
  );
}
