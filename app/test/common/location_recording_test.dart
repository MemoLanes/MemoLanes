import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/recording/location_recording.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/gps.dart';

void main() {
  test(
    'recordLocation preserves fields and capture and receipt times',
    () async {
      final backend = _RecordingApi();
      RustLib.initMock(api: backend);
      addTearDown(RustLib.dispose);
      final receivedAt = DateTime.utc(2026, 11, 16, 12);
      final data = LocationData(
        latitude: 1,
        longitude: 2,
        timestampMs: receivedAt.millisecondsSinceEpoch - 60000,
        accuracy: 3,
        altitude: 4,
        speed: 5,
      );

      expect(await recordLocation(data, receivedAt: receivedAt), isTrue);
      final recorded = backend.recorded!;
      expect(recorded.rawGpsPoint.point.latitude, data.latitude);
      expect(recorded.rawGpsPoint.point.longitude, data.longitude);
      expect(recorded.rawGpsPoint.timestampMs, data.timestampMs);
      expect(recorded.rawGpsPoint.accuracy, data.accuracy);
      expect(recorded.rawGpsPoint.altitude, data.altitude);
      expect(recorded.rawGpsPoint.speed, data.speed);
      expect(recorded.receivedTimestampMs, receivedAt.millisecondsSinceEpoch);
    },
  );
}

class _RecordingApi extends Fake implements RustLibApi {
  ExtendedRawGPSPoint? recorded;

  @override
  Future<bool> crateApiApiOnLocationUpdate({
    required ExtendedRawGPSPoint data,
  }) async {
    recorded = data;
    return true;
  }
}
