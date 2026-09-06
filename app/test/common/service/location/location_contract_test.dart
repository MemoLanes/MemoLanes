import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/service/location/geolocator_service.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/main.dart' as app;

class _FakeLocationService implements ILocationService {
  static const info = LocationProviderInfo(
    id: 'test-provider',
    displayName: _displayName,
  );

  static String _displayName(BuildContext context) => 'Test provider';

  final controller = StreamController<LocationData>.broadcast();
  final lifecycleEvents = <bool>[];
  LocationStartOptions? options;
  Completer<void>? stopBarrier;
  var disposed = false;

  @override
  Stream<LocationData> get locations => controller.stream;

  @override
  LocationProviderInfo get providerInfo => info;

  @override
  Future<void> setForeground(bool foreground) async {
    lifecycleEvents.add(foreground);
  }

  @override
  Future<void> recoverPendingDeliveries(LocationBatchConsumer consumer) async {}

  @override
  Future<void> start(LocationStartOptions options) async {
    this.options = options;
  }

  @override
  Future<void> stop() => stopBarrier?.future ?? Future<void>.value();

  @override
  Future<void> dispose() async {
    disposed = true;
    await stop();
    await controller.close();
  }
}

void main() {
  test(
    'third-party providers implement the contract without an enum change',
    () {
      final provider = _FakeLocationService();
      ILocationService factory() => provider;

      expect(factory(), same(provider));
      expect(provider.providerInfo.id, 'test-provider');
      expect(provider.providerInfo, isNot(LocationProviderInfo.native));
    },
  );

  test('product entry point accepts a location service factory', () {
    void runner({LocationServiceFactory? locationServiceFactory}) =>
        app.runMemoLanesApp(locationServiceFactory: locationServiceFactory);

    expect(runner, isNotNull);
  });

  test('the upstream default factory creates the native provider', () async {
    final provider = createDefaultLocationService();

    expect(provider, isA<GeoLocatorService>());
    expect(provider.providerInfo, LocationProviderInfo.native);
    await (provider as GeoLocatorService).dispose();
  });

  test('stop can wait for provider-owned in-flight work', () async {
    final provider = _FakeLocationService();
    provider.stopBarrier = Completer<void>();

    var completed = false;
    final stopping = provider.stop().then((_) => completed = true);
    await Future<void>.delayed(Duration.zero);
    expect(completed, isFalse);

    provider.stopBarrier!.complete();
    await stopping;
    expect(completed, isTrue);
    await provider.dispose();
    expect(provider.disposed, isTrue);
  });

  test('durable cursor describes a contiguous stable-prefix batch', () {
    const cursor = DurableDeliveryCursor(
      providerId: 'test-provider',
      streamId: 'recording-1',
      firstSequence: 4,
      lastSequence: 6,
    );
    const batch = LocationRecordingBatch(
      locations: [
        LocationData(latitude: 1, longitude: 2, accuracy: 3, timestampMs: 4),
        LocationData(latitude: 2, longitude: 3, accuracy: 4, timestampMs: 5),
        LocationData(latitude: 3, longitude: 4, accuracy: 5, timestampMs: 6),
      ],
      isReplay: true,
      durableCursor: cursor,
    );

    expect(batch.durableCursor?.providerId, 'test-provider');
    expect(batch.durableCursor?.streamId, 'recording-1');
    expect(batch.durableCursor?.firstSequence, 4);
    expect(batch.durableCursor?.lastSequence, 6);
    expect(batch.isReplay, isTrue);
  });
}
