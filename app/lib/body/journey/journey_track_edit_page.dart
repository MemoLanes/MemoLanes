import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/body/journey/editor/journey_editor_map_view.dart';
import 'package:memolanes/body/journey/editor/journey_track_edit_mode_bar.dart';
import 'package:memolanes/body/journey/editor/top_persistent_toast.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart'
    show AddLinesOutcome, EditSession;
import 'package:pointer_interceptor/pointer_interceptor.dart';

class JourneyTrackEditPage extends StatefulWidget {
  final EditSession editSession;

  const JourneyTrackEditPage({super.key, required this.editSession});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

enum _EditorToastRequest { syncCurrentState, saveSuccess, clear }

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  static const int _minEditZoom = 13;

  late final EditSession _editSession;
  api.MapRendererProxy? _mapRendererProxy;
  JourneyEditorMapBounds? _initialMapBounds;

  OperationMode _mode = OperationMode.move;
  bool _canUndo = false;
  bool _isLinkedDrawEnabled = false;
  bool _isDrawModeMenuOpen = false;
  bool _operationInProgress = false;
  String? _linkedDrawErrorTrKey;

  bool _zoomOk = false;

  final GlobalKey<JourneyEditorMapViewState> _mapWebviewKey = GlobalKey();

  String? _currentPersistentToastMessage() {
    if (_linkedDrawErrorTrKey != null) {
      return context.tr(_linkedDrawErrorTrKey!);
    }

    switch (_mode) {
      case OperationMode.move:
        return null;
      case OperationMode.edit:
        return context.tr(
          _isLinkedDrawEnabled
              ? "journey.editor.linked_draw_mode_enabled"
              : "journey.editor.free_draw_mode_enabled",
        );
      case OperationMode.editReadonly:
        return context.tr("journey.editor.zoom_too_low");
      case OperationMode.delete:
        return context.tr("journey.editor.erase_mode_enabled");
    }
  }

  void _clearLinkedDrawConstraintError() {
    if (!mounted || _linkedDrawErrorTrKey == null) return;
    setState(() {
      _linkedDrawErrorTrKey = null;
    });
  }

  void _showLinkedDrawConstraintToast(String trKey) {
    if (!mounted) return;
    final message = context.tr(trKey);
    setState(() {
      _linkedDrawErrorTrKey = trKey;
    });
    TopPersistentToast().show(context, message);
  }

  void _showToast(
    _EditorToastRequest request, {
    bool clearLinkedDrawError = false,
  }) {
    if (!mounted) return;

    if (clearLinkedDrawError) {
      _clearLinkedDrawConstraintError();
    }

    switch (request) {
      case _EditorToastRequest.syncCurrentState:
        final message = _currentPersistentToastMessage();
        if (message == null) {
          TopPersistentToast().hide();
        } else {
          TopPersistentToast().show(context, message);
        }
        break;
      case _EditorToastRequest.saveSuccess:
        TopPersistentToast().hide();
        Fluttertoast.showToast(msg: context.tr("common.save_success"));
        break;
      case _EditorToastRequest.clear:
        TopPersistentToast().hide();
        break;
    }
  }

  Future<bool> _confirmDiscardUnsavedChanges() async {
    _showToast(_EditorToastRequest.clear);
    final shouldExit = await showCommonDialog(
      context,
      context.tr("journey.editor.discard_changes_confirm"),
      hasCancel: true,
    );
    return shouldExit;
  }

  @override
  void initState() {
    _editSession = widget.editSession;
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    try {
      final (rendererProxy, bounds) = await _editSession.getMapRendererProxy();
      if (!mounted) return;
      setState(() {
        _mapRendererProxy = rendererProxy;
        _initialMapBounds = bounds;
        _canUndo = _editSession.canUndo();
      });
    } catch (e) {
      log.error("[JourneyTrackEditPage] Load map error: $e");
      _mapWebviewKey.currentState?.setDrawMode(false);
      _mapWebviewKey.currentState?.setDeleteMode(false);
    }
  }

  bool _beginOperation() {
    if (!mounted || _operationInProgress) return false;
    setState(() {
      _operationInProgress = true;
    });
    return true;
  }

  void _finishOperation() {
    if (!mounted) return;
    setState(() {
      _operationInProgress = false;
      _canUndo = _editSession.canUndo();
    });
  }

  Future<void> _showOperationError() async {
    if (!mounted) return;
    _showToast(_EditorToastRequest.clear);
    await showCommonDialog(
      context,
      context.tr("journey.editor.operation_failed"),
    );
    if (mounted) {
      _showToast(_EditorToastRequest.syncCurrentState);
    }
  }

  void _applyMode(
    OperationMode next, {
    bool clearLinkedDrawError = false,
    bool syncToastWhenUnchanged = false,
  }) {
    if (!mounted) return;

    if (next == OperationMode.edit && _zoomOk == false) {
      next = OperationMode.editReadonly;
    } else if (next == OperationMode.editReadonly && _zoomOk == true) {
      next = OperationMode.edit;
    }

    if (next == _mode) {
      if (clearLinkedDrawError || syncToastWhenUnchanged) {
        _showToast(
          _EditorToastRequest.syncCurrentState,
          clearLinkedDrawError: clearLinkedDrawError,
        );
      }
      return;
    }

    setState(() {
      _mode = next;
    });

    final map = _mapWebviewKey.currentState;

    switch (next) {
      case OperationMode.move:
        map?.setDrawMode(false);
        map?.setDeleteMode(false);
        break;

      case OperationMode.edit:
        map?.setDrawMode(true);
        map?.setDeleteMode(false);
        break;

      case OperationMode.editReadonly:
        map?.setDrawMode(false);
        map?.setDeleteMode(false);
        break;

      case OperationMode.delete:
        map?.setDrawMode(false);
        map?.setDeleteMode(true);
        break;
    }

    _showToast(
      _EditorToastRequest.syncCurrentState,
      clearLinkedDrawError: clearLinkedDrawError,
    );
  }

  void _handleModeChange(OperationMode mode) {
    if (_operationInProgress) return;
    final shouldClearLinkedError = _linkedDrawErrorTrKey != null;

    if (_isDrawModeMenuOpen) {
      setState(() {
        _isDrawModeMenuOpen = false;
      });
    }

    _applyMode(mode, clearLinkedDrawError: shouldClearLinkedError);
  }

  void _handleDrawToolPressed() {
    if (_operationInProgress) return;
    final shouldOpenMenu = !_isDrawModeMenuOpen;
    final isDrawMode =
        _mode == OperationMode.edit || _mode == OperationMode.editReadonly;

    if (!isDrawMode) {
      _applyMode(
        OperationMode.edit,
        clearLinkedDrawError: _linkedDrawErrorTrKey != null,
      );
    }

    if (!mounted) return;
    setState(() {
      _isDrawModeMenuOpen = shouldOpenMenu;
    });
  }

  void _dismissDrawModeMenu() {
    if (!mounted || !_isDrawModeMenuOpen) return;
    setState(() {
      _isDrawModeMenuOpen = false;
    });
  }

  void _handleDrawEntrySelected(DrawEntryMode mode) {
    if (_operationInProgress) return;
    final wasMode = _mode;
    final wasErrorLocked = _linkedDrawErrorTrKey != null;

    setState(() {
      _isLinkedDrawEnabled = mode == DrawEntryMode.linked;
      _isDrawModeMenuOpen = false;
    });

    if (wasMode == OperationMode.edit) {
      _showToast(
        _EditorToastRequest.syncCurrentState,
        clearLinkedDrawError: wasErrorLocked,
      );
      return;
    }

    _applyMode(OperationMode.edit, clearLinkedDrawError: wasErrorLocked);
  }

  void _handleMapZoomUpdate(int? zoom) {
    if (!mounted || zoom == null) return;

    final nextZoomOk = zoom >= _minEditZoom;
    if (nextZoomOk == _zoomOk) return;

    setState(() {
      _zoomOk = nextZoomOk;
    });

    _applyMode(
      _mode,
      clearLinkedDrawError: _linkedDrawErrorTrKey != null,
      syncToastWhenUnchanged: true,
    );
  }

  Future<void> _onDrawPath(List<JourneyEditorDrawPoint> points) async {
    if (_mode != OperationMode.edit) return;
    if (points.length < 2) return;
    if (!_beginOperation()) return;

    try {
      if (_linkedDrawErrorTrKey != null) {
        _clearLinkedDrawConstraintError();
      }

      final recordPoints = points
          .map((p) => (p.lat, p.lng))
          .toList(growable: false);
      final outcome = await _editSession.addLines(
        points: recordPoints,
        snapEndpoints: _isLinkedDrawEnabled,
      );
      switch (outcome) {
        case AddLinesOutcome.added || AddLinesOutcome.ignored:
          if (!mounted) return;
          _showToast(_EditorToastRequest.syncCurrentState);
          await _mapWebviewKey.currentState?.manualRefresh();
        case AddLinesOutcome.linkedDrawTooFar:
          _showLinkedDrawConstraintToast('journey.editor.linked_draw_too_far');
        case AddLinesOutcome.linkedDrawNeedsMultipleTracks:
          _showLinkedDrawConstraintToast(
            'journey.editor.linked_draw_needs_multiple_tracks',
          );
        case AddLinesOutcome.linkedDrawInvalidLinkTargets:
          _showLinkedDrawConstraintToast(
            'journey.editor.linked_draw_invalid_link_targets',
          );
      }
    } catch (error, stackTrace) {
      log.error("[JourneyTrackEditPage] addLines failed: $error", stackTrace);
      await _showOperationError();
    } finally {
      _finishOperation();
    }
  }

  Future<void> _onSelectionBox(
    double startLat,
    double startLng,
    double endLat,
    double endLng,
  ) async {
    if (_mode != OperationMode.delete) return;
    if (!_beginOperation()) return;

    try {
      await _editSession.deletePointsInBox(
        startLat: startLat,
        startLng: startLng,
        endLat: endLat,
        endLng: endLng,
      );

      if (!mounted) return;
      await _mapWebviewKey.currentState?.manualRefresh();
    } catch (error, stackTrace) {
      log.error(
        "[JourneyTrackEditPage] deletePointsInBox failed: $error",
        stackTrace,
      );
      await _showOperationError();
    } finally {
      _finishOperation();
    }
  }

  Future<void> _undo() async {
    if (!_canUndo || !_beginOperation()) return;
    _dismissDrawModeMenu();
    try {
      await _editSession.undo();
      if (!mounted) return;
      await _mapWebviewKey.currentState?.manualRefresh();
    } catch (error, stackTrace) {
      log.error("[JourneyTrackEditPage] undo failed: $error", stackTrace);
      await _showOperationError();
    } finally {
      _finishOperation();
    }
  }

  Future<void> _save() async {
    if (!_canUndo || !_beginOperation()) return;
    _dismissDrawModeMenu();
    _showToast(_EditorToastRequest.clear);

    try {
      final shouldSave = await showCommonDialog(
        context,
        context.tr("common.save_confirm"),
        title: context.tr("common.save"),
        hasCancel: true,
      );
      if (!mounted) return;
      if (!shouldSave) {
        _showToast(
          _EditorToastRequest.syncCurrentState,
          clearLinkedDrawError: true,
        );
        return;
      }

      await showLoadingDialog(asyncTask: _editSession.commit());
      if (!mounted) return;
      _showToast(_EditorToastRequest.saveSuccess);
      popCurrentRoute(context, true);
    } catch (error, stackTrace) {
      log.error("[JourneyTrackEditPage] commit failed: $error", stackTrace);
      await _showOperationError();
    } finally {
      _finishOperation();
    }
  }

  @override
  void dispose() {
    // Keep the editor's persistent overlay from leaking onto the previous page.
    _showToast(_EditorToastRequest.clear);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;
        if (_operationInProgress) return;

        if (_canUndo) {
          final shouldExit = await _confirmDiscardUnsavedChanges();
          if (!context.mounted) return;
          if (!shouldExit) {
            _showToast(
              _EditorToastRequest.syncCurrentState,
              clearLinkedDrawError: true,
            );
            return;
          }
        }
        _showToast(_EditorToastRequest.clear);
        if (!context.mounted) return;

        Navigator.of(context).pop(result);
      },
      child: Scaffold(
        body: Stack(
          children: [
            if (_mapRendererProxy != null)
              Listener(
                onPointerDown: (_) => _dismissDrawModeMenu(),
                child: JourneyEditorMapView(
                  key: _mapWebviewKey,
                  mapRendererProxy: _mapRendererProxy!,
                  initialMapBounds: _initialMapBounds,
                  onSelectionBox: _onSelectionBox,
                  onDrawPath: _onDrawPath,
                  onMapZoomChanged: _handleMapZoomUpdate,
                ),
              )
            else
              const Center(child: CircularProgressIndicator()),
            CapsuleStyleOverlayAppBar.overlayBar(
              title: context.tr("journey.editor.page_title"),
            ),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: SafeArea(
                minimum: const EdgeInsets.all(ModeSwitchBar.safeAreaMinimum),
                child: PointerInterceptor(
                  child: ModeSwitchBar(
                    currentMode: _mode,
                    onModeChanged: _handleModeChange,
                    currentDrawMode: _isLinkedDrawEnabled
                        ? DrawEntryMode.linked
                        : DrawEntryMode.freehand,
                    isDrawMenuOpen: _isDrawModeMenuOpen,
                    onDrawPressed: _handleDrawToolPressed,
                    onDrawModeChanged: _handleDrawEntrySelected,
                    onUndo: _canUndo && !_operationInProgress ? _undo : null,
                    onSave: _canUndo && !_operationInProgress ? _save : null,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
