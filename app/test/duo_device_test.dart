import 'dart:ui' show DisplayFeature, DisplayFeatureState, DisplayFeatureType;

import 'package:duo_dynamic_sizing/duo_dynamic_sizing.dart'
    as native
    show DuoHardware;
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/app_dialog.dart';
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
    _HardwareChannel channel, {
    List<DisplayFeature> engineFeatures = _engineFeatures,
  }) async {
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
                .copyWith(displayFeatures: engineFeatures),
            child: DuoDeviceProvider(child: child!),
          ),
        ),
        home: home,
      ),
    );
    await tester.pumpAndSettle();
  }

  testWidgets(
    'actual Duo masks engine splitting and keeps native hardware above Navigator',
    (tester) async {
      final channel = _HardwareChannel(_duo);
      List<DisplayFeature>? observed;
      native.DuoHardware? hardware;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            observed = MediaQuery.of(context).displayFeatures;
            hardware = native.DuoHardware.of(context);
            return const Scaffold();
          },
        ),
        channel,
      );
      expect(observed, isEmpty);
      expect(hardware!.folds.single.frame, _hinge);
      expect(channel.snapshots, 1);
      expect(channel.listens, 1);
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pumpAndSettle();
      expect(channel.cancels, 1);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets('ordinary devices without a hinge keep engine features', (
    tester,
  ) async {
    final channel = _HardwareChannel({
      ..._duo,
      'supported': true,
      'hinge': false,
    });
    List<DisplayFeature>? observed;
    AppLayout? layout;
    DuoBarEdge? edge;
    await pumpProvider(
      tester,
      Builder(
        builder: (context) {
          observed = MediaQuery.of(context).displayFeatures;
          layout = AppLayout.fromContext(context);
          edge = DuoBarScope.of(context);
          return const Scaffold();
        },
      ),
      channel,
    );
    expect(observed, _engineFeatures);
    expect(layout!.detachedBackBounds, isNull);
    expect(edge, DuoBarEdge.unknown);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets(
    'Android keeps ordinary features and does not subscribe to native hardware',
    (tester) async {
      final channel = _HardwareChannel(_duo);
      List<DisplayFeature>? observed;
      native.DuoHardware? hardware;
      AppLayout? layout;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            observed = MediaQuery.of(context).displayFeatures;
            hardware = native.DuoHardware.of(context);
            layout = AppLayout.fromContext(context);
            return const Scaffold();
          },
        ),
        channel,
      );
      expect(observed, _engineFeatures);
      expect(hardware, native.DuoHardware.none);
      expect(layout!.detachedBackBounds, isNull);
      expect(channel.snapshots, 0);
      expect(channel.listens, 0);
    },
    variant: const TargetPlatformVariant({TargetPlatform.android}),
  );

  testWidgets(
    'missing hardware detection leaves the framework fallback untouched',
    (tester) async {
      final channel = _HardwareChannel({'supported': true});
      DuoBarEdge? edge;
      List<DisplayFeature>? observed;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) {
            edge = DuoBarScope.of(context);
            observed = MediaQuery.of(context).displayFeatures;
            return const Scaffold();
          },
        ),
        channel,
      );
      expect(edge, DuoBarEdge.unknown);
      expect(observed, _engineFeatures);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
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
      var initializations = 0;
      await pumpProvider(
        tester,
        Builder(
          builder: (context) => Scaffold(
            body: TextButton(
              onPressed: () => showAppDialog(
                context,
                builder: (_) => _DraftField(onInit: () => initializations++),
              ),
              child: const Text('Open'),
            ),
          ),
        ),
        channel,
      );
      await tester.tap(find.text('Open'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Unsaved draft');
      channel.update(_duo);
      await tester.pumpAndSettle();
      expect(initializations, 1);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'Unsaved draft',
      );
      expect(
        tester.getRect(find.byType(TextField)).left,
        greaterThanOrEqualTo(_hinge.right),
      );
      channel.update({'supported': true});
      await tester.pumpAndSettle();
      expect(initializations, 1);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        'Unsaved draft',
      );
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );
}

class _HardwareChannel {
  _HardwareChannel(this.data);
  Map<String, Object?> data;
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
      return data;
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

class _DraftField extends StatefulWidget {
  const _DraftField({required this.onInit});
  final VoidCallback onInit;
  @override
  State<_DraftField> createState() => _DraftFieldState();
}

class _DraftFieldState extends State<_DraftField> {
  final _controller = TextEditingController();
  @override
  void initState() {
    super.initState();
    widget.onInit();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => TextField(controller: _controller);
}
