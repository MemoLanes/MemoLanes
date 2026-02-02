import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/body/journey/editor/journey_editor_map_view.dart';
import 'package:memolanes/body/journey/editor/journey_track_edit_mode_bar.dart';
import 'package:memolanes/body/journey/editor/top_persistent_toast.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;

class JourneyTrackEditPage extends StatefulWidget {
  final String journeyId;

  const JourneyTrackEditPage({super.key, required this.journeyId});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  static const int _minEditZoom = 13;

  late EditSession _editSession;
  api.MapRendererProxy? _mapRendererProxy;
  JourneyEditorMapViewCamera? _initialMapView;

  OperationMode _mode = OperationMode.move;
  bool _canUndo = false;

  bool _zoomOk = false;

  final GlobalKey<JourneyEditorMapViewState> _mapWebviewKey = GlobalKey();

  void _showAddModeEnabled() {
    _showFloatingSnackBar(
      context.tr("journey.editor.draw_mode_enabled"),
    );
  }

  void _showDeleteModeEnabled() {
    _showFloatingSnackBar(
      context.tr("journey.editor.erase_mode_enabled"),
    );
  }

  void _showZoomTooLow() {
    _showFloatingSnackBar(
      context.tr("journey.editor.zoom_too_low"),
    );
  }

  void _showFloatingSnackBar(String message) {
    if (!mounted) return;
    TopPersistentToast().show(context, message);
  }

  void _removeToast() {
    TopPersistentToast().hide();
  }

  Future<bool> _confirmDiscardUnsavedChanges() async {
    _removeToast();
    final shouldExit = await showCommonDialog(
      context,
      context.tr("journey.editor.discard_changes_confirm"),
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
    try {
      EditSession? session =
          await EditSession.newInstance(journeyId: widget.journeyId);
      if (session == null) {
        log.error("[JourneyTrackEditPage] EditSession is null");
        return;
      }

      final result = await session.getMapRendererProxy();
      if (!mounted) return;
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
      });
    } catch (e) {
      log.error("[JourneyTrackEditPage] Load map error: $e");
      if (!mounted) return;

      _mapWebviewKey.currentState?.setDrawMode(false);
      _mapWebviewKey.currentState?.setDeleteMode(false);

      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        _showFloatingSnackBar(
            context.tr("journey.editor.bitmap_not_supported"));
        Navigator.of(context).maybePop();
      });
    }
  }

  Future<void> _refreshCanUndo() async {
    final session = _editSession;
    final canUndo = session.canUndo();
    if (!mounted) return;
    setState(() {
      _canUndo = canUndo;
    });
  }

  void _applyMode(OperationMode next) {
    if (!mounted) return;

    if (next == OperationMode.edit && !_zoomOk) {
      next = OperationMode.editReadonly;
    } else if (next == OperationMode.editReadonly && _zoomOk) {
      next = OperationMode.edit;
    }

    if (next == _mode) return;

    setState(() {
      _mode = next;
    });

    final map = _mapWebviewKey.currentState;

    switch (next) {
      case OperationMode.move:
        _removeToast();
        map?.setDrawMode(false);
        map?.setDeleteMode(false);
        break;

      case OperationMode.edit:
        _showAddModeEnabled();
        map?.setDrawMode(true);
        map?.setDeleteMode(false);
        break;

      case OperationMode.editReadonly:
        _showZoomTooLow();
        map?.setDrawMode(false);
        map?.setDeleteMode(false);
        break;

      case OperationMode.delete:
        _showDeleteModeEnabled();
        map?.setDrawMode(false);
        map?.setDeleteMode(true);
        break;
    }
  }

  void _handleModeChange(OperationMode mode) {
    _applyMode(mode);
  }

  void _handleMapZoomUpdate(int? zoom) {
    if (zoom == null) return;

    final nextZoomOk = zoom >= _minEditZoom;
    if (nextZoomOk == _zoomOk) return;

    setState(() {
      _zoomOk = nextZoomOk;
    });

    _applyMode(_mode);
  }

  Future<void> _onDrawPath(List<JourneyEditorDrawPoint> points) async {
    if (_mode != OperationMode.edit) return;
    if (points.length < 2) return;

    final session = _editSession;

    final recordPoints = points.map((p) => (p.lat, p.lng)).toList();

    await session.addLines(points: recordPoints);

    if (!mounted) return;

    await _mapWebviewKey.currentState?.manualRefresh();

    _refreshCanUndo();
  }

  Future<void> _onSelectionBox(
    double startLat,
    double startLng,
    double endLat,
    double endLng,
  ) async {
    if (_mode != OperationMode.delete) return;

    final session = _editSession;

    await session.deletePointsInBox(
      startLat: startLat,
      startLng: startLng,
      endLat: endLat,
      endLng: endLng,
    );

    if (!mounted) return;
    await _mapWebviewKey.currentState?.manualRefresh();

    _refreshCanUndo();
  }

  @override
  void dispose() {
    // If user manually pops this page while a SnackBar is visible, ensure the
    // SnackBar is dismissed and doesn't remain on the previous page.
    _removeToast();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;

        if (_canUndo) {
          final shouldExit = await _confirmDiscardUnsavedChanges();
          if (!mounted || !shouldExit) return;
        }
        _removeToast();
        if (!mounted) return;

        Navigator.of(context).pop(result);
      },
      child: Scaffold(
        body: Stack(
          children: [
            if (_mapRendererProxy != null)
              JourneyEditorMapView(
                key: _mapWebviewKey,
                mapRendererProxy: _mapRendererProxy!,
                initialMapView: _initialMapView,
                onSelectionBox: _onSelectionBox,
                onDrawPath: _onDrawPath,
                onMapZoomChanged: _handleMapZoomUpdate,
              )
            else
              const Center(child: CircularProgressIndicator()),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: SafeArea(
                minimum: const EdgeInsets.all(16),
                child: ModeSwitchBar(
                  currentMode: _mode,
                  onModeChanged: _handleModeChange,
                  canUndo: _canUndo,
                  onUndo: () async {
                    final session = _editSession;
                    await session.undo();
                    if (!mounted) return;
                    await _mapWebviewKey.currentState?.manualRefresh();
                    _refreshCanUndo();
                  },
                  canSave: _canUndo,
                  onSave: () async {
                    final session = _editSession;

                    if (!_canUndo) {
                      Navigator.of(context).pop(false);
                      return;
                    }
                    _removeToast();

                    final shouldSave = await showCommonDialog(
                      context,
                      context.tr("common.save_confirm"),
                      title: context.tr("common.save"),
                      hasCancel: true,
                    );
                    if (!mounted || !shouldSave) return;

                    await showLoadingDialog(
                      context: context,
                      asyncTask: session.commit(),
                    );
                    if (!mounted) return;
                    _removeToast();
                    Fluttertoast.showToast(
                      msg: context.tr("common.save_success"),
                    );

                    Navigator.of(context).pop(true);
                  },
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
