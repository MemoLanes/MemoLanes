import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';

/// Returns the OS-cached last known location via `geolocator`.
///
/// This helper is backend-agnostic: no matter which [LocationBackend] is active,
/// we always read the cached fix from `geolocator`.
///
/// The returned [LocationData] may be stale and is only for transient UI
Future<LocationData?> getLastKnownLocation() async {
  try {
    final pos = await Geolocator.getLastKnownPosition();
    if (pos == null) return null;
    return LocationData(
      latitude: pos.latitude,
      longitude: pos.longitude,
      accuracy: pos.accuracy,
      timestampMs: pos.timestamp.millisecondsSinceEpoch,
      altitude: pos.altitude,
      speed: pos.speed,
    );
  } catch (e, st) {
    log.error("[getLastKnownLocation] $e", st);
    return null;
  }
}
