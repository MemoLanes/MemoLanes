import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/service/location/location_data.dart';

LocationData _location(int timestampMs, double latitude) => LocationData(
  latitude: latitude,
  longitude: 2,
  accuracy: 3,
  timestampMs: timestampMs,
  altitude: 4,
  speed: 5,
);

void main() {
  test('round-trips through isolate-safe map data', () {
    final location = _location(123, 1);

    final restored = LocationData.fromMap(location.toMap());

    expect(restored.latitude, location.latitude);
    expect(restored.longitude, location.longitude);
    expect(restored.accuracy, location.accuracy);
    expect(restored.timestampMs, location.timestampMs);
    expect(restored.altitude, location.altitude);
    expect(restored.speed, location.speed);
  });

  test('sorts by provider timestamp and preserves ties', () {
    final firstTie = _location(20, 1);
    final secondTie = _location(20, 2);

    final sorted = sortLocationDataByTimestamp([
      firstTie,
      _location(10, 3),
      secondTie,
    ]);

    expect(sorted.map((location) => location.timestampMs), [10, 20, 20]);
    expect(sorted[1], same(firstTie));
    expect(sorted[2], same(secondTie));
  });
}
