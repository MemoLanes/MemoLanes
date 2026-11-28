import 'package:flutter/foundation.dart' show VoidCallback;
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording/location_recording.dart';

import 'geolocator_service.dart';
import 'location_service.dart';
import 'tracelet_location_service.dart';

const _legacyPreference = 'Settings.useLegacyLocationBackend';

const supportedLocationBackends = [
  LocationBackend.tracelet,
  LocationBackend.native,
];

LocationBackend get selectedLocationBackend =>
    MMKVUtil.getBool(_legacyPreference)
    ? LocationBackend.native
    : LocationBackend.tracelet;

ILocationService createLocationService([
  LocationBackend? backend,
  VoidCallback? onJourneyFinalized,
]) {
  backend ??= selectedLocationBackend;
  return switch (backend) {
    LocationBackend.native => GeoLocatorService(),
    LocationBackend.tracelet => TraceletLocationService(
      (data) async {
        await recordLocation(data, receivedAt: DateTime.now());
      },
      checkBatchStart: (data) async {
        final finalized = await tryAutoFinalizeBeforeLocation(
          data,
          receivedAt: DateTime.now(),
        );
        if (finalized == true) onJourneyFinalized?.call();
        return finalized != null;
      },
    ),
  };
}

void saveLocationBackend(LocationBackend backend) {
  if (!MMKVUtil.putBool(_legacyPreference, backend == LocationBackend.native)) {
    throw StateError('Could not save the location provider preference');
  }
}
