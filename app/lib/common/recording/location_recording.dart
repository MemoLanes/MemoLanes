import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/gps.dart';

Future<bool> recordLocation(
  LocationData data, {
  required DateTime receivedAt,
}) => api.onLocationUpdate(
  data: ExtendedRawGPSPoint(
    rawGpsPoint: RawGPSPoint(
      point: Point(latitude: data.latitude, longitude: data.longitude),
      timestampMs: data.timestampMs,
      accuracy: data.accuracy,
      altitude: data.altitude,
      speed: data.speed,
    ),
    receivedTimestampMs: receivedAt.millisecondsSinceEpoch,
  ),
);
