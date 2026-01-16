import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/body/journey/editor/journey_editor_map_view.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:easy_localization/easy_localization.dart';
import 'package:fluttertoast/fluttertoast.dart';

class JourneyTrackEditPage extends StatefulWidget {
  final String journeyId;

  const JourneyTrackEditPage({super.key, required this.journeyId});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  static const double _minEditZoom = 13.0;

  EditSession? _editSession;
  api.MapRendererProxy? _mapRendererProxy;
  JourneyEditorMapViewCamera? _initialMapView;
  bool _isAddMode = false;
  bool _isDeleteMode = false;
  bool _canUndo = false;
  bool _editingSupported = true;
  bool _popAllowed = false;
  bool _restoreAddModeAfterZoom = false;
  final GlobalKey<JourneyEditorMapViewState> _mapWebviewKey = GlobalKey();
  ScaffoldMessengerState? _snackBarMessenger;
  ScaffoldFeatureController<SnackBar, SnackBarClosedReason>?
      _activeSnackBarController;

  Future<void> _dismissSnackBarsAndWait() async {
    final controller = _activeSnackBarController;
    if (controller != null) {
      controller.close();
      try {
        await controller.closed;
      } catch (_) {
        // Ignore if already closed or disposed.
      }
    }
    _snackBarMessenger?.removeCurrentSnackBar();
    _snackBarMessenger?.clearSnackBars();
    _activeSnackBarController = null;
  }

  void _showAddModeDisabled() {
    _showFloatingSnackBar(
      context.tr("journey.journey_track_edit_add_mode_disabled"),
    );
  }

  void _showAddModeEnabled() {
    _showFloatingSnackBar(
      context.tr("journey.journey_track_edit_add_mode_enabled"),
    );
  }

  Widget _snackBarText(
    String message, {
    TextStyle? style,
    bool allowExplicitNewlines = false,
  }) {
    if (allowExplicitNewlines && message.contains('\n')) {
      final lines = message.split('\n');
      return Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final line in lines)
            FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerLeft,
              child: Text(
                line,
                style: style,
                maxLines: 1,
                softWrap: false,
                overflow: TextOverflow.ellipsis,
              ),
            ),
        ],
      );
    }

    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.centerLeft,
      child: Text(
        message,
        style: style,
        maxLines: 1,
        softWrap: false,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }

  void _showFloatingSnackBar(
    String message, {
    double mapRelativeY = 0.25,
  }) {
    if (!mounted) return;
    EdgeInsets margin = const EdgeInsets.fromLTRB(16, 0, 16, 16);
    final overlayState = Overlay.maybeOf(context);
    final overlayBox = overlayState?.context.findRenderObject() as RenderBox?;
    final mapBox =
        _mapWebviewKey.currentContext?.findRenderObject() as RenderBox?;
    if (overlayBox != null && mapBox != null) {
      final mapTopLeft =
          mapBox.localToGlobal(Offset.zero, ancestor: overlayBox);
      final mapTop = mapTopLeft.dy;
      final mapBottom = mapTop + mapBox.size.height;
      final overlayHeight = overlayBox.size.height;

      final relative = mapRelativeY.clamp(0.0, 1.0).toDouble();

      final topMargin = mapTop + 16;
      final targetBottomY = mapTop + mapBox.size.height * relative;
      double bottomMargin = (overlayHeight - targetBottomY) + 16;

      // Prevent impossible constraints (negative available height) in edge cases.
      final minBottomMargin = (overlayHeight - mapBottom) + 16;
      double maxBottomMargin = overlayHeight - topMargin - 56;
      if (maxBottomMargin < 16) maxBottomMargin = 16;
      bottomMargin =
          bottomMargin.clamp(minBottomMargin, maxBottomMargin).toDouble();

      margin = EdgeInsets.fromLTRB(16, topMargin, 16, bottomMargin);
    }

    final messenger = ScaffoldMessenger.of(context);
    _snackBarMessenger = messenger;
    messenger.removeCurrentSnackBar();
    messenger.clearSnackBars();
    final controller = messenger.showSnackBar(
      SnackBar(
        content: _snackBarText(
          message,
          style: const TextStyle(color: Colors.white),
          allowExplicitNewlines: true,
        ),
        backgroundColor: Colors.black.withValues(alpha: 0.4),
        behavior: SnackBarBehavior.floating,
        margin: margin,
        action: SnackBarAction(
          label: 'OK',
          onPressed: () {
            messenger.hideCurrentSnackBar();
          },
        ),
      ),
    );

    _activeSnackBarController = controller;
    controller.closed.whenComplete(() {
      if (!mounted) return;
      if (identical(_activeSnackBarController, controller)) {
        _activeSnackBarController = null;
      }
    });
  }

  Future<bool> _confirmDiscardUnsavedChanges() async {
    final shouldExit = await showCommonDialog(
      context,
      context.tr("journey.journey_track_edit_discard_changes_confirm"),
      hasCancel: true,
    );
    return shouldExit;
  }

  @override
  void initState() {
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    final session = await EditSession.newInstance(journeyId: widget.journeyId);
    final result = await session.getMapRendererProxy();
    if (mounted) {
      setState(() {
        _editSession = session;
        _mapRendererProxy = result.$1;
        final cameraOption = result.$2;
        if (cameraOption != null) {
          _initialMapView = (
            lng: cameraOption.lng,
            lat: cameraOption.lat,
            zoom: cameraOption.zoom,
          );
        }
        _canUndo = session.canUndo();
        _editingSupported = true;
      });
    }

    // Detect whether the edit session is backed by a vector journey.
    final supported = session.isVector();

    if (!mounted) return;
    if (!supported) {
      setState(() {
        _editingSupported = false;
        _isAddMode = false;
        _isDeleteMode = false;
        _canUndo = false;
      });
      _mapWebviewKey.currentState?.setDrawMode(false);
      _mapWebviewKey.currentState?.setDeleteMode(false);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        _showFloatingSnackBar(
          context.tr("journey.journey_track_edit_bitmap_not_supported"),
          mapRelativeY: 0.4,
        );
      });
    }
  }

  Future<void> _refreshCanUndo() async {
    final session = _editSession;
    if (session == null) return;
    final canUndo = session.canUndo();
    if (!mounted) return;
    setState(() {
      _canUndo = canUndo;
    });
  }

  Future<void> _handleMapMoved() async {
    if (!_isAddMode && !_restoreAddModeAfterZoom) return;
    final zoomTooLowMessage =
        context.tr("journey.journey_track_edit_zoom_too_low");
    final mapView = await _mapWebviewKey.currentState?.getCurrentMapView();
    if (mapView == null) return;
    if (!mounted) return;

    final zoomOk = mapView.zoom >= _minEditZoom;
    if (_isAddMode && !zoomOk) {
      setState(() {
        _isAddMode = false;
        _restoreAddModeAfterZoom = true;
      });
      _mapWebviewKey.currentState?.setDrawMode(false);
      _showFloatingSnackBar(zoomTooLowMessage);
      return;
    }

    if (_restoreAddModeAfterZoom && zoomOk && !_isDeleteMode) {
      _restoreAddModeAfterZoom = false;
      setState(() {
        _isAddMode = true;
      });
      _mapWebviewKey.currentState?.setDrawMode(true);
      _showAddModeEnabled();
    }
  }

  Future<void> _onDrawPath(List<JourneyEditorDrawPoint> points) async {
    if (!_editingSupported) return;
    if (!_isAddMode) return;
    if (points.length < 2) return;

    // Downsample and limit the number of points to avoid too many segments.
    final filteredPoints = _limitPointCount(
      _downsampleDrawPoints(points),
      maxPoints: 50,
    );
    if (filteredPoints.length < 2) return;

    final session = _editSession;
    if (session == null) return;

    await session.pushUndoCheckpoint();

    // Approximate the freehand path by adding many small straight segments.
    api.MapRendererProxy? latestProxy = _mapRendererProxy;
    for (var i = 0; i < filteredPoints.length - 1; i++) {
      final a = filteredPoints[i];
      final b = filteredPoints[i + 1];
      final result = await session.addLine(
        startLat: a.lat,
        startLng: a.lng,
        endLat: b.lat,
        endLng: b.lng,
      );
      latestProxy = result.$1;
    }

    if (!mounted) return;
    if (latestProxy != null) {
      setState(() {
        _mapRendererProxy = latestProxy;
      });
    }

    await _refreshCanUndo();
  }

  List<JourneyEditorDrawPoint> _limitPointCount(
    List<JourneyEditorDrawPoint> points, {
    required int maxPoints,
  }) {
    if (points.length <= maxPoints) return points;
    if (maxPoints < 2) return [points.first, points.last];

    final stride = (points.length - 1) / (maxPoints - 1);
    final result = <JourneyEditorDrawPoint>[];
    for (var i = 0; i < maxPoints; i++) {
      final index = (i * stride).round();
      result.add(points[index]);
    }

    // Ensure last point is exact.
    if (result.last != points.last) {
      result[result.length - 1] = points.last;
    }

    return result;
  }

  List<JourneyEditorDrawPoint> _downsampleDrawPoints(
    List<JourneyEditorDrawPoint> points,
  ) {
    if (points.length <= 2) return points;

    const minDistanceMeters = 3.0;
    final result = <JourneyEditorDrawPoint>[points.first];
    var last = points.first;

    for (var i = 1; i < points.length - 1; i++) {
      final current = points[i];
      if (_approxDistanceMeters(last, current) >= minDistanceMeters) {
        result.add(current);
        last = current;
      }
    }

    // Always keep the last point to preserve the path end.
    result.add(points.last);
    return result;
  }

  double _approxDistanceMeters(
    JourneyEditorDrawPoint a,
    JourneyEditorDrawPoint b,
  ) {
    const metersPerDeg = 111320.0;
    final latRad = (a.lat + b.lat) * 0.5 * (3.141592653589793 / 180.0);
    final cosLat = math.cos(latRad);
    final dx = (a.lng - b.lng) * metersPerDeg * cosLat;
    final dy = (a.lat - b.lat) * metersPerDeg;
    return math.sqrt(dx * dx + dy * dy);
  }

  Future<void> _onSelectionBox(
    double startLat,
    double startLng,
    double endLat,
    double endLng,
  ) async {
    if (!_editingSupported) return;
    if (!_isDeleteMode) return;

    final session = _editSession;
    if (session == null) return;

    await session.pushUndoCheckpoint();

    final result = await session.deletePointsInBox(
      startLat: startLat,
      startLng: startLng,
      endLat: endLat,
      endLng: endLng,
    );

    if (!mounted) return;
    setState(() {
      _mapRendererProxy = result.$1;
    });

    await _refreshCanUndo();
  }

  @override
  void dispose() {
    // If user manually pops this page while a SnackBar is visible, ensure the
    // SnackBar is dismissed and doesn't remain on the previous page.
    _dismissSnackBarsAndWait();
    _editSession?.discard();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      // Always intercept pop to dismiss SnackBar before exit.
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;
        final navigator = Navigator.of(context);
        final needConfirm = _canUndo && !_popAllowed;
        if (needConfirm) {
          final shouldExit = await _confirmDiscardUnsavedChanges();
          if (!mounted) return;
          if (!shouldExit) return;

          setState(() {
            _popAllowed = true;
          });
        }

        await _dismissSnackBarsAndWait();
        if (!mounted) return;
        navigator.pop(result);
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_track_edit_title")),
        ),
        body: SafeAreaWrapper(
          child: Stack(
            children: [
              if (_mapRendererProxy != null)
                JourneyEditorMapView(
                  key: _mapWebviewKey,
                  mapRendererProxy: _mapRendererProxy!,
                  initialMapView: _initialMapView,
                  onSelectionBox: _onSelectionBox,
                  onDrawPath: _onDrawPath,
                  onMapMoved: _handleMapMoved,
                )
              else
                const Center(child: CircularProgressIndicator()),
              Positioned(
                bottom: 16,
                left: 0,
                right: 0,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    if (_canUndo)
                      FloatingActionButton(
                        heroTag: "undo_track_edit",
                        backgroundColor: _editingSupported
                            ? const Color(0xFFFFFFFF)
                            : Colors.grey,
                        foregroundColor:
                            _editingSupported ? Colors.black : Colors.white,
                        onPressed: !_editingSupported
                            ? null
                            : () async {
                                final session = _editSession;
                                if (session == null) return;
                                final result = await session.undo();
                                if (!mounted) return;
                                setState(() {
                                  _mapRendererProxy = result.$1;
                                });
                                await _refreshCanUndo();
                              },
                        child: const Icon(Icons.undo),
                      )
                    else
                      const SizedBox(width: 56, height: 56),
                    const SizedBox(width: 32),
                    FloatingActionButton(
                      heroTag: "add_track",
                      backgroundColor: !_editingSupported
                          ? Colors.grey
                          : (_restoreAddModeAfterZoom && !_isAddMode)
                              ? const Color(0xFFFFD54F)
                              : (_isAddMode
                                  ? const Color(0xFFB6E13D)
                                  : const Color(0xFFFFFFFF)),
                      foregroundColor: !_editingSupported
                          ? Colors.white
                          : (_isAddMode ? Colors.white : Colors.black),
                      onPressed: !_editingSupported
                          ? null
                          : () async {
                              if (_isAddMode) {
                                setState(() {
                                  _isAddMode = false;
                                  _restoreAddModeAfterZoom = false;
                                });
                                _mapWebviewKey.currentState?.setDrawMode(false);
                                _showAddModeDisabled();
                                return;
                              }

                              if (_restoreAddModeAfterZoom) {
                                setState(() {
                                  _restoreAddModeAfterZoom = false;
                                });
                                _showAddModeDisabled();
                                return;
                              }

                              if (_isDeleteMode) {
                                setState(() {
                                  _isDeleteMode = false;
                                });
                                _mapWebviewKey.currentState
                                    ?.setDeleteMode(false);
                              }

                              final zoomTooLowMessage = context.tr(
                                "journey.journey_track_edit_zoom_too_low",
                              );
                              final addEnabledMessage = context.tr(
                                "journey.journey_track_edit_add_mode_enabled",
                              );

                              final mapView = await _mapWebviewKey.currentState
                                  ?.getCurrentMapView();
                              if (!mounted) return;
                              if (mapView == null) return;

                              if (mapView.zoom < _minEditZoom) {
                                setState(() {
                                  _restoreAddModeAfterZoom = true;
                                });
                                _showFloatingSnackBar(
                                  zoomTooLowMessage,
                                );
                                return;
                              }

                              setState(() {
                                _isAddMode = true;
                              });
                              _mapWebviewKey.currentState?.setDrawMode(true);
                              _showFloatingSnackBar(addEnabledMessage);
                            },
                      child: const Icon(Icons.edit),
                    ),
                    const SizedBox(width: 32),
                    FloatingActionButton(
                      heroTag: "delete_track",
                      backgroundColor: !_editingSupported
                          ? Colors.grey
                          : (_isDeleteMode
                              ? const Color(0xFFE13D3D)
                              : const Color(0xFFFFFFFF)),
                      foregroundColor: !_editingSupported
                          ? Colors.white
                          : (_isDeleteMode ? Colors.white : Colors.black),
                      onPressed: !_editingSupported
                          ? null
                          : () {
                              setState(() {
                                _isDeleteMode = !_isDeleteMode;
                                if (_isDeleteMode) {
                                  _isAddMode = false;
                                  _restoreAddModeAfterZoom = false;
                                }
                              });
                              _mapWebviewKey.currentState?.setDrawMode(false);
                              _mapWebviewKey.currentState
                                  ?.setDeleteMode(_isDeleteMode);
                              _showFloatingSnackBar(
                                _isDeleteMode
                                    ? context.tr(
                                        "journey.journey_track_edit_delete_mode_enabled",
                                      )
                                    : context.tr(
                                        "journey.journey_track_edit_delete_mode_disabled",
                                      ),
                              );
                            },
                      child: const Icon(Icons.delete),
                    ),
                    const SizedBox(width: 32),
                    FloatingActionButton(
                      heroTag: "save_track",
                      backgroundColor: _editingSupported
                          ? const Color(0xFFFFFFFF)
                          : Colors.grey,
                      foregroundColor:
                          _editingSupported ? Colors.black : Colors.white,
                      onPressed: !_editingSupported
                          ? null
                          : () async {
                              final saveMessage =
                                  context.tr("common.save_success");
                              final navigator = Navigator.of(context);
                              final session = _editSession;
                              if (session == null) return;
                              await session.commit();
                              if (!mounted) return;
                              await _dismissSnackBarsAndWait();
                              Fluttertoast.showToast(
                                msg: saveMessage,
                              );
                              setState(() {
                                _popAllowed = true;
                              });
                              navigator.pop(true);
                            },
                      child: const Icon(Icons.save),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
