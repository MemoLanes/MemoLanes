import 'package:flutter/foundation.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/recording/location_recording.dart';

import 'geolocator_service.dart';
import 'location_service.dart';
import 'tracelet_location_service.dart';

const _legacyPreference = 'Settings.useLegacyLocationBackend';

List<LocationBackend> get supportedLocationBackends => [
  if (!kIsWeb && defaultTargetPlatform == TargetPlatform.iOS)
    LocationBackend.tracelet,
  LocationBackend.native,
];

LocationBackend get selectedLocationBackend {
  final supported = supportedLocationBackends;
  return supported.length == 1 || MMKVUtil.getBool(_legacyPreference)
      ? LocationBackend.native
      : supported.first;
}

ILocationService createLocationService([LocationBackend? backend]) {
  backend ??= selectedLocationBackend;
  if (!supportedLocationBackends.contains(backend)) {
    throw UnsupportedError('Location service $backend is unavailable');
  }
  return switch (backend) {
    LocationBackend.native => GeoLocatorService(),
    LocationBackend.tracelet => TraceletLocationService((data) async {
      await recordLocation(data, receivedAt: DateTime.now());
    }),
  };
}

void saveLocationBackend(LocationBackend backend) {
  if (!supportedLocationBackends.contains(backend)) {
    throw UnsupportedError('Location service $backend is unavailable');
  }
  if (!MMKVUtil.putBool(_legacyPreference, backend == LocationBackend.native)) {
    throw StateError('Could not save the location provider preference');
  }
}
