import 'dart:convert';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/body/onboarding/onboarding_controller.dart';
import 'package:memolanes/body/onboarding/onboarding_overlay.dart';
import 'package:memolanes/body/onboarding/onboarding_page.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:provider/provider.dart';

import 'support/no_location_gps.dart';

const _homeKey = ValueKey('test-home');
const _tour = OnboardingPage(map: SizedBox.expand());
final _skip = find.byKey(const ValueKey('onboarding-skip'));
final _next = find.byKey(const ValueKey('onboarding-next'));
final _card = find.byKey(const ValueKey('onboarding-coach-card'));

class _Translations extends AssetLoader {
  const _Translations();

  @override
  Future<Map<String, dynamic>> load(String path, Locale locale) async =>
      jsonDecode(
        File('$path/${locale.toLanguageTag()}.json').readAsStringSync(),
      ) as Map<String, dynamic>;
}

Future<void> _pumpApp(
  WidgetTester tester, {
  Widget home = const Scaffold(key: _homeKey),
  Locale locale = const Locale('en', 'US'),
  double textScale = 1,
  GpsManager Function() createGps = NoLocationGps.new,
}) async {
  await tester.runAsync(() async {
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: [locale],
        startLocale: locale,
        saveLocale: false,
        path: 'assets/translations',
        assetLoader: const _Translations(),
        child: ChangeNotifierProvider<GpsManager>(
          create: (_) => createGps(),
          child: Builder(
            builder: (context) => MaterialApp(
              theme: AppTheme.light,
              localizationsDelegates: [
                ...GlobalMaterialLocalizations.delegates,
                ...context.localizationDelegates,
              ],
              supportedLocales: context.supportedLocales,
              locale: context.locale,
              builder: (context, child) => MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: TextScaler.linear(textScale)),
                child: child!,
              ),
              home: home,
            ),
          ),
        ),
      ),
    );
  });
  await tester.pumpAndSettle();
}

OnboardingController _controller(WidgetTester tester) =>
    tester.widget<OnboardingOverlay>(find.byType(OnboardingOverlay)).controller;

Future<void> _advanceTo(WidgetTester tester, OnboardingStep step) async {
  final controller = _controller(tester);
  while (controller.step.index < step.index) {
    controller.next();
  }
  await tester.pumpAndSettle();
  expect(controller.step, step);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
  });

  testWidgets('the tour leaves the live journey page and its data alone', (
    tester,
  ) async {
    final api = _EmptyJourneyApi();
    RustLib.initMock(api: api);
    addTearDown(RustLib.dispose);
    await _pumpApp(
      tester,
      createGps: _RecordingGps.new,
      home: AppNavigationShell(
        key: _homeKey,
        navigationBuilder: (_, {required axis}) => const SizedBox.expand(),
        bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
            JourneyOverlay(
              onJourneySelected: (_) async => fail('Unexpected live journey'),
              isLoading: false,
              refreshRevision: 0,
            ),
      ),
    );
    final home = tester.element(find.byKey(_homeKey));
    final emptyState = find.text('No journeys yet');
    final example = find.text('Example journey · For this tour only');
    expect(emptyState, findsOneWidget);
    final reads = api.reads;
    final result = _tour.show(home);
    await tester.pumpAndSettle();
    expect(_controller(tester).step, OnboardingStep.recording);
    expect(find.text(home.tr('home.start_new_journey')), findsOneWidget);
    expect(find.text('REC'), findsOneWidget);
    final recordingControl = _controller(tester).targetBounds;
    expect(recordingControl, isNotNull);
    await tester.tapAt(recordingControl!.center);
    await tester.pumpAndSettle();
    await tester.tap(_next);
    await tester.pumpAndSettle();
    expect(_controller(tester).step, OnboardingStep.mapControls);
    expect(find.byKey(const ValueKey('record-active')), findsOneWidget);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(_controller(tester).step, OnboardingStep.recording);
    await _advanceTo(tester, OnboardingStep.journeys);
    expect(example, findsOneWidget);
    await _advanceTo(tester, OnboardingStep.settings);
    await tester.tap(_next);
    await tester.pumpAndSettle();
    expect(await result, isTrue);
    expect(emptyState, findsOneWidget);
    expect(example, findsNothing);
    expect(api.reads, reads);
    expect(tester.element(find.byKey(_homeKey)), same(home));

    final replay = _tour.show(home);
    await tester.pumpAndSettle();
    expect(_controller(tester).step, OnboardingStep.recording);
    await tester.tap(_skip);
    await tester.pumpAndSettle();
    expect(await replay, isTrue);
  });

  for (final size in [const Size(740, 360), const Size(360, 640)]) {
    testWidgets(
      'calendar remains clear and its sample row is revealed at $size',
      (tester) async {
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = size;
        tester.view.viewPadding = const FakeViewPadding(top: 24, bottom: 16);
        tester.view.systemGestureInsets = const FakeViewPadding(bottom: 24);
        addTearDown(tester.view.reset);
        await _pumpApp(tester, locale: const Locale('zh', 'CN'));
        final result = _tour.show(tester.element(find.byKey(_homeKey)));
        await tester.pumpAndSettle();
        await _advanceTo(tester, OnboardingStep.journeys);
        final controller = _controller(tester);
        final target = tester
            .widget<OnboardingTarget>(find.byType(OnboardingTarget))
            .target!;
        final calendar = find.byKey(target);
        final viewport = RenderAbstractViewport.of(
          tester.renderObject(calendar),
        );
        final visible = MatrixUtils.transformRect(
          viewport.getTransformTo(null),
          viewport.paintBounds,
        );
        expect(
          controller.targetBounds,
          tester.getRect(calendar).intersect(visible),
        );
        expect(
          tester.getRect(_card).overlaps(controller.targetBounds!),
          isFalse,
        );

        await _advanceTo(tester, OnboardingStep.journeyRecord);
        expect(controller.targetBounds, isNotNull);
        controller.previous();
        await tester.pumpAndSettle();
        expect(
          tester.getRect(_card).overlaps(controller.targetBounds!),
          isFalse,
        );
        await tester.tap(_skip);
        await tester.pumpAndSettle();
        expect(await result, isTrue);
      },
    );
  }

  testWidgets('short coach card actions stay reachable after rotation', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(260, 420);
    tester.view.viewPadding = const FakeViewPadding(top: 24, right: 8);
    addTearDown(tester.view.reset);
    await _pumpApp(tester, textScale: 2);
    final result = _tour.show(tester.element(find.byKey(_homeKey)));
    await tester.pumpAndSettle();
    await _advanceTo(tester, OnboardingStep.timeRange);
    tester.view.physicalSize = const Size(420, 260);
    await tester.pumpAndSettle();
    final skipBounds = tester.getRect(_skip);
    expect(_skip.hitTestable(), findsOneWidget);
    await tester.scrollUntilVisible(
      _next.hitTestable(),
      80,
      scrollable: find.descendant(of: _card, matching: find.byType(Scrollable)),
    );
    expect(tester.getRect(_skip), skipBounds);
    await tester.tap(_next);
    await tester.pumpAndSettle();
    expect(_controller(tester).step, OnboardingStep.journeys);
    await tester.tap(_skip);
    await tester.pumpAndSettle();
    expect(await result, isTrue);
  });
}

class _RecordingGps extends NoLocationGps {
  @override
  GpsRecordingStatus get recordingStatus => GpsRecordingStatus.recording;
}

class _EmptyJourneyApi extends Fake implements RustLibApi {
  int reads = 0;

  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async {
    reads++;
    return null;
  }

  @override
  Future<List<DateTime>> crateApiApiJourneyDates({
    Set<JourneyKind>? journeyKinds,
  }) async {
    reads++;
    return [];
  }
}
