import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/widgets.dart';

import 'location_data.dart';

export 'location_data.dart';

typedef LocationProviderDisplayName = String Function(BuildContext context);

String _nativeProviderDisplayName(BuildContext context) =>
    context.tr('location_service.location_backend.native');

/// Display metadata supplied by a location provider.
///
/// [id] is a stable machine-readable identifier. The display-name callback is
/// deliberately provider-owned so downstream products can add a provider
/// without changing an upstream enum or translation switch.
@immutable
class LocationProviderInfo {
  const LocationProviderInfo({required this.id, required this.displayName})
    : assert(id != '');

  static const native = LocationProviderInfo(
    id: 'native',
    displayName: _nativeProviderDisplayName,
  );

  final String id;
  final LocationProviderDisplayName displayName;

  @override
  bool operator ==(Object other) =>
      other is LocationProviderInfo && other.id == id;

  @override
  int get hashCode => id.hashCode;
}

/// A monotonic stable-prefix cursor owned by a durable provider.
///
/// Sequences start at one and are contiguous within one [providerId]/[streamId]
/// pair. A provider must retry the exact same payload with the exact same
/// cursor until it is acknowledged, then advance to the next contiguous range.
@immutable
class DurableDeliveryCursor {
  const DurableDeliveryCursor({
    required this.providerId,
    required this.streamId,
    required this.firstSequence,
    required this.lastSequence,
  }) : assert(providerId != ''),
       assert(streamId != ''),
       assert(firstSequence > 0),
       assert(lastSequence >= firstSequence);

  final String providerId;
  final String streamId;
  final int firstSequence;
  final int lastSequence;
}

@immutable
class LocationRecordingBatch {
  const LocationRecordingBatch({
    required this.locations,
    required this.isReplay,
    this.durableCursor,
  });

  final List<LocationData> locations;
  final bool isReplay;
  final DurableDeliveryCursor? durableCursor;
}

/// Delivers a recording batch to the application-owned recording pipeline.
///
/// A normally completed future acknowledges the whole batch, even when the
/// GPS preprocessor ignores every point. A thrown error means the batch was
/// not acknowledged; durable providers must retain it for a later retry.
typedef LocationBatchConsumer = Future<void> Function(
  LocationRecordingBatch batch,
);

class LocationStartOptions {
  const LocationStartOptions({
    required this.allowBackground,
    this.recordingConsumer,
  });

  final bool allowBackground;
  final LocationBatchConsumer? recordingConsumer;
}

typedef LocationServiceFactory = ILocationService Function();

/// Provider-neutral location acquisition.
///
/// [locations] is exclusively the live display stream. Recording data is
/// delivered through [LocationStartOptions.recordingConsumer], which also
/// gives durable providers a clear acknowledgement boundary.
abstract interface class ILocationService {
  LocationProviderInfo get providerInfo;

  Stream<LocationData> get locations;

  /// Starts acquisition. Repeating an equivalent start must be safe.
  Future<void> start(LocationStartOptions options);

  /// Reports app visibility and must tolerate duplicates and inactive states.
  Future<void> setForeground(bool foreground);

  /// Delivers the durable backlog that existed before this call began.
  ///
  /// This is an awaited recovery barrier: when it completes, every recoverable
  /// batch that preceded the call has either been acknowledged by [consumer]
  /// or the method has thrown. It must not start live acquisition. Providers
  /// without durable storage implement this as a no-op.
  Future<void> recoverPendingDeliveries(LocationBatchConsumer consumer);

  /// Stops producing points and waits for all in-flight recording deliveries.
  /// Repeating a stop must be safe.
  Future<void> stop();

  /// Permanently releases provider-owned subscriptions, timers, and streams.
  /// The service must not be started again after this completes.
  Future<void> dispose();
}
