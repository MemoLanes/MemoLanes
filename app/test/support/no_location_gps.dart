import 'package:flutter/foundation.dart';
import 'package:memolanes/common/gps_manager.dart';

class NoLocationGps extends ChangeNotifier implements GpsManager {
  @override
  GpsRecordingStatus get recordingStatus => GpsRecordingStatus.none;

  @override
  Null get latestPosition => null;

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
