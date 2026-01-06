import 'package:flutter/material.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:easy_localization/easy_localization.dart';

class JourneyTrackEditPage extends StatefulWidget {
  final String journeyId;

  const JourneyTrackEditPage({super.key, required this.journeyId});

  @override
  State<JourneyTrackEditPage> createState() => _JourneyTrackEditPageState();
}

class _JourneyTrackEditPageState extends State<JourneyTrackEditPage> {
  api.MapRendererProxy? _mapRendererProxy;
  MapView? _initialMapView;
  bool _isAddMode = false;
  bool _isDeleteMode = false;
  bool _canUndo = false;
  bool _editingSupported = true;
  bool _popAllowed = false;
  final GlobalKey<BaseMapWebviewState> _mapWebviewKey = GlobalKey();

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

  void _showFloatingSnackBar(BuildContext context, String message) {
    final screenHeight = MediaQuery.of(context).size.height;
    final bottomMargin = screenHeight * 0.75;

    final messenger = ScaffoldMessenger.of(context);
    messenger.removeCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(
        content: _snackBarText(
          message,
          style: const TextStyle(color: Colors.white),
          allowExplicitNewlines: true,
        ),
        backgroundColor: Colors.black.withValues(alpha: 0.4),
        behavior: SnackBarBehavior.floating,
        margin: EdgeInsets.fromLTRB(16, 0, 16, bottomMargin),
        action: SnackBarAction(
          label: 'OK',
          onPressed: () {
            messenger.hideCurrentSnackBar();
          },
        ),
      ),
    );
  }

  void _showDefaultSnackBar(BuildContext context, String message) {
    final messenger = ScaffoldMessenger.of(context);
    messenger.showSnackBar(
      SnackBar(
        content: _snackBarText(message),
        action: SnackBarAction(
          label: 'OK',
          onPressed: () {
            messenger.hideCurrentSnackBar();
          },
        ),
      ),
    );
  }

  Future<bool> _confirmDiscardUnsavedChanges() async {
    final shouldExit = await showDialog<bool>(
          context: context,
          builder: (context) {
            return AlertDialog(
              title: Text(context.tr("common.info")),
              content: Text(
                context.tr(
                  "journey.journey_track_edit_discard_changes_confirm",
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.pop(context, false),
                  child: Text(context.tr("common.cancel")),
                ),
                TextButton(
                  onPressed: () => Navigator.pop(context, true),
                  child: Text(context.tr("common.ok")),
                ),
              ],
            );
          },
        ) ??
        false;

    return shouldExit;
  }

  @override
  void initState() {
    super.initState();
    _loadMap();
  }

  Future<void> _loadMap() async {
    final result = await api.startJourneyEdit(journeyId: widget.journeyId);
    if (mounted) {
      setState(() {
        _mapRendererProxy = result.$1;
        final cameraOption = result.$2;
        if (cameraOption != null) {
          _initialMapView = (
            lng: cameraOption.lng,
            lat: cameraOption.lat,
            zoom: cameraOption.zoom,
          );
        }
        _canUndo = false;
        _editingSupported = true;
      });
    }

    // Detect whether the edit session is backed by a vector journey.
    bool supported = true;
    try {
      await api.addLineInEdit(startLat: 0, startLng: 0, endLat: 0, endLng: 0);
    } catch (_) {
      supported = false;
    }

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
          context,
          context.tr("journey.journey_track_edit_bitmap_not_supported"),
        );
      });
    }
  }

  Future<void> _refreshCanUndo() async {
    final canUndo = await api.canUndoInEdit();
    if (!mounted) return;
    setState(() {
      _canUndo = canUndo;
    });
  }

  Future<void> _onDrawPath(List<DrawPoint> points) async {
    if (!_editingSupported) return;
    if (!_isAddMode) return;
    if (points.length < 2) return;

    await api.pushUndoCheckpointInEdit();

    // Approximate the freehand path by adding many small straight segments.
    api.MapRendererProxy? latestProxy = _mapRendererProxy;
    for (var i = 0; i < points.length - 1; i++) {
      final a = points[i];
      final b = points[i + 1];
      final result = await api.addLineInEdit(
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

  Future<void> _onSelectionBox(
    double startLat,
    double startLng,
    double endLat,
    double endLng,
  ) async {
    if (!_editingSupported) return;
    if (!_isDeleteMode) return;

    await api.pushUndoCheckpointInEdit();

    final result = await api.deletePointsInBoxInEdit(
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
    api.discardJourneyEdit();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      // When there are unsaved changes, block the pop until user confirms.
      canPop: !_canUndo || _popAllowed,
      onPopInvokedWithResult: (didPop, result) async {
        if (didPop) return;
        final shouldExit = await _confirmDiscardUnsavedChanges();
        if (!mounted) return;
        if (!context.mounted) return;
        if (!shouldExit) return;

        setState(() {
          _popAllowed = true;
        });
        Navigator.of(context).pop();
      },
      child: Scaffold(
        appBar: AppBar(
          title: Text(context.tr("journey.journey_track_edit_title")),
        ),
        body: SafeAreaWrapper(
          child: Stack(
            children: [
              if (_mapRendererProxy != null)
                BaseMapWebview(
                  key: _mapWebviewKey,
                  mapRendererProxy: _mapRendererProxy!,
                  initialMapView: _initialMapView,
                  trackingMode: TrackingMode.off,
                  onSelectionBox: _onSelectionBox,
                  onDrawPath: _onDrawPath,
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
                        backgroundColor: const Color(0xFFFFFFFF),
                        foregroundColor: Colors.black,
                        onPressed: !_editingSupported
                            ? null
                            : () async {
                                final result = await api.undoInEdit();
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
                      backgroundColor:
                          _isAddMode ? const Color(0xFFB6E13D) : Colors.grey,
                      foregroundColor: Colors.white,
                      onPressed: !_editingSupported
                          ? null
                          : () {
                              setState(() {
                                _isAddMode = !_isAddMode;
                                if (_isAddMode) {
                                  _isDeleteMode = false;
                                }
                              });
                              _mapWebviewKey.currentState?.setDeleteMode(false);
                              _mapWebviewKey.currentState
                                  ?.setDrawMode(_isAddMode);
                              _showFloatingSnackBar(
                                context,
                                _isAddMode
                                    ? context.tr(
                                        "journey.journey_track_edit_add_mode_enabled",
                                      )
                                    : context.tr(
                                        "journey.journey_track_edit_add_mode_disabled",
                                      ),
                              );
                            },
                      child: const Icon(Icons.edit),
                    ),
                    const SizedBox(width: 32),
                    FloatingActionButton(
                      heroTag: "delete_track",
                      backgroundColor:
                          _isDeleteMode ? const Color(0xFFE13D3D) : Colors.grey,
                      foregroundColor: Colors.white,
                      onPressed: !_editingSupported
                          ? null
                          : () {
                              setState(() {
                                _isDeleteMode = !_isDeleteMode;
                                if (_isDeleteMode) {
                                  _isAddMode = false;
                                }
                              });
                              _mapWebviewKey.currentState?.setDrawMode(false);
                              _mapWebviewKey.currentState
                                  ?.setDeleteMode(_isDeleteMode);
                              _showFloatingSnackBar(
                                context,
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
                      backgroundColor: const Color(0xFFFFFFFF),
                      onPressed: !_editingSupported
                          ? null
                          : () async {
                              await api.saveJourneyEdit();
                              if (!context.mounted) return;
                              _showDefaultSnackBar(
                                context,
                                context.tr("common.save_success"),
                              );
                              setState(() {
                                _popAllowed = true;
                              });
                              Navigator.pop(context, true);
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
