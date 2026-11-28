import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/src/rust/frb_generated.dart';

class _RecordingApi extends Fake implements RustLibApi {
  int checks = 0;
  bool canFinalize = false;

  @override
  Future<bool> crateApiApiHasOngoingJourney() async => true;

  @override
  Future<bool> crateApiApiTryAutoFinalizeJourney() async {
    checks++;
    return canFinalize;
  }
}

void main() {
  testWidgets('resuming defers and coalesces checks for a paused journey', (
    tester,
  ) async {
    final api = _RecordingApi();
    RustLib.initMock(api: api);
    addTearDown(RustLib.dispose);
    const toast = MethodChannel('PonnamKarthik/fluttertoast');
    final messenger = tester.binding.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(toast, (_) async => true);
    addTearDown(() => messenger.setMockMethodCallHandler(toast, null));
    final manager = GpsManager(wasRecording: false);
    manager.readyToStart();
    await tester.pump();
    expect(manager.recordingStatus, GpsRecordingStatus.paused);
    expect(api.checks, 1);

    Future<void> resumeAfterTenMinutes() async {
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      await tester.pump(const Duration(minutes: 10));
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      await tester.pump();
    }

    await resumeAfterTenMinutes();
    await tester.pump(const Duration(seconds: 4));
    expect(api.checks, 1);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pump(const Duration(seconds: 1));
    expect(api.checks, 2);
    expect(manager.recordingStatus, GpsRecordingStatus.paused);
    await tester.pump(const Duration(seconds: 5));
    expect(api.checks, 2);

    api.canFinalize = true;
    await resumeAfterTenMinutes();
    expect(api.checks, 2);
    await tester.pump(const Duration(seconds: 5));
    expect(api.checks, 3);
    expect(manager.recordingStatus, GpsRecordingStatus.none);

    await resumeAfterTenMinutes();
    expect(api.checks, 3);
    manager.dispose();
    await tester.pump();
  });
}
