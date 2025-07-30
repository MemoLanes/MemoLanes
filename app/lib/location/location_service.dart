import 'dart:async';

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

abstract class ILocationService {
  Future<void> startLocationUpdates(bool enableBackground);

  Future<void> stopLocationUpdates();

  StreamSubscription<LocationData> onLocationUpdate(
      void Function(LocationData) callback);
}
