import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/onboarding_overlay.dart';
import 'package:memolanes/common/onboarding.dart';
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

class _Translations extends AssetLoader {
  const _Translations();

  @override
  Future<Map<String, dynamic>> load(String path, Locale locale) async =>
      jsonDecode(
        File('$path/${locale.toLanguageTag()}.json').readAsStringSync(),
      ) as Map<String, dynamic>;
}

Widget _app(
  OnboardingController controller, {
  required VoidCallback onTargetTap,
  Locale locale = const Locale('en', 'US'),
  double textScale = 1,
  bool showTarget = true,
  Size targetSize = const Size(48, 48),
  Widget? body,
}) => EasyLocalization(
  supportedLocales: [locale],
  startLocale: locale,
  saveLocale: false,
  path: 'assets/translations',
  assetLoader: const _Translations(),
  child: Builder(
    builder: (context) => MaterialApp(
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: textScale > 1 ? ThemeMode.dark : ThemeMode.light,
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
      home: RepaintBoundary(
        key: const Key('tour-scene'),
        child: OnboardingScope(
          controller: controller,
          child: ListenableBuilder(
            listenable: controller,
            builder: (context, _) => AppNavigationShell(
              foregroundOverlay: controller.isActive
                  ? OnboardingOverlay(controller: controller)
                  : null,
              navigationBuilder: (_, {required axis}) =>
                  const SizedBox.expand(),
              bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
                  body ??
                  ColoredBox(
                    color: const Color(0xFF345678),
                    child: Align(
                      alignment: const Alignment(0.8, 0.4),
                      child: showTarget
                          ? OnboardingTarget(
                              anchor: controller.step.anchor,
                              child: SizedBox.fromSize(
                                key: const Key('real-control'),
                                size: targetSize,
                                child: GestureDetector(
                                  onTap: onTargetTap,
                                  child: const ColoredBox(color: Colors.green),
                                ),
                              ),
                            )
                          : const SizedBox.shrink(),
                    ),
                  ),
            ),
          ),
        ),
      ),
    ),
  ),
);

Future<void> _pump(WidgetTester tester, Widget app) async {
  await tester.runAsync(() async {
    await tester.pumpWidget(app);
  });
  await tester.pumpAndSettle();
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

  test(
    'tour visits all five tabs, can go back, and restarts after skipping',
    () async {
      final tabs = <int>[];
      final controller = OnboardingController(selectTab: tabs.add);
      addTearDown(controller.dispose);
      final first = controller.start();
      expect(controller.start(), same(first));
      for (var i = 1; i < OnboardingController.steps.length; i++) {
        controller.next();
      }
      expect(tabs, [0, 0, 1, 2, 2, 3, 4]);
      controller.previous();
      expect(tabs.last, 3);
      controller.finish();
      expect(await first, isTrue);
      expect(controller.isActive, isFalse);
      final replay = controller.start();
      expect(controller.index, 0);
      controller.finish();
      expect(await replay, isTrue);
    },
  );

  test('disposal does not count an interrupted tour as completed', () async {
    final controller = OnboardingController(selectTab: (_) {});
    final result = controller.start();
    controller.dispose();
    expect(await result, isFalse);
  });

  testWidgets(
    'measures actual controls, blocks taps through holes, and skips',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(393, 852);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      var taps = 0;
      final controller = OnboardingController(selectTab: (_) {});
      addTearDown(controller.dispose);
      await _pump(tester, _app(controller, onTargetTap: () => taps++));
      final result = controller.start();
      await tester.pumpAndSettle();
      expect(
        controller.targetBounds,
        tester.getRect(find.byKey(const Key('real-control'))),
      );
      expect(find.text('Start a journey'), findsOneWidget);
      expect(find.text('REC'), findsOneWidget);
      expect(
        find.textContaining('REC appears in the upper left'),
        findsOneWidget,
      );
      expect(find.textContaining('Loading this screen'), findsNothing);
      await tester.tapAt(controller.targetBounds!.center);
      expect(taps, 0);
      await tester.tap(find.text('Next'));
      await tester.pumpAndSettle();
      expect(find.text('Map tools'), findsOneWidget);
      expect(
        controller.targetBounds,
        tester.getRect(find.byKey(const Key('real-control'))),
      );
      await tester.tap(find.text('Back'));
      await tester.pumpAndSettle();
      expect(controller.index, 0);
      await tester.tap(find.text('Skip'));
      await tester.pumpAndSettle();
      expect(await result, isTrue);
      expect(find.byType(OnboardingOverlay), findsNothing);
      await tester.tap(find.byKey(const Key('real-control')));
      expect(taps, 1);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'large real panels stay visible through the mask and clear of the coach card',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(393, 852);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final controller = OnboardingController(selectTab: (_) {});
      addTearDown(controller.dispose);
      controller.start();
      await _pump(
        tester,
        _app(controller, onTargetTap: () {}, targetSize: const Size(320, 400)),
      );
      final target = tester.getRect(find.byKey(const Key('real-control')));
      final card = tester.getRect(
        find.byKey(const ValueKey('onboarding-coach-card')),
      );
      expect(card.overlaps(target.inflate(6)), isFalse);
      await tester.runAsync(() async {
        final boundary = tester.renderObject<RenderRepaintBoundary>(
          find.byKey(const Key('tour-scene')),
        );
        final image = await boundary.toImage(pixelRatio: 1);
        final bytes = (await image.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        ))!;
        final index =
            (target.center.dy.floor() * image.width +
                target.center.dx.floor()) *
            4;
        // The original control remains fully visible, while the rest dims.
        final green = Colors.green.toARGB32();
        expect(bytes.getUint8(index), (green >> 16) & 0xff);
        expect(bytes.getUint8(index + 1), (green >> 8) & 0xff);
        expect(bytes.getUint8(index + 2), green & 0xff);
        final outside = (2 * image.width + 2) * 4;
        expect(bytes.getUint8(outside), lessThan(0x34));
        expect(bytes.getUint8(outside + 1), lessThan(0x56));
        expect(bytes.getUint8(outside + 2), lessThan(0x78));
        image.dispose();
      });
      controller.finish();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'all chapters keep actions reachable after rotation and with large text',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      for (final locale in [
        const Locale('en', 'US'),
        const Locale('zh', 'CN'),
      ]) {
        final controller = OnboardingController(selectTab: (_) {});
        final result = controller.start();
        for (final size in [const Size(320, 640), const Size(740, 360)]) {
          tester.view.physicalSize = size;
          await _pump(
            tester,
            _app(controller, onTargetTap: () {}, locale: locale, textScale: 2),
          );
          final positions = {
            for (final key in [
              'onboarding-skip',
              'onboarding-previous',
              'onboarding-next',
            ])
              key: tester.getRect(find.byKey(ValueKey(key))),
          };
          expect(
            tester
                .getTopLeft(find.byKey(const ValueKey('onboarding-toolbar')))
                .dy,
            lessThan(40),
          );
          for (var i = 0; i < OnboardingController.steps.length; i++) {
            for (final entry in positions.entries) {
              expect(
                tester.getRect(find.byKey(ValueKey(entry.key))),
                entry.value,
              );
            }
            final isChinese = locale.languageCode == 'zh';
            expect(
              find.text(isChinese ? '跳过' : 'Skip').hitTestable(),
              findsOneWidget,
            );
            final next = find.widgetWithText(
              AppButton,
              i == OnboardingController.steps.length - 1
                  ? (isChinese ? '完成' : 'Done')
                  : (isChinese ? '下一步' : 'Next'),
            );
            expect(next.hitTestable(), findsOneWidget);
            expect(tester.takeException(), isNull);
            if (i < OnboardingController.steps.length - 1) {
              await tester.tap(next);
              await tester.pumpAndSettle();
            }
          }
          for (var i = 1; i < OnboardingController.steps.length; i++) {
            controller.previous();
          }
          await tester.pumpAndSettle();
        }
        controller.finish();
        await tester.pumpAndSettle();
        expect(await result, isTrue);
        await tester.pumpWidget(const SizedBox.shrink());
        controller.dispose();
      }
    },
  );

  for (final hasJourneys in [true, false]) {
    testWidgets(
      'journey row guide scrolls to ${hasJourneys ? 'a real record' : 'a temporary example'}',
      (tester) async {
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = const Size(393, 852);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPhysicalSize);
        RustLib.initMock(api: _JourneyListApi(hasJourneys));
        addTearDown(RustLib.dispose);
        final controller = OnboardingController(selectTab: (_) {});
        addTearDown(controller.dispose);
        controller.start();
        while (controller.step.anchor != OnboardingAnchor.journeys) {
          controller.next();
        }
        await _pump(
          tester,
          _app(
            controller,
            onTargetTap: () {},
            body: JourneyOverlay(
              onJourneySelected: (_) async =>
                  fail('The tour must not open a journey'),
              isLoading: false,
              refreshRevision: 0,
            ),
          ),
        );
        controller.next();
        await tester.pumpAndSettle();
        expect(controller.step.anchor, OnboardingAnchor.journeyRecord);
        final row = controller.targetBounds;
        expect(row, isNotNull);
        final viewport = tester.getRect(find.byType(CustomScrollView));
        expect(row!.top, greaterThanOrEqualTo(viewport.top));
        expect(row.bottom, lessThanOrEqualTo(viewport.bottom));
        expect(find.textContaining('Loading this screen'), findsNothing);
        expect(
          find.text('Example journey · For this tour only'),
          hasJourneys ? findsNothing : findsOneWidget,
        );
        await tester.tapAt(row.center);
        await tester.pumpAndSettle();
        controller.previous();
        await tester.pumpAndSettle();
        expect(controller.step.anchor, OnboardingAnchor.journeys);
        expect(controller.targetBounds, isNotNull);
        expect(find.text('Example journey · For this tour only'), findsNothing);
        controller.finish();
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
    );
  }

  testWidgets('unavailable targets never prevent skipping the tour', (
    tester,
  ) async {
    final controller = OnboardingController(selectTab: (_) {});
    addTearDown(controller.dispose);
    final result = controller.start();
    await _pump(
      tester,
      _app(controller, onTargetTap: () {}, showTarget: false),
    );
    expect(find.textContaining('Loading this screen'), findsOneWidget);
    await tester.tap(find.text('Skip'));
    await tester.pumpAndSettle();
    expect(await result, isTrue);
  });
}

class _JourneyListApi extends Fake implements RustLibApi {
  _JourneyListApi(this.hasJourneys);

  final bool hasJourneys;
  final date = DateTime.utc(
    DateTime.now().year,
    DateTime.now().month,
    DateTime.now().day,
  );

  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async =>
      hasJourneys ? date : null;

  @override
  Future<List<DateTime>> crateApiApiJourneyDates({
    Set<JourneyKind>? journeyKinds,
  }) async => hasJourneys ? [date] : [];

  @override
  Future<List<JourneyHeader>> crateApiApiListJourneysOnDate({
    required int year,
    required int month,
    required int day,
    Set<JourneyKind>? journeyKinds,
  }) async => hasJourneys
      ? [
          JourneyHeader(
            id: 'real-journey',
            revision: 'revision',
            journeyDate: date,
            createdAt: date,
            start: date.add(const Duration(hours: 9)),
            journeyType: JourneyType.vector,
            journeyKind: JourneyKind.defaultKind,
            hasRawData: false,
          ),
        ]
      : [];
}
