import 'dart:async';

import 'package:flutter/material.dart';
import 'package:easy_localization/easy_localization.dart';

class LocationData {
  final double latitude;
  final double longitude;
  final int timestampMs;
  final double accuracy;
  final double? altitude;
  final double? speed;

  LocationData({
    required this.latitude,
    required this.longitude,
    required this.accuracy,
    required this.timestampMs,
    this.altitude,
    this.speed,
  });

  DateTime get timestamp => DateTime.fromMillisecondsSinceEpoch(timestampMs);

  @override
  String toString() {
    return 'LocationData(latitude: $latitude,longitude: $longitude, timestampMs: $timestampMs, accuracy: $accuracy, altitude: $altitude, speed: $speed)';
  }
}

enum LocationBackend {
  native;

  String displayName(BuildContext context) {
    switch (this) {
      case LocationBackend.native:
        return context.tr("location_service.location_backend.native");
    }
  }
}

abstract class ILocationService {
  Future<void> startLocationUpdates(bool enableBackground);

  Future<void> stopLocationUpdates();

  StreamSubscription<LocationData> onLocationUpdate(
      void Function(LocationData) callback);

  LocationBackend get locationBackend;
}
