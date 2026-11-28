import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';
import 'package:memolanes/common/service/location/geolocator_service.dart';

class _LocationPlatform extends GeolocatorPlatform {
  final positions = StreamController<Position>(sync: true);

  @override
  Stream<Position> getPositionStream({LocationSettings? locationSettings}) =>
      positions.stream;

  void send(int timestampMs) => positions.add(
    Position(
      longitude: 120,
      latitude: 30,
      timestamp: DateTime.fromMillisecondsSinceEpoch(timestampMs),
      accuracy: 5,
      altitude: 0,
      altitudeAccuracy: 0,
      heading: 0,
      headingAccuracy: 0,
      speed: 0,
      speedAccuracy: 0,
    ),
  );
}

void main() {
  test(
    'idle checks and stopping flush buffered fixes in sample order',
    () async {
      debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      final original = GeolocatorPlatform.instance;
      addTearDown(() => GeolocatorPlatform.instance = original);
      final platform = _LocationPlatform();
      GeolocatorPlatform.instance = platform;
      final service = GeoLocatorService();
      final fixes = <int>[];
      service.onLocationUpdate((fix) => fixes.add(fix.timestampMs));
      await service.startLocationUpdates(true);

      platform.send(20);
      platform.send(10);
      expect(fixes, isEmpty);
      service.flushPendingUpdates();
      expect(fixes, [10, 20]);

      platform.send(30);
      await service.dispose();
      expect(fixes, [10, 20, 30]);
      await platform.positions.close();
    },
  );
}
