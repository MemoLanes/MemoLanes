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
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;
import 'package:pointer_interceptor/pointer_interceptor.dart';

class JourneyTrackEditPage extends StatefulWidget {
  final EditSession editSession;

  const JourneyTrackEditPage({super.key, required this.editSession});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  static const int _minEditZoom = 13;
  static const String _linkedDrawTooFarError = 'linked_draw_too_far';

  late final EditSession _editSession;
  api.MapRendererProxy? _mapRendererProxy;
  JourneyEditorMapViewCamera? _initialMapView;

  OperationMode _mode = OperationMode.move;
  bool _canUndo = false;
  bool _isLinkedDrawEnabled = false;
  bool _isLinkedDrawErrorLocked = false;

  bool _zoomOk = false;

  final GlobalKey<JourneyEditorMapViewState> _mapWebviewKey = GlobalKey();

  void _showDrawModeEnabledToast() {
    _showFloatingSnackBar(
      context.tr(
        _isLinkedDrawEnabled
            ? "journey.editor.linked_draw_mode_enabled"
            : "journey.editor.free_draw_mode_enabled",
      ),
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

  String _linkedDrawTooFarMessage() {
    return context.tr("journey.editor.linked_draw_too_far");
  }

  bool _isLinkedDrawTooFarError(Object error) {
    return error.toString().contains(_linkedDrawTooFarError);
  }

  void _showFloatingSnackBar(String message) {
    if (!mounted) return;
    if (_isLinkedDrawErrorLocked) return;
    TopPersistentToast().show(context, message);
  }

  void _lockLinkedDrawErrorToast() {
    if (!mounted) return;
    setState(() {
      _isLinkedDrawErrorLocked = true;
    });
  }

  void _unlockLinkedDrawErrorToast() {
    if (!mounted || !_isLinkedDrawErrorLocked) return;
    setState(() {
      _isLinkedDrawErrorLocked = false;
    });
  }

  void _restoreNormalEditingToast() {
    if (_isLinkedDrawErrorLocked) {
      _unlockLinkedDrawErrorToast();
      _removeToast();
    }
    _showDrawModeEnabledToast();
  }

  void _restoreCurrentModeToast() {
    if (_mode == OperationMode.edit) {
      _restoreNormalEditingToast();
      return;
    }

    if (_mode == OperationMode.editReadonly) {
      if (_isLinkedDrawErrorLocked) {
        _unlockLinkedDrawErrorToast();
        _removeToast();
      }
      _showZoomTooLow();
      return;
    }

    if (_mode == OperationMode.delete) {
      if (_isLinkedDrawErrorLocked) {
        _unlockLinkedDrawErrorToast();
        _removeToast();
      }
      _showDeleteModeEnabled();
    }
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
    _editSession = widget.editSession;
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    try {
      final (rendererProxy, cameraOption) =
          await _editSession.getMapRendererProxy();
      setState(() {
        _mapRendererProxy = rendererProxy;
        if (cameraOption != null) {
          _initialMapView = (
            lng: cameraOption.lng,
            lat: cameraOption.lat,
            zoom: cameraOption.zoom,
          );
        }
        _canUndo = _editSession.canUndo();
      });
    } catch (e) {
      log.error("[JourneyTrackEditPage] Load map error: $e");
      _mapWebviewKey.currentState?.setDrawMode(false);
      _mapWebviewKey.currentState?.setDeleteMode(false);
    }
  }

  Future<void> _refreshCanUndo() async {
    final canUndo = _editSession.canUndo();
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
        _showDrawModeEnabledToast();
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
    if (mode == OperationMode.edit) {
      if (_isLinkedDrawErrorLocked) {
        _restoreNormalEditingToast();
      }

      final switchedFromLinked = _isLinkedDrawEnabled;
      if (switchedFromLinked) {
        setState(() {
          _isLinkedDrawEnabled = false;
        });
      }

      if (_mode == OperationMode.edit) {
        if (_isLinkedDrawErrorLocked || switchedFromLinked) {
          _showDrawModeEnabledToast();
        }
        return;
      }
    }

    if (_isLinkedDrawErrorLocked) {
      _unlockLinkedDrawErrorToast();
      _removeToast();
    }

    _applyMode(mode);
  }

  void _handleDrawEntrySelected(DrawEntryMode mode) {
    final wasMode = _mode;
    final wasErrorLocked = _isLinkedDrawErrorLocked;

    setState(() {
      _isLinkedDrawEnabled = mode == DrawEntryMode.linked;
    });

    if (wasErrorLocked) {
      _restoreNormalEditingToast();
      if (wasMode == OperationMode.edit) return;
    }

    if (wasMode == OperationMode.edit) {
      _showDrawModeEnabledToast();
      return;
    }

    _applyMode(OperationMode.edit);
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

    if (_isLinkedDrawErrorLocked) {
      _restoreNormalEditingToast();
    }

    final recordPoints = points.map((p) => (p.lat, p.lng)).toList();

    try {
      await _editSession.addLines(
        points: recordPoints,
        snapEndpoints: _isLinkedDrawEnabled,
      );
    } catch (error, stackTrace) {
      if (_isLinkedDrawTooFarError(error)) {
        if (mounted) {
          TopPersistentToast().show(context, _linkedDrawTooFarMessage());
        }
        _lockLinkedDrawErrorToast();
        return;
      }

      log.error("[JourneyTrackEditPage] addLines failed: $error", stackTrace);
      return;
    }

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

    await _editSession.deletePointsInBox(
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
    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;

        if (_canUndo) {
          final shouldExit = await _confirmDiscardUnsavedChanges();
          if (!context.mounted || !shouldExit) return;
        }
        _removeToast();
        if (!context.mounted) return;

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
            CapsuleStyleOverlayAppBar.overlayBar(
              title: context.tr("journey.editor.page_title"),
            ),
            if (_mode == OperationMode.edit ||
                _mode == OperationMode.editReadonly)
              Positioned.fill(
                child: SafeArea(
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 24),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        const Spacer(),
                        Padding(
                          padding: EdgeInsets.only(
                            right: 8,
                            bottom: isLandscape ? 16 : screenSize.height * 0.08,
                          ),
                          child: PointerInterceptor(
                            child: Column(
                              mainAxisSize: MainAxisSize.min,
                              crossAxisAlignment: CrossAxisAlignment.end,
                              children: [
                                _DrawEntryModeButton(
                                  icon: Icons.draw_rounded,
                                  tooltip:
                                      context.tr('journey.editor.free_draw'),
                                  isSelected: !_isLinkedDrawEnabled,
                                  onPressed: () => _handleDrawEntrySelected(
                                    DrawEntryMode.freehand,
                                  ),
                                ),
                                _DrawEntryModeButton(
                                  icon: Icons.link_rounded,
                                  tooltip:
                                      context.tr('journey.editor.linked_draw'),
                                  isSelected: _isLinkedDrawEnabled,
                                  onPressed: () => _handleDrawEntrySelected(
                                    DrawEntryMode.linked,
                                  ),
                                ),
                              ],
                            ),
                          ),
                        ),
                        const SizedBox(height: 172),
                      ],
                    ),
                  ),
                ),
              ),
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
                    await _editSession.undo();
                    if (!mounted) return;
                    await _mapWebviewKey.currentState?.manualRefresh();
                    _refreshCanUndo();
                  },
                  canSave: _canUndo,
                  onSave: () async {
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
                    if (!context.mounted) return;
                    if (!shouldSave) {
                      _restoreCurrentModeToast();
                      return;
                    }

                    await showLoadingDialog(
                      asyncTask: _editSession.commit(),
                    );
                    if (!context.mounted) return;
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

class _DrawEntryModeButton extends StatelessWidget {
  const _DrawEntryModeButton({
    required this.icon,
    required this.tooltip,
    required this.isSelected,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final bool isSelected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    const accentColor = Color(0xFFB4EC51);

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      width: 48,
      height: 48,
      decoration: const BoxDecoration(
        color: Colors.black,
        shape: BoxShape.circle,
      ),
      child: IconButton(
        onPressed: onPressed,
        tooltip: tooltip,
        icon: Icon(
          icon,
          color: isSelected ? accentColor : accentColor.withValues(alpha: 0.5),
        ),
      ),
    );
  }
}
