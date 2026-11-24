import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:logging/logging.dart';
import 'package:memolanes/common/service/location/tracelet_location_service.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:permission_handler/permission_handler.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('flutter.baseflow.com/permissions/methods');
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  setUp(() => debugDefaultTargetPlatformOverride = TargetPlatform.android);
  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
    messenger.setMockMethodCallHandler(channel, null);
  });

  test('resuming requires the current background location grant', () async {
    var status = PermissionStatus.denied;
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'checkPermissionStatus');
      expect(call.arguments, Permission.locationAlways.value);
      return status.index;
    });
    final service = TraceletLocationService((_) async {});

    expect(await service.canResumeRecording, isFalse);
    status = PermissionStatus.granted;
    expect(await service.canResumeRecording, isTrue);
    status = PermissionStatus.permanentlyDenied;
    expect(await service.canResumeRecording, isFalse);
  });

  test('an existing background grant needs no permission prompt', () async {
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'checkPermissionStatus');
      expect(call.arguments, Permission.locationAlways.value);
      return PermissionStatus.granted.index;
    });

    expect(await checkAndRequestBackgroundLocationPermission(), isTrue);
  });

  test(
    'a failed permission check is logged and does not allow recording',
    () async {
      final records = <LogRecord>[];
      final subscription = Logger('App').onRecord.listen(records.add);
      addTearDown(subscription.cancel);
      messenger.setMockMethodCallHandler(channel, (call) async {
        throw PlatformException(
          code: 'permission_unavailable',
          message: 'Could not read background location permission',
        );
      });

      expect(await checkAndRequestBackgroundLocationPermission(), isFalse);
      final record = records.single;
      expect(record.message, contains('Requesting background location failed'));
      expect(record.message, contains('permission_unavailable'));
      expect(record.stackTrace, isNotNull);
    },
  );
}
