import 'dart:async';
import 'dart:ui' show DisplayFeature, DisplayFeatureState, DisplayFeatureType;

import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    as native
    show DuoHardware;
import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/theme/app_theme.dart';

const _hinge = Rect.fromLTWH(468, 0, 24, 640);
const _engineFeatures = [
  DisplayFeature(
    bounds: _hinge,
    type: DisplayFeatureType.hinge,
    state: DisplayFeatureState.postureHalfOpened,
  ),
];
const _duo = <String, Object?>{
  'supported': true,
  'hinge': true,
  'status': 2,
  'angle': 90,
  'bar': 2,
  'regions': [
    {
      'kind': 'division',
      'x': 468.0,
      'y': 0.0,
      'w': 24.0,
      'h': 640.0,
      'active': true,
    },
  ],
};

void main() {
  Future<void> pumpProvider(
    WidgetTester tester,
    Widget home,
    _HardwareChannel channel,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(960, 640);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    channel.install();
    addTearDown(channel.dispose);
    await tester.pumpWidget(
      MaterialApp(
        theme: AppTheme.light,
        builder: (context, child) => Directionality(
          textDirection: TextDirection.ltr,
          child: MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(displayFeatures: _engineFeatures),
            child: DuoDeviceProvider(child: child!),
          ),
        ),
        home: home,
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets(
    'only chrome waits for hardware and a side edge does not wait for the hinge',
    (tester) async {
      final response = Completer<Map<String, Object?>>();
      final channel = _HardwareChannel(_duo, firstResponse: response.future);
      final axes = <Axis>[];
      await pumpProvider(
        tester,
        AppNavigationShell(
          navigationBuilder: (_, {required axis}) {
            axes.add(axis);
            return const SizedBox.expand(key: Key('navigation'));
          },
          bottomTrailingOverlay: const SizedBox(key: Key('attribution')),
          bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
              Stack(
                fit: StackFit.expand,
                children: const [
                  Text('Page content'),
                  MapOverlayPanel.controls(child: Text('Map controls')),
                ],
              ),
        ),
        channel,
      );
      expect(find.text('Page content'), findsOneWidget);
      expect(find.byKey(const Key('navigation')), findsNothing);
      expect(find.byKey(const Key('attribution')), findsNothing);
      expect(find.text('Map controls'), findsNothing);
      expect(axes, isEmpty);
      response.complete({..._duo, 'hinge': false});
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('navigation')), findsOneWidget);
      expect(find.byKey(const Key('attribution')), findsOneWidget);
      expect(find.text('Map controls'), findsOneWidget);
      expect(axes, everyElement(Axis.vertical));
      final context = tester.element(find.text('Page content'));
      expect(MediaQuery.of(context).displayFeatures, isEmpty);
      expect(native.DuoHardware.of(context).folds.single.frame, _hinge);
      expect(channel.snapshots, 1);
      expect(channel.listens, 1);
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();
      expect(channel.cancels, 1);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'a failed initial read releases chrome into the ordinary layout',
    (tester) async {
      final response = Completer<Map<String, Object?>>();
      final channel = _HardwareChannel({}, firstResponse: response.future);
      AppLayout? layout;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            layout = AppLayout.fromContext(context);
            return const SizedBox.expand();
          },
        ),
        channel,
      );
      expect(layout!.chromeReady, isFalse);
      response.completeError(MissingPluginException());
      await tester.pumpAndSettle();
      expect(layout!.chromeReady, isTrue);
      expect(layout!.navigationAxis, Axis.horizontal);
      expect(channel.snapshots, 1);
      expect(channel.listens, 0);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'ordinary devices keep engine features and only iOS subscribes to hardware',
    (tester) async {
      final channel = _HardwareChannel({'supported': true, 'hinge': false});
      List<DisplayFeature>? observed;
      AppLayout? layout;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            observed = MediaQuery.of(context).displayFeatures;
            layout = AppLayout.fromContext(context);
            return const Scaffold();
          },
        ),
        channel,
      );
      expect(observed, _engineFeatures);
      expect(layout!.detachedBackBounds, isNull);
      expect(layout!.chromeReady, isTrue);
      final subscriptions = defaultTargetPlatform == TargetPlatform.iOS ? 1 : 0;
      expect(channel.snapshots, subscriptions);
      expect(channel.listens, subscriptions);
    },
    variant: const TargetPlatformVariant({
      TargetPlatform.iOS,
      TargetPlatform.android,
    }),
  );

  testWidgets(
    'layout consumers skip angle ticks and react to posture changes',
    (tester) async {
      final channel = _HardwareChannel(_duo);
      var builds = 0;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            AppLayout.fromContext(context);
            builds++;
            return const Scaffold();
          },
        ),
        channel,
      );
      final before = builds;
      channel.update({..._duo, 'angle': 95});
      await tester.pumpAndSettle();
      expect(builds, before);
      channel.update({..._duo, 'angle': 180, 'status': 3});
      await tester.pumpAndSettle();
      expect(builds, greaterThan(before));
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'an open modal retains its form draft when hardware detection arrives',
    (tester) async {
      final channel = _HardwareChannel({'supported': true});
      await pumpProvider(
        tester,
        Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () =>
                  showAppDialog(context, builder: (_) => const TextField()),
              child: const Text('Open'),
            ),
          ),
        ),
        channel,
      );
      await tester.tap(find.text('Open'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Unsaved draft');
      for (final hardware in [
        _duo,
        {'supported': true},
      ]) {
        channel.update(hardware);
        await tester.pumpAndSettle();
        expect(find.text('Unsaved draft'), findsOneWidget);
      }
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );
}

class _HardwareChannel {
  _HardwareChannel(this.data, {this.firstResponse});
  Map<String, Object?> data;
  final Future<Map<String, Object?>>? firstResponse;
  MockStreamHandlerEventSink? _sink;
  int snapshots = 0;
  int listens = 0;
  int cancels = 0;
  static const _methods = MethodChannel('duo_dynamic_sizing');
  static const _events = EventChannel('duo_dynamic_sizing/events');

  void install() {
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(_methods, (call) async {
      expect(call.method, 'snapshot');
      snapshots++;
      return firstResponse == null ? data : await firstResponse;
    });
    messenger.setMockStreamHandler(
      _events,
      MockStreamHandler.inline(
        onListen: (_, sink) {
          listens++;
          _sink = sink;
        },
        onCancel: (_) {
          cancels++;
          _sink = null;
        },
      ),
    );
  }

  void update(Map<String, Object?> next) {
    data = next;
    _sink!.success(next);
  }

  void dispose() {
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(_methods, null);
    messenger.setMockStreamHandler(_events, null);
  }
}
