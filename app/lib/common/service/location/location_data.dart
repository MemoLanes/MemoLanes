class LocationData {
  const LocationData({
    required this.latitude,
    required this.longitude,
    required this.accuracy,
    required this.timestampMs,
    this.altitude,
    this.speed,
  });

  factory LocationData.fromMap(Map<String, dynamic> map) {
    return LocationData(
      latitude: (map['latitude'] as num).toDouble(),
      longitude: (map['longitude'] as num).toDouble(),
      accuracy: (map['accuracy'] as num?)?.toDouble() ?? 0,
      timestampMs: (map['timestampMs'] as num).toInt(),
      altitude: (map['altitude'] as num?)?.toDouble(),
      speed: (map['speed'] as num?)?.toDouble(),
    );
  }

  final double latitude;
  final double longitude;
  final int timestampMs;
  final double accuracy;
  final double? altitude;
  final double? speed;

  DateTime get timestamp => DateTime.fromMillisecondsSinceEpoch(timestampMs);

  Map<String, Object?> toMap() => <String, Object?>{
    'latitude': latitude,
    'longitude': longitude,
    'accuracy': accuracy,
    'timestampMs': timestampMs,
    'altitude': altitude,
    'speed': speed,
  };

  @override
  String toString() {
    return 'LocationData(latitude: $latitude, longitude: $longitude, '
        'timestampMs: $timestampMs, accuracy: $accuracy, altitude: $altitude, '
        'speed: $speed)';
  }
}

/// Returns a stable ordering by the timestamp supplied by the provider.
///
/// Providers may deliver buffered or recovered points out of order. The GPS
/// preprocessor is stateful, so every recording boundary must establish a
/// deterministic order before passing points to it.
List<LocationData> sortLocationDataByTimestamp(
  Iterable<LocationData> locations,
) {
  final indexed = locations.indexed.toList(growable: false);
  indexed.sort((a, b) {
    final timestampOrder = a.$2.timestampMs.compareTo(b.$2.timestampMs);
    return timestampOrder != 0 ? timestampOrder : a.$1.compareTo(b.$1);
  });
  return indexed.map((entry) => entry.$2).toList(growable: false);
}
