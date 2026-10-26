import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/map_controls/screenshot_button.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/screenshot.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/src/rust/journey_header.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('zh', 'CN');
  const loader = AppTranslationLoader();
  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
    await loader.load('assets/translations', locale);
  });
  var captures = 0;

  Widget app({bool enabled = true, Future<void> Function()? onCapture}) =>
      Scaffold(
        body: Center(
          child: MapScreenshotButton(
            onPressed: !enabled
                ? null
                : onCapture ??
                      () async {
                        captures++;
                      },
          ),
        ),
      );

  Future<void> pumpApp(WidgetTester tester, Widget home) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale, Locale('en', 'US')],
          path: 'assets/translations',
          assetLoader: loader,
          fallbackLocale: locale,
          startLocale: locale,
          saveLocale: false,
          child: Builder(
            builder: (context) => MaterialApp(
              theme: AppTheme.light,
              localizationsDelegates: [
                ...GlobalMaterialLocalizations.delegates,
                ...context.localizationDelegates,
              ],
              supportedLocales: context.supportedLocales,
              locale: context.locale,
              home: home,
            ),
          ),
        ),
      );
      await tester.pump();
    });
    await tester.pump();
  }

  setUp(() => captures = 0);

  Future<void> waitForLogo(
    WidgetTester tester, {
    String label = 'Logo 水印',
  }) async {
    final logoOption = find.widgetWithText(FilterChip, label);
    await tester.runAsync(() async {
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (tester.widget<FilterChip>(logoOption).onSelected == null &&
          DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 10));
        await tester.pump();
      }
    });
    expect(tester.widget<FilterChip>(logoOption).onSelected, isNotNull);
  }

  testWidgets('watermark follows app language and retains visibility choice', (
    tester,
  ) async {
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
    picture.dispose();
    addTearDown(map.dispose);
    await pumpApp(
      tester,
      ScreenshotPreviewPage(map: map, logicalSize: const Size(320, 640)),
    );
    await waitForLogo(tester);
    final page = find.byType(ScreenshotPreviewPage);
    final state = tester.state(page);

    Future<void> expectLogo(String asset) async {
      await tester.runAsync(() async {
        final data = await rootBundle.load(asset);
        final expected = await decodeScreenshot(
          data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes),
        );
        try {
          final actual = tester
              .widget<RawImage>(find.byType(RawImage).last)
              .image!;
          expect(actual.width, expected.width);
          expect(actual.height, expected.height);
          final expectedPixels = (await expected.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          ))!;
          final actualPixels = (await actual.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          ))!;
          expect(
            actualPixels.buffer.asUint8List(
              actualPixels.offsetInBytes,
              actualPixels.lengthInBytes,
            ),
            orderedEquals(
              expectedPixels.buffer.asUint8List(
                expectedPixels.offsetInBytes,
                expectedPixels.lengthInBytes,
              ),
            ),
          );
          // This part of the old icon's square backdrop must be transparent.
          expect(
            expectedPixels.getUint8((50 * expected.width + 80) * 4 + 3),
            0,
          );
        } finally {
          expected.dispose();
        }
      });
    }

    await expectLogo('assets/screenshot_logo_zh.png');
    await tester.tap(find.widgetWithText(FilterChip, 'Logo 水印'));
    await tester.pumpAndSettle();
    await tester.runAsync(
      () => tester.element(page).setLocale(const Locale('en', 'US')),
    );
    await tester.pumpAndSettle();
    await waitForLogo(tester, label: 'Watermark');
    final englishOption = find.widgetWithText(FilterChip, 'Watermark');
    expect(tester.state(page), same(state));
    expect(tester.widget<FilterChip>(englishOption).selected, isFalse);
    expect(find.byType(RawImage), findsOneWidget);
    await tester.tap(englishOption);
    await tester.pumpAndSettle();
    await expectLogo('assets/screenshot_logo_en.png');

    await tester.runAsync(() => tester.element(page).setLocale(locale));
    await tester.pumpAndSettle();
    await waitForLogo(tester);
    expect(tester.widget<FilterChip>(find.byType(FilterChip)).selected, isTrue);
    await expectLogo('assets/screenshot_logo_zh.png');
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets(
    'journey preview can independently hide its card and default logo',
    (tester) async {
      final recorder = ui.PictureRecorder();
      Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
      final picture = recorder.endRecording();
      final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
      picture.dispose();
      final journey = JourneyHeader(
        id: 'screenshot-journey',
        revision: '1',
        journeyDate: DateTime.utc(2026, 10, 19),
        createdAt: DateTime.utc(2026, 10, 19),
        journeyType: JourneyType.vector,
        journeyKind: JourneyKind.defaultKind,
        hasRawData: false,
      );
      await pumpApp(
        tester,
        ScreenshotPreviewPage(
          map: map,
          logicalSize: const Size(320, 640),
          journey: journey,
        ),
      );
      await waitForLogo(tester);
      final cardOption = find.widgetWithText(FilterChip, '旅程信息');
      final logoOption = find.widgetWithText(FilterChip, 'Logo 水印');
      expect(tester.widget<FilterChip>(cardOption).selected, isTrue);
      expect(tester.widget<FilterChip>(logoOption).selected, isTrue);
      expect(find.byType(ReadOnlyJourneyInfoCard), findsOneWidget);
      expect(find.byType(RawImage), findsNWidgets(2));
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(map),
      );

      await tester.tap(cardOption);
      await tester.pumpAndSettle();
      expect(find.byType(ReadOnlyJourneyInfoCard), findsNothing);
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(map),
      );
      expect(find.byType(RawImage), findsNWidgets(2));
      await tester.tap(logoOption);
      await tester.pumpAndSettle();
      expect(find.byType(RawImage), findsOneWidget);
      await tester.tap(cardOption);
      await tester.pumpAndSettle();
      expect(find.byType(ReadOnlyJourneyInfoCard), findsOneWidget);
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(map),
      );
      expect(tester.widget<FilterChip>(logoOption).selected, isFalse);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      map.dispose();
    },
  );

  testWidgets('preview retains export resolution and prioritizes save action', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(320, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
    picture.dispose();
    await pumpApp(
      tester,
      ScreenshotPreviewPage(map: map, logicalSize: const Size(320, 640)),
    );
    await waitForLogo(tester);
    expect(tester.widget<FilterChip>(find.byType(FilterChip)).selected, isTrue);
    expect(find.widgetWithText(FilterChip, '旅程信息'), findsNothing);
    final actions = find.byType(AppButton);
    expect(actions, findsNWidgets(2));
    expect(tester.widget<AppButton>(actions.first).label, '保存到相册');
    expect(
      tester.getSize(actions.first).width,
      greaterThan(tester.getSize(actions.last).width),
    );
    final saveLabel = find.descendant(
      of: find.text('保存到相册'),
      matching: find.byType(RichText),
    );
    expect(
      tester.renderObject<RenderParagraph>(saveLabel).didExceedMaxLines,
      isFalse,
    );

    final boundaryFinder = find
        .ancestor(
          of: find.byType(RawImage).first,
          matching: find.byType(RepaintBoundary),
        )
        .first;
    final boundary = tester.renderObject<RenderRepaintBoundary>(boundaryFinder);
    expect(boundary.size, const Size(320, 640));
    final exported = (await tester.runAsync(
      () => boundary.toImage(pixelRatio: 2),
    ))!;
    expect(exported.width, 640);
    expect(exported.height, 1280);
    exported.dispose();

    const channel = MethodChannel('com.memolanes/screenshots');
    Uint8List? savedPng;
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'saveToPhotos');
      savedPng = call.arguments['png'] as Uint8List;
      throw PlatformException(code: 'permission_denied');
    });
    addTearDown(() => messenger.setMockMethodCallHandler(channel, null));
    await tester.runAsync(() async {
      tester.widget<AppButton>(actions.first).onPressed!();
      // Rasterization/PNG encoding use the engine's real event loop; their
      // continuations still need the widget test's fake clock to be pumped.
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (savedPng == null && DateTime.now().isBefore(deadline)) {
        await tester.pump();
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      expect(savedPng, isNotNull);
      final image = await decodeScreenshot(savedPng!);
      expect(image.width, 640);
      expect(image.height, 1280);
      // Rounded preview corners must not trim the exported image.
      final data = (await image.toByteData(
        format: ui.ImageByteFormat.rawRgba,
      ))!;
      expect(data.buffer.asUint8List(data.offsetInBytes, 4), [
        33,
        150,
        243,
        255,
      ]);
      image.dispose();
    });
    await tester.pumpAndSettle();
    expect(find.text('没有保存相册的权限，请在系统设置中允许后重试'), findsOneWidget);
    for (final action in tester.widgetList<AppButton>(actions)) {
      expect(action.onPressed, isNotNull);
    }
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    map.dispose();
  });

  testWidgets('snapshot controls fit landscape and large text', (tester) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
    picture.dispose();
    addTearDown(map.dispose);
    final journey = JourneyHeader(
      id: 'screenshot-layout',
      revision: '1',
      journeyDate: DateTime.utc(2026, 10, 19),
      createdAt: DateTime.utc(2026, 10, 19),
      journeyType: JourneyType.vector,
      journeyKind: JourneyKind.defaultKind,
      hasRawData: false,
    );
    for (final (size, textScale) in [
      (const Size(844, 390), 1.0),
      (const Size(320, 568), 2.0),
    ]) {
      tester.view.physicalSize = size;
      await pumpApp(
        tester,
        Builder(
          builder: (context) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(textScale)),
            child: ScreenshotPreviewPage(
              key: ValueKey(size),
              map: map,
              logicalSize: const Size(320, 640),
              journey: journey,
            ),
          ),
        ),
      );
      await waitForLogo(tester);
      await tester.pumpAndSettle();
      final save = find.widgetWithText(AppButton, '保存到相册');
      await tester.ensureVisible(save);
      expect(tester.getRect(save).overlaps(Offset.zero & size), isTrue);
      expect(find.byType(ReadOnlyJourneyInfoCard), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });

  for (final includeLogo in [true, false]) {
    testWidgets(
      'journey preview, gallery and share keep the map fixed with logo $includeLogo',
      (tester) async {
        // An off-center landmark must stay fixed when the card is toggled.
        Future<ui.Image> routeImage() async {
          final recorder = ui.PictureRecorder();
          final canvas = Canvas(recorder);
          canvas.drawColor(Colors.white, BlendMode.src);
          canvas.drawRect(
            Rect.fromCenter(
              center: const Offset(320, 400),
              width: 100,
              height: 100,
            ),
            Paint()..color = const Color(0xff0000ff),
          );
          final picture = recorder.endRecording();
          try {
            return await picture.toImage(640, 1280);
          } finally {
            picture.dispose();
          }
        }

        final map = (await tester.runAsync(routeImage))!;
        addTearDown(map.dispose);
        final journey = JourneyHeader(
          id: 'screenshot-export',
          revision: '1',
          journeyDate: DateTime.utc(2026, 10, 19),
          createdAt: DateTime.utc(2026, 10, 19),
          journeyType: JourneyType.vector,
          journeyKind: JourneyKind.defaultKind,
          hasRawData: false,
        );
        await pumpApp(
          tester,
          ScreenshotPreviewPage(
            map: map,
            logicalSize: const Size(320, 640),
            journey: journey,
          ),
        );
        await waitForLogo(tester);
        if (!includeLogo) {
          await tester.tap(find.widgetWithText(FilterChip, 'Logo 水印'));
          await tester.pumpAndSettle();
        }

        const galleryChannel = MethodChannel('com.memolanes/screenshots');
        const shareChannel = MethodChannel('dev.fluttercommunity.plus/share');
        const pathChannel = MethodChannel('plugins.flutter.io/path_provider');
        final messenger =
            TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
        // Exercise share_plus's actual PNG file creation up to the native bridge.
        final temp = Directory.systemTemp.createTempSync(
          'memolanes_screenshot_',
        );
        Uint8List? savedPng;
        Uint8List? sharedPng;
        messenger.setMockMethodCallHandler(pathChannel, (call) async {
          expect(call.method, 'getTemporaryDirectory');
          return temp.path;
        });
        messenger.setMockMethodCallHandler(shareChannel, (call) async {
          expect(call.method, 'share');
          expect(call.arguments['mimeTypes'], ['image/png']);
          final paths = (call.arguments['paths'] as List).cast<String>();
          expect(paths, hasLength(1));
          sharedPng = File(paths.single).readAsBytesSync();
          expect(call.arguments['originWidth'], greaterThan(0));
          expect(call.arguments['originHeight'], greaterThan(0));
          return ''; // Dismiss the system sheet and retain the preview.
        });
        messenger.setMockMethodCallHandler(galleryChannel, (call) async {
          expect(call.method, 'saveToPhotos');
          savedPng = call.arguments['png'] as Uint8List;
          // Leave the preview open so the same selection can also be shared.
          throw PlatformException(code: 'permission_denied');
        });
        addTearDown(() {
          for (final channel in [galleryChannel, shareChannel, pathChannel]) {
            messenger.setMockMethodCallHandler(channel, null);
          }
          temp.deleteSync(recursive: true);
        });

        Future<Uint8List> pixels(ui.Image image) async {
          expect(
            Size(image.width.toDouble(), image.height.toDouble()),
            const Size(640, 1280),
          );
          final bytes = (await image.toByteData(
            format: ui.ImageByteFormat.rawRgba,
          ))!;
          return bytes.buffer.asUint8List(
            bytes.offsetInBytes,
            bytes.lengthInBytes,
          );
        }

        Future<void> export(String label, bool Function() received) async {
          await tester.runAsync(() async {
            tester
                .widget<AppButton>(find.widgetWithText(AppButton, label))
                .onPressed!();
            final deadline = DateTime.now().add(const Duration(seconds: 10));
            while (!received() && DateTime.now().isBefore(deadline)) {
              await tester.pump();
              await Future<void>.delayed(const Duration(milliseconds: 10));
            }
            expect(received(), isTrue, reason: '$label must receive a PNG');
          });
          await tester.pumpAndSettle();
        }

        final cardOption = find.widgetWithText(FilterChip, '旅程信息');
        for (final includeCard in [false, true, false]) {
          // Deliberately export before pumping the option's new frame.
          tester.widget<FilterChip>(cardOption).onSelected!(includeCard);
          savedPng = null;
          sharedPng = null;
          // Cover either action being the first tap after changing the card.
          final actions = includeLogo ? ['保存到相册', '分享'] : ['分享', '保存到相册'];
          for (final action in actions) {
            await export(
              action,
              () => action == '分享' ? sharedPng != null : savedPng != null,
            );
          }
          expect(
            find.byType(ReadOnlyJourneyInfoCard),
            includeCard ? findsOneWidget : findsNothing,
          );

          final boundary = tester.renderObject<RenderRepaintBoundary>(
            find
                .ancestor(
                  of: find.byType(RawImage).first,
                  matching: find.byType(RepaintBoundary),
                )
                .first,
          );
          late Uint8List previewPixels;
          await tester.runAsync(() async {
            final preview = await boundary.toImage(pixelRatio: 2);
            try {
              previewPixels = await pixels(preview);
              final center = (640 * 640 + 320) * 4;
              final originalCenter = (400 * 640 + 320) * 4;
              const blue = [0, 0, 255, 255];
              const white = [255, 255, 255, 255];
              expect(previewPixels.sublist(center, center + 4), white);
              expect(
                previewPixels.sublist(originalCenter, originalCenter + 4),
                blue,
              );
            } finally {
              preview.dispose();
            }
          });

          await tester.runAsync(() async {
            for (final png in [savedPng!, sharedPng!]) {
              final image = await decodeScreenshot(png);
              try {
                expect(await pixels(image), orderedEquals(previewPixels));
              } finally {
                image.dispose();
              }
            }
          });
        }
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  testWidgets('one tap captures immediately even after idling', (tester) async {
    await pumpApp(tester, app());
    final button = find.byType(MapScreenshotButton);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(captures, 1);
    await tester.pump(const Duration(seconds: 3));
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(captures, 2);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('capture shows progress and ignores repeats until complete', (
    tester,
  ) async {
    final pending = Completer<void>();
    addTearDown(() {
      if (!pending.isCompleted) pending.complete();
    });
    await pumpApp(
      tester,
      app(
        onCapture: () {
          captures++;
          return pending.future;
        },
      ),
    );
    final button = find.byType(MapScreenshotButton);
    // Both taps arrive before the button gets its next build.
    await tester.tap(button);
    await tester.tap(button);
    expect(captures, 1);
    await tester.pump();
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.byTooltip('正在生成截图…'), findsOneWidget);
    await tester.tap(button);
    await tester.pump(const Duration(milliseconds: 300));
    expect(captures, 1);
    pending.complete();
    await tester.pumpAndSettle();
    expect(find.byType(CircularProgressIndicator), findsNothing);
    expect(find.byTooltip('截图'), findsOneWidget);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(captures, 2);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('cancelled contact and drag do not capture', (tester) async {
    await pumpApp(tester, app());
    final button = find.byType(MapScreenshotButton);
    final cancelled = await tester.startGesture(tester.getCenter(button));
    await cancelled.cancel();
    final dragged = await tester.startGesture(tester.getCenter(button));
    await dragged.moveBy(const Offset(-80, 0));
    await dragged.up();
    await tester.pumpAndSettle();
    expect(captures, 0);
    await tester.tap(button);
    await tester.pumpAndSettle();
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('keyboard activation captures on the first press', (
    tester,
  ) async {
    await pumpApp(tester, app());
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('disabled button ignores taps and works once enabled', (
    tester,
  ) async {
    await pumpApp(tester, app(enabled: false));
    await tester.tap(find.byType(MapScreenshotButton));
    await tester.pumpAndSettle();
    expect(captures, 0);
    await pumpApp(tester, app());
    await tester.tap(find.byType(MapScreenshotButton));
    await tester.pumpAndSettle();
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('capture can finish after the button is removed', (tester) async {
    final pending = Completer<void>();
    addTearDown(() {
      if (!pending.isCompleted) pending.complete();
    });
    await pumpApp(tester, app(onCapture: () => pending.future));
    await tester.tap(find.byType(MapScreenshotButton));
    await tester.pump();
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
    pending.complete();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });
}
