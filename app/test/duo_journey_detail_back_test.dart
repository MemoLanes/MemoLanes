import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/duo_hardware_test_data.dart';

import 'package:memolanes/body/map/journey_flow_controller.dart';
import 'package:memolanes/body/map/overlay/journey_detail_overlay.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/map_glass_back_button.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/src/rust/utils.dart' show MapBounds;
import 'package:memolanes/theme/app_theme.dart';

const _size = Size(678, 466);
const _locale = Locale('en', 'US');
const _loader = AppTranslationLoader();

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
    await _loader.load('assets/translations', _locale);
    RustLib.initMock(api: _MapApi());
  });
  tearDownAll(RustLib.dispose);

  testWidgets(
    'shared panel geometry does not carry a different journey draft',
    (tester) async {
      final panel = GlobalKey<MapOverlayPanelState>();
      final flow = await _openDetail(
        tester,
        DuoBarEdge.unknown,
        const EdgeInsets.only(bottom: 34),
        panelKey: panel,
      );
      flow.editInformation();
      await tester.pumpAndSettle();
      await tester.enterText(
        find.byType(TextField),
        'Draft of the first journey',
      );
      flow.requestBack();
      flow.requestBack();
      await flow.open(
        JourneyHeader(
          id: 'second-journey',
          revision: 'v1',
          journeyDate: DateTime.utc(2026, 10, 13),
          createdAt: DateTime.utc(2026, 10, 13),
          journeyType: JourneyType.vector,
          journeyKind: JourneyKind.defaultKind,
          hasRawData: false,
        ),
        readMapView: () async => null,
      );
      await tester.pumpAndSettle();
      expect(flow.session!.journey.id, 'second-journey');
      flow.editInformation();
      await tester.pumpAndSettle();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        isEmpty,
      );
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets('Duo detail Back receives taps', (tester) async {
    final flow = await _openDetail(
      tester,
      DuoBarEdge.right,
      const EdgeInsets.fromLTRB(0, 0, 84, 34),
    );
    final back = find.byType(MapGlassBackButton);
    expect(back.hitTestable(), findsOneWidget);
    await tester.tap(back);
    await tester.pumpAndSettle();
    expect(flow.phase, JourneyPhase.picker);
    expect(find.text('Picker'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('Duo detail retains edge swipe outside Back', (tester) async {
    final flow = await _openDetail(
      tester,
      DuoBarEdge.right,
      const EdgeInsets.fromLTRB(0, 0, 84, 34),
    );
    await tester.flingFrom(const Offset(10, 240), const Offset(500, 0), 1500);
    await tester.pumpAndSettle();
    expect(flow.phase, JourneyPhase.picker);
    expect(find.text('Picker'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));
}

Future<JourneyFlowController> _openDetail(
  WidgetTester tester,
  DuoBarEdge edge,
  EdgeInsets padding, {
  GlobalKey<MapOverlayPanelState>? panelKey,
}) async {
  tester.view.physicalSize = _size;
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final media = MediaQueryData(
    size: _size,
    padding: padding,
    viewPadding: padding,
  );
  final fold = DuoHardware(
    supported: true,
    hasHinge: true,
    status: DuoHingeStatus.closed,
    angle: 0,
    cameras: [
      DuoRegion(
        Rect.fromLTWH(edge == DuoBarEdge.right ? 594 : 0, 0, 84, 120),
        isActive: true,
      ),
    ],
  );
  final layout = AppLayout.resolve(
    media: media,
    size: _size,
    hardware: fold,
    barEdge: edge,
    platform: TargetPlatform.iOS,
    navigationVisible: false,
  );
  final journey = JourneyHeader(
    id: 'back-hit-test',
    revision: 'v1',
    journeyDate: DateTime.utc(2026, 10, 12),
    createdAt: DateTime.utc(2026, 10, 12),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
    hasRawData: false,
  );
  final flow = JourneyFlowController();
  addTearDown(flow.dispose);
  await flow.open(journey, readMapView: () async => null);
  await tester.runAsync(() async {
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [_locale],
        path: 'assets/translations',
        assetLoader: _loader,
        fallbackLocale: _locale,
        child: Builder(
          builder: (context) => MaterialApp(
            theme: AppTheme.light.copyWith(platform: TargetPlatform.iOS),
            locale: context.locale,
            localizationsDelegates: context.localizationDelegates,
            supportedLocales: context.supportedLocales,
            home: MediaQuery(
              data: media,
              child: AppLayoutScope(
                layout: layout,
                child: Scaffold(
                  body: ListenableBuilder(
                    listenable: flow,
                    builder: (context, _) => flow.session == null
                        ? const Text('Picker')
                        : JourneyDetailOverlay(
                            key: ValueKey(flow.session!.journey.id),
                            controller: flow,
                            panelKey: panelKey,
                          ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 1));
  });
  await tester.pumpAndSettle();
  return flow;
}

class _MapApi extends Fake implements RustLibApi {
  @override
  Future<(api.MapRendererProxy, MapBounds?)>
  crateApiApiGetMapRendererProxyForJourney({required String journeyId}) async =>
      (_MapProxy(), null);
}

class _MapProxy extends Fake implements api.MapRendererProxy {}
