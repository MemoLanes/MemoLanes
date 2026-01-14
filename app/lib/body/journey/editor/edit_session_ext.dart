import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/edit_session.dart' show EditSession;

extension JourneyEditSessionExt on EditSession {
  String journeyId() => api.editSessionJourneyId(that: this);

  bool isVector() => api.editSessionIsVector(that: this);

  bool canUndo() => api.editSessionCanUndo(that: this);

  Future<bool> pushUndoCheckpoint() =>
      api.editSessionPushUndoCheckpoint(that: this);

  Future<(api.MapRendererProxy, api.CameraOption?)> getMapRendererProxy() =>
      api.editSessionGetMapRendererProxy(that: this);

  Future<(api.MapRendererProxy, api.CameraOption?)> undo() =>
      api.editSessionUndo(that: this);

  Future<(api.MapRendererProxy, api.CameraOption?)> deletePointsInBox({
    required double startLat,
    required double startLng,
    required double endLat,
    required double endLng,
  }) =>
      api.editSessionDeletePointsInBox(
        that: this,
        startLat: startLat,
        startLng: startLng,
        endLat: endLat,
        endLng: endLng,
      );

  Future<(api.MapRendererProxy, api.CameraOption?)> addLine({
    required double startLat,
    required double startLng,
    required double endLat,
    required double endLng,
  }) =>
      api.editSessionAddLine(
        that: this,
        startLat: startLat,
        startLng: startLng,
        endLat: endLat,
        endLng: endLng,
      );

  Future<void> commit() => api.editSessionCommit(that: this);

  void discard() => api.editSessionDiscard(that: this);
}
