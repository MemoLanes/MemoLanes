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

/// Represents an error encountered by the location service.
class LocationError {
  final LocationErrorCode code;
  final String message;

  LocationError({required this.code, required this.message});

  @override
  String toString() {
    return 'LocationError(code: $code, message: $message)';
  }
}

/// Specific error codes for location issues.
enum LocationErrorCode {
  permissionDenied,
  locationServiceDisabled,
  timeout,
  positionUnavailable,
  unknown,
}

/// An abstract interface for managing location services.
/// This allows for easy swapping of location providers (e.g., Geolocator, mock services).
abstract class ILocationService {
  /// Initializes and starts continuous location updates.
  ///
  /// The [options] parameter allows configuration of accuracy, update frequency,
  /// and background location settings.
  ///
  /// Returns a [Future] that completes when the service has started listening
  /// for updates.
  Future<void> startLocationUpdates(bool enableBackground);

  /// Stops any ongoing location updates.
  ///
  /// Returns a [Future] that completes when the updates have been successfully stopped.
  Future<void> stopLocationUpdates();

  /// Retrieves the current device location once.
  ///
  /// This method does not start continuous updates.
  /// Returns a [Future] that resolves with the [LocationData] or rejects if
  /// an error occurs (e.g., location permission denied, GPS off).
  Future<LocationData> getCurrentLocation();

  /// Registers a callback function to receive continuous location updates.
  ///
  /// The `callback` function will be invoked with new [LocationData] whenever
  /// available, typically after [startLocationUpdates] has been called.
  ///
  /// Returns a [StreamSubscription] that can be used to cancel the subscription.
  StreamSubscription<LocationData> onLocationUpdate(
      void Function(LocationData) callback);

  /// Registers a callback function to receive error notifications from the
  /// location service.
  ///
  /// The `callback` function will be invoked with a [LocationError] object
  /// detailing the issue.
  ///
  /// Returns a [StreamSubscription] that can be used to cancel the subscription.
  StreamSubscription<LocationError> onLocationError(
      void Function(LocationError) callback);
}
