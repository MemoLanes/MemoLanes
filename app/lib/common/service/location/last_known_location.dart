import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/service/location/location_service.dart';

/// Returns the OS-cached last known location, used as a quick seed for the UI
/// while the live location stream is still acquiring a fix.
///
/// We always go through the `geolocator` plugin here, even when the active
/// [ILocationService] backend is [LocationBackend.tencent]. The Tencent LBS
/// SDK does not expose an equivalent "last known location" API, so the cached
/// fix from the OS (via `geolocator`) is the only cross-backend source we can
/// rely on for this purpose.
///
/// This is decoupled from any specific backend implementation on purpose:
/// - It is not part of [ILocationService] because Tencent cannot implement it.
/// - It does not live inside `GeoLocatorService` because callers should be
///   able to use it regardless of which backend is currently active.
///
/// The returned [LocationData] may be arbitrarily stale (hours/days old) and
/// must NOT be fed into journey recording. It is intended for transient UI
/// fallbacks (e.g. an initial map marker) only.
Future<LocationData?> getLastKnownLocation() async {
  try {
    final pos = await Geolocator.getLastKnownPosition();
    if (pos == null) return null;
    return LocationData(
      latitude: pos.latitude,
      longitude: pos.longitude,
      accuracy: pos.accuracy,
      timestampMs: pos.timestamp?.millisecondsSinceEpoch ?? 0,
      altitude: pos.altitude,
      speed: pos.speed,
    );
  } catch (e, st) {
    log.error("[getLastKnownLocation] $e", st);
    return null;
  }
}
