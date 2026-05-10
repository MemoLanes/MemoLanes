import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/body/journey/editor/journey_editor_map_view.dart';
import 'package:memolanes/body/journey/editor/journey_track_edit_mode_bar.dart';
import 'package:memolanes/body/journey/editor/top_persistent_toast.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';
import 'package:memolanes/common/component/frosted_bar_container.dart';
import 'package:memolanes/common/component/frosted_bar_item.dart';
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

enum _EditorToastRequest {
  syncCurrentState,
  saveSuccess,
  clear,
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  static const int _minEditZoom = 13;

  late final EditSession _editSession;
  api.MapRendererProxy? _mapRendererProxy;
  JourneyEditorMapViewCamera? _initialMapView;

  OperationMode _mode = OperationMode.move;
  bool _canUndo = false;
  bool _isLinkedDrawEnabled = false;
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
          _zoomOk = cameraOption.zoom >= _minEditZoom;
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
    final shouldClearLinkedError = _linkedDrawErrorTrKey != null;

    if (mode == OperationMode.edit) {
      final switchedFromLinked = _isLinkedDrawEnabled;
      if (switchedFromLinked) {
        setState(() {
          _isLinkedDrawEnabled = false;
        });
      }

      if (_mode == OperationMode.edit) {
        if (shouldClearLinkedError || switchedFromLinked) {
          _showToast(
            _EditorToastRequest.syncCurrentState,
            clearLinkedDrawError: shouldClearLinkedError,
          );
        }
        return;
      }
    }

    _applyMode(mode, clearLinkedDrawError: shouldClearLinkedError);
  }

  void _handleDrawEntrySelected(DrawEntryMode mode) {
    final wasMode = _mode;
    final wasErrorLocked = _linkedDrawErrorTrKey != null;

    setState(() {
      _isLinkedDrawEnabled = mode == DrawEntryMode.linked;
    });

    if (wasMode == OperationMode.edit) {
      _showToast(
        _EditorToastRequest.syncCurrentState,
        clearLinkedDrawError: wasErrorLocked,
      );
      return;
    }

    _applyMode(
      OperationMode.edit,
      clearLinkedDrawError: wasErrorLocked,
    );
  }

  void _handleMapZoomUpdate(int? zoom) {
    if (zoom == null) return;

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

    if (_linkedDrawErrorTrKey != null) {
      _clearLinkedDrawConstraintError();
    }

    final recordPoints = points.map((p) => (p.lat, p.lng)).toList();

    try {
      final outcome = await _editSession.addLines(
        points: recordPoints,
        snapEndpoints: _isLinkedDrawEnabled,
      );
      if (outcome == AddLinesOutcome.linkedDrawTooFar) {
        _showLinkedDrawConstraintToast('journey.editor.linked_draw_too_far');
        return;
      }
      if (outcome == AddLinesOutcome.linkedDrawNeedsMultipleTracks) {
        _showLinkedDrawConstraintToast(
          'journey.editor.linked_draw_needs_multiple_tracks',
        );
        return;
      }
      if (outcome == AddLinesOutcome.linkedDrawInvalidLinkTargets) {
        _showLinkedDrawConstraintToast(
          'journey.editor.linked_draw_invalid_link_targets',
        );
        return;
      }
    } catch (error, stackTrace) {
      log.error("[JourneyTrackEditPage] addLines failed: $error", stackTrace);
      if (mounted) {
        _showToast(_EditorToastRequest.syncCurrentState);
      }
      return;
    }

    if (!mounted) return;
    _showToast(_EditorToastRequest.syncCurrentState);
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
    _showToast(_EditorToastRequest.clear);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    const double drawModeBarExtent = 60;
    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;

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
                            bottom: isLandscape ? 16 : screenSize.height * 0.08,
                          ),
                          child: PointerInterceptor(
                            child: FrostedBarContainer(
                              axis: Axis.vertical,
                              extent: drawModeBarExtent,
                              mainAxisPadding: 0,
                              child: Column(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  _DrawModeItemSlot(
                                    barExtent: drawModeBarExtent,
                                    child: _DrawEntryModeButton(
                                      icon: Icons.draw_rounded,
                                      label: context
                                          .tr('journey.editor.free_draw'),
                                      tooltip: context
                                          .tr('journey.editor.free_draw'),
                                      isSelected: !_isLinkedDrawEnabled,
                                      onPressed: () => _handleDrawEntrySelected(
                                        DrawEntryMode.freehand,
                                      ),
                                    ),
                                  ),
                                  _DrawModeItemSlot(
                                    barExtent: drawModeBarExtent,
                                    child: _DrawEntryModeButton(
                                      icon: Icons.link_rounded,
                                      label: context.tr(
                                        'journey.editor.linked_draw',
                                      ),
                                      tooltip: context
                                          .tr('journey.editor.linked_draw'),
                                      isSelected: _isLinkedDrawEnabled,
                                      onPressed: () => _handleDrawEntrySelected(
                                        DrawEntryMode.linked,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
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
                    _showToast(_EditorToastRequest.clear);

                    final shouldSave = await showCommonDialog(
                      context,
                      context.tr("common.save_confirm"),
                      title: context.tr("common.save"),
                      hasCancel: true,
                    );
                    if (!context.mounted) return;
                    if (!shouldSave) {
                      _showToast(
                        _EditorToastRequest.syncCurrentState,
                        clearLinkedDrawError: true,
                      );
                      return;
                    }

                    await showLoadingDialog(
                      asyncTask: _editSession.commit(),
                    );
                    if (!context.mounted) return;
                    _showToast(_EditorToastRequest.saveSuccess);

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

class _DrawModeItemSlot extends StatelessWidget {
  const _DrawModeItemSlot({
    required this.barExtent,
    required this.child,
  });

  final double barExtent;
  final Widget child;

  static const double _widthInset = 6;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: barExtent - _widthInset,
      height: barExtent,
      child: child,
    );
  }
}

class _DrawEntryModeButton extends StatelessWidget {
  const _DrawEntryModeButton({
    required this.icon,
    required this.label,
    required this.tooltip,
    required this.isSelected,
    required this.onPressed,
  });

  final IconData icon;
  final String label;
  final String tooltip;
  final bool isSelected;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: FrostedBarItem(
        icon: icon,
        label: label,
        isSelected: isSelected,
        onTap: onPressed,
      ),
    );
  }
}
