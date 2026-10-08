import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:memolanes/common/log.dart';

const _channel = MethodChannel('com.memolanes/device_region');

/// Returns an uppercase system region code, or null when none is available.
/// Falls back to the first preferred locale with a region on other platforms.
Future<String?> getDeviceRegion() async {
  try {
    final region = _normalizeRegion(
      await _channel.invokeMethod<String>('getRegion'),
    );
    if (region != null) return region;
  } catch (error) {
    log.warning('Failed to read device region: $error');
  }

  for (final locale in WidgetsBinding.instance.platformDispatcher.locales) {
    final region = _normalizeRegion(locale.countryCode);
    if (region != null) return region;
  }
  return null;
}

String? _normalizeRegion(String? value) {
  final region = value?.trim().toUpperCase();
  return region == null || region.isEmpty ? null : region;
}
