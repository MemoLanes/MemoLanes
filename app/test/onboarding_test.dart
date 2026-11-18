import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
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
  Alignment targetAlignment = const Alignment(0.8, 0.4),
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
                      alignment: targetAlignment,
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
    'the coach card fits above a tall calendar without covering its spotlight',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(407, 904);
      tester.view.viewPadding = const FakeViewPadding(top: 39, bottom: 16);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetViewPadding);
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
          locale: const Locale('zh', 'CN'),
          targetSize: const Size(350, 280),
          targetAlignment: Alignment.center,
        ),
      );
      final target = tester.getRect(find.byKey(const Key('real-control')));
      final card = tester.getRect(
        find.byKey(const ValueKey('onboarding-coach-card')),
      );
      expect(card.bottom, lessThan(target.top));
      final progress = tester.getRect(find.text('旅程 · 4 / 7'));
      final previous = tester.getRect(
        find.byKey(const ValueKey('onboarding-previous')),
      );
      final next = find.byKey(const ValueKey('onboarding-next'));
      final title = tester.getRect(find.text('选择旅程日期'));
      expect(progress.left, closeTo(title.left, 0.01));
      expect(progress.right, lessThan(previous.left));
      expect(progress.center.dy, closeTo(tester.getCenter(next).dy, 0.01));
      expect(
        tester
            .getSize(find.descendant(of: next, matching: find.byType(Material)))
            .height,
        28,
      );
      expect(
        tester
            .state<ScrollableState>(find.byType(Scrollable))
            .position
            .maxScrollExtent,
        0,
      );
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
    'card actions and fixed skip stay reachable after rotation and with large text',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      const insets = FakeViewPadding(top: 24, right: 8);
      tester.view.viewPadding = insets;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetViewPadding);
      final card = find.byKey(const ValueKey('onboarding-coach-card'));
      final skip = find.byKey(const ValueKey('onboarding-skip'));
      final previous = find.byKey(const ValueKey('onboarding-previous'));
      final next = find.byKey(const ValueKey('onboarding-next'));
      final copy = find.descendant(of: card, matching: find.byType(Scrollable));
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
          final skipBounds = tester.getRect(skip);
          expect(skipBounds.top, insets.top + 16);
          expect(skipBounds.right, size.width - insets.right - 16);
          for (var i = 0; i < OnboardingController.steps.length; i++) {
            final isChinese = locale.languageCode == 'zh';
            final last = i == OnboardingController.steps.length - 1;
            final cardBounds = tester.getRect(card);
            final previousBounds = tester.getRect(previous);
            final nextBounds = tester.getRect(next);
            expect(tester.getRect(skip), skipBounds);
            expect(cardBounds.overlaps(skipBounds), isFalse);
            for (final bounds in [previousBounds, nextBounds]) {
              expect(cardBounds.contains(bounds.topLeft), isTrue);
              expect(cardBounds.contains(bounds.bottomRight), isTrue);
              expect(
                bounds.width,
                greaterThanOrEqualTo(kMinInteractiveDimension),
              );
              expect(
                bounds.height,
                greaterThanOrEqualTo(kMinInteractiveDimension),
              );
            }
            expect(previousBounds.right, lessThan(nextBounds.left));
            expect(previousBounds.center.dy, nextBounds.center.dy);
            expect(
              tester.widget<TextButton>(previous).onPressed == null,
              i == 0,
            );
            expect(
              find.text(isChinese ? '跳过' : 'Skip').hitTestable(),
              findsOneWidget,
            );
            expect(
              find.descendant(
                of: next,
                matching: find.text(
                  last
                      ? (isChinese ? '完成' : 'Done')
                      : (isChinese ? '下一步' : 'Next'),
                ),
              ),
              findsOneWidget,
            );
            expect(next.hitTestable(), findsOneWidget);
            if (i == 1 && size.width > size.height) {
              await tester.drag(copy, const Offset(0, -100));
              await tester.pumpAndSettle();
              expect(
                tester.state<ScrollableState>(copy).position.pixels,
                greaterThan(0),
              );
              expect(tester.getRect(next), nextBounds);
              expect(tester.getRect(skip), skipBounds);
            }
            expect(tester.takeException(), isNull);
            if (!last) {
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

  testWidgets('short coach card scrolls to its actions with large text', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(420, 260);
    tester.view.viewPadding = const FakeViewPadding(top: 24, right: 8);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetViewPadding);
    final controller = OnboardingController(selectTab: (_) {});
    addTearDown(controller.dispose);
    controller.start();
    while (controller.step.anchor != OnboardingAnchor.timeRange) {
      controller.next();
    }
    await _pump(
      tester,
      _app(
        controller,
        onTargetTap: () {},
        textScale: 2,
        targetSize: const Size(240, 76),
      ),
    );
    final card = find.byKey(const ValueKey('onboarding-coach-card'));
    final skip = find.byKey(const ValueKey('onboarding-skip'));
    final next = find.byKey(const ValueKey('onboarding-next'));
    final scrollable = find.descendant(
      of: card,
      matching: find.byType(Scrollable),
    );
    final skipBounds = tester.getRect(skip);
    expect(next.hitTestable(), findsNothing);
    await tester.scrollUntilVisible(
      next.hitTestable(),
      80,
      scrollable: scrollable,
    );
    expect(
      tester.getRect(card).contains(tester.getRect(next).bottomRight),
      isTrue,
    );
    expect(tester.getRect(skip), skipBounds);
    await tester.tap(next);
    await tester.pumpAndSettle();
    expect(controller.step.anchor, OnboardingAnchor.journeys);
    expect(tester.takeException(), isNull);
    controller.finish();
    await tester.pumpAndSettle();
  });

  testWidgets('calendar spotlight stays within the visible scroll area', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(420, 420);
    tester.view.viewPadding = const FakeViewPadding(top: 24, bottom: 16);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetViewPadding);
    RustLib.initMock(api: _JourneyListApi(false));
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
        locale: const Locale('zh', 'CN'),
        body: JourneyOverlay(
          onJourneySelected: (_) async =>
              fail('The tour must not open a journey'),
          isLoading: false,
          refreshRevision: 0,
        ),
      ),
    );
    final calendarTarget = find.byWidgetPredicate(
      (widget) =>
          widget is OnboardingTarget &&
          widget.anchor == OnboardingAnchor.journeys,
    );
    for (final size in [const Size(420, 420), const Size(740, 360)]) {
      tester.view.physicalSize = size;
      await tester.pumpAndSettle();
      final viewport = RenderAbstractViewport.of(
        tester.renderObject(calendarTarget),
      );
      final visibleArea = MatrixUtils.transformRect(
        viewport.getTransformTo(null),
        viewport.paintBounds,
      );
      final calendar = tester.getRect(calendarTarget);
      expect(calendar.bottom, greaterThan(visibleArea.bottom));
      expect(controller.targetBounds, calendar.intersect(visibleArea));
      expect(tester.takeException(), isNull);
    }
    controller.finish();
    await tester.pumpAndSettle();
  });

  for (final size in [const Size(740, 360), const Size(360, 640)]) {
    testWidgets('date guide card keeps the calendar visible at $size', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = size;
      tester.view.viewPadding = const FakeViewPadding(top: 24, bottom: 16);
      tester.view.systemGestureInsets = const FakeViewPadding(bottom: 24);
      addTearDown(tester.view.reset);
      RustLib.initMock(api: _JourneyListApi(false));
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
          locale: const Locale('zh', 'CN'),
          body: JourneyOverlay(
            onJourneySelected: (_) async =>
                fail('The tour must not open a journey'),
            isLoading: false,
            refreshRevision: 0,
          ),
        ),
      );
      final card = find.byKey(const ValueKey('onboarding-coach-card'));
      final calendar = controller.targetBounds;
      expect(calendar, isNotNull);
      expect(tester.getRect(card).overlaps(calendar!), isFalse);
    });
  }

  for (final hasJourneys in [true, false]) {
    testWidgets(
      'journey guide uses ${hasJourneys ? 'real records' : 'isolated calendar and record examples'}',
      (tester) async {
        tester.view.devicePixelRatio = 1;
        tester.view.physicalSize = const Size(393, 852);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPhysicalSize);
        RustLib.initMock(api: _JourneyListApi(hasJourneys));
        addTearDown(RustLib.dispose);
        final controller = OnboardingController(selectTab: (_) {});
        addTearDown(controller.dispose);
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
        final calendar = find.byType(CalendarDatePicker2);
        final example = find.text('Example journey · For this tour only');
        final recordsViewport = find.byType(CustomScrollView);
        void expectNormalJourneys() {
          expect(example, findsNothing);
          expect(calendar, hasJourneys ? findsOneWidget : findsNothing);
          if (!hasJourneys) {
            expect(find.text('No journeys yet'), findsOneWidget);
          }
        }

        expectNormalJourneys();
        controller.start();
        while (controller.step.anchor != OnboardingAnchor.journeys) {
          controller.next();
        }
        await tester.pumpAndSettle();
        expect(calendar, findsOneWidget);
        final calendarTarget = find.byWidgetPredicate(
          (widget) =>
              widget is OnboardingTarget &&
              widget.anchor == OnboardingAnchor.journeys,
        );
        expect(
          find.descendant(of: calendarTarget, matching: calendar),
          findsOneWidget,
        );
        expect(controller.targetBounds, tester.getRect(calendarTarget));
        expect(example, hasJourneys ? findsNothing : findsOneWidget);
        if (!hasJourneys) {
          expect(
            tester.widget<CalendarDatePicker2>(calendar).value,
            hasLength(1),
          );
          expect(
            tester.getRect(recordsViewport).contains(tester.getCenter(example)),
            isTrue,
          );
        }
        controller.previous();
        await tester.pumpAndSettle();
        expectNormalJourneys();
        controller.next();
        await tester.pumpAndSettle();
        controller.next();
        await tester.pumpAndSettle();
        expect(controller.step.anchor, OnboardingAnchor.journeyRecord);
        final row = controller.targetBounds;
        expect(row, isNotNull);
        final viewport = tester.getRect(recordsViewport);
        expect(row!.top, greaterThanOrEqualTo(viewport.top));
        expect(row.bottom, lessThanOrEqualTo(viewport.bottom));
        expect(find.textContaining('Loading this screen'), findsNothing);
        expect(example, hasJourneys ? findsNothing : findsOneWidget);
        await tester.tapAt(row.center);
        await tester.pumpAndSettle();
        controller.previous();
        await tester.pumpAndSettle();
        expect(controller.step.anchor, OnboardingAnchor.journeys);
        expect(controller.targetBounds, isNotNull);
        expect(example, hasJourneys ? findsNothing : findsOneWidget);
        controller.finish();
        await tester.pumpAndSettle();
        expectNormalJourneys();
        controller.start();
        while (controller.step.anchor != OnboardingAnchor.journeyRecord) {
          controller.next();
        }
        await tester.pumpAndSettle();
        expect(example, hasJourneys ? findsNothing : findsOneWidget);
        controller.finish();
        await tester.pumpAndSettle();
        expectNormalJourneys();
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
      : fail('The example journey must not query stored records');
}
