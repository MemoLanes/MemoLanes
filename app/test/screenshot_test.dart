import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/map_controls/screenshot_button.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/screenshot.dart';
import 'package:memolanes/body/map/screenshot/screenshot_preview_page.dart';
import 'package:memolanes/body/map/screenshot/screenshot_caption.dart';
import 'package:memolanes/body/map/screenshot/screenshot_composition.dart';
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
  Widget app({required Future<void> Function() onCapture}) => Scaffold(
    body: Center(child: MapScreenshotButton(onPressed: onCapture)),
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

  Future<void> waitForLogo(WidgetTester tester) async {
    final logoOption = find.byType(SegmentedButton<ScreenshotLogoPosition>);
    await tester.runAsync(() async {
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (tester
                  .widget<SegmentedButton<ScreenshotLogoPosition>>(logoOption)
                  .onSelectionChanged ==
              null &&
          DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 10));
        await tester.pump();
      }
    });
    expect(
      tester
          .widget<SegmentedButton<ScreenshotLogoPosition>>(logoOption)
          .onSelectionChanged,
      isNotNull,
    );
  }

  Future<ui.Image> capturePreview(
    WidgetTester tester, {
    double pixelRatio = 2,
  }) async {
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find
          .ancestor(
            of: find.byType(RawImage).first,
            matching: find.byType(RepaintBoundary),
          )
          .first,
    );
    return (await tester.runAsync(
      () => boundary.toImage(pixelRatio: pixelRatio),
    ))!;
  }

  Future<Uint8List> pixels(ui.Image image) async {
    final bytes = (await image.toByteData(format: ui.ImageByteFormat.rawRgba))!;
    return bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes);
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

    Future<Uint8List> previewPixels() async {
      final image = await capturePreview(tester);
      try {
        return (await tester.runAsync(() => pixels(image)))!;
      } finally {
        image.dispose();
      }
    }

    final chinesePixels = await previewPixels();
    tester
        .widget<SegmentedButton<ScreenshotLogoPosition>>(
          find.byType(SegmentedButton<ScreenshotLogoPosition>),
        )
        .onSelectionChanged!({ScreenshotLogoPosition.hidden});
    await tester.pumpAndSettle();
    await tester.runAsync(
      () => tester.element(page).setLocale(const Locale('en', 'US')),
    );
    await tester.pumpAndSettle();
    await waitForLogo(tester);
    final englishOption = find.byType(SegmentedButton<ScreenshotLogoPosition>);
    expect(tester.state(page), same(state));
    expect(
      tester
          .widget<SegmentedButton<ScreenshotLogoPosition>>(englishOption)
          .selected,
      {ScreenshotLogoPosition.hidden},
    );
    final mapPixels = (await tester.runAsync(() => pixels(map)))!;
    expect(await previewPixels(), orderedEquals(mapPixels));
    tester
        .widget<SegmentedButton<ScreenshotLogoPosition>>(englishOption)
        .onSelectionChanged!({ScreenshotLogoPosition.bottomRight});
    await tester.pumpAndSettle();
    final englishPixels = await previewPixels();
    expect(englishPixels, isNot(orderedEquals(mapPixels)));
    expect(englishPixels, isNot(orderedEquals(chinesePixels)));

    await tester.runAsync(() => tester.element(page).setLocale(locale));
    await tester.pumpAndSettle();
    await waitForLogo(tester);
    expect(
      tester
          .widget<SegmentedButton<ScreenshotLogoPosition>>(
            find.byType(SegmentedButton<ScreenshotLogoPosition>),
          )
          .selected,
      {ScreenshotLogoPosition.bottomRight},
    );
    expect(await previewPixels(), orderedEquals(chinesePixels));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('watermarks render as vectors at high export resolution', (
    tester,
  ) async {
    // A 1536px-wide watermark exceeds the old 960px raster asset's resolution.
    const logicalSize = Size(160, 80);
    const pixelRatio = 16.0;
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final map = (await tester.runAsync(() => picture.toImage(2560, 1280)))!;
    picture.dispose();
    addTearDown(map.dispose);
    await pumpApp(
      tester,
      ScreenshotPreviewPage(map: map, logicalSize: logicalSize),
    );

    for (final language in ['zh', 'en']) {
      if (language == 'en') {
        await tester.runAsync(
          () => tester
              .element(find.byType(ScreenshotPreviewPage))
              .setLocale(const Locale('en', 'US')),
        );
        await tester.pumpAndSettle();
      }
      await waitForLogo(tester);
      final actual = await capturePreview(tester, pixelRatio: pixelRatio);
      await tester.runAsync(() async {
        final svg = await vg.loadPicture(
          SvgAssetLoader('assets/screenshot_logo_$language.svg'),
          null,
        );
        final referenceRecorder = ui.PictureRecorder();
        final canvas = Canvas(referenceRecorder)
          ..drawColor(Colors.blue, BlendMode.src);
        const logoWidth = 96.0;
        final logoLeft = logicalSize.width - 20 - logoWidth;
        final logoHeight = logoWidth / svg.size.aspectRatio;
        canvas
          ..scale(pixelRatio)
          ..translate(logoLeft, logicalSize.height - 20 - logoHeight)
          ..scale(logoWidth / svg.size.width)
          ..drawPicture(svg.picture);
        final referencePicture = referenceRecorder.endRecording();
        final reference = await referencePicture.toImage(2560, 1280);
        svg.picture.dispose();
        referencePicture.dispose();
        try {
          final actualPixels = await pixels(actual);
          final referencePixels = await pixels(reference);
          expect(actual.width, 2560);
          expect(actual.height, 1280);
          // Allow one rounding step when composing Flutter's canvas transforms.
          var maxDifference = 0;
          for (var i = 0; i < actualPixels.length; i++) {
            final difference = (actualPixels[i] - referencePixels[i]).abs();
            if (difference > maxDifference) maxDifference = difference;
          }
          expect(maxDifference, lessThanOrEqualTo(1));
          // The original black backdrop stays transparent over the map.
          final transparentPoint =
              (((logicalSize.height - 20 - logoHeight) * pixelRatio + 80)
                          .floor() *
                      2560 +
                  ((logoLeft + 8) * pixelRatio).floor()) *
              4;
          expect(actualPixels.sublist(transparentPoint, transparentPoint + 4), [
            33,
            150,
            243,
            255,
          ]);
        } finally {
          reference.dispose();
          actual.dispose();
        }
      });
    }
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('export retains resolution and recovers from denied permission', (
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
    expect(
      tester
          .widget<SegmentedButton<ScreenshotLogoPosition>>(
            find.byType(SegmentedButton<ScreenshotLogoPosition>),
          )
          .selected,
      {ScreenshotLogoPosition.bottomRight},
    );
    expect(find.widgetWithText(FilterChip, '旅程信息'), findsNothing);

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
      await tester.tap(find.widgetWithText(AppButton, '保存到相册'));
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
    for (final action in tester.widgetList<AppButton>(find.byType(AppButton))) {
      expect(action.onPressed, isNotNull);
    }
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
    map.dispose();
  });

  testWidgets('save blocks back until Photos completes and returns success', (
    tester,
  ) async {
    final recorder = ui.PictureRecorder();
    Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
    final picture = recorder.endRecording();
    final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
    picture.dispose();
    addTearDown(map.dispose);
    await pumpApp(tester, const Scaffold(body: Text('Map')));
    final route = MaterialPageRoute<bool>(
      builder: (_) =>
          ScreenshotPreviewPage(map: map, logicalSize: const Size(320, 640)),
    );
    final result = tester
        .state<NavigatorState>(find.byType(Navigator))
        .push(route);
    await tester.pumpAndSettle();
    await waitForLogo(tester);

    final saving = Completer<void>();
    var received = false;
    const channel = MethodChannel('com.memolanes/screenshots');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (call) async {
      expect(call.method, 'saveToPhotos');
      received = true;
      await saving.future;
      return null;
    });
    addTearDown(() {
      if (!saving.isCompleted) saving.complete();
      messenger.setMockMethodCallHandler(channel, null);
    });
    await tester.runAsync(() async {
      await tester.tap(find.widgetWithText(AppButton, '保存到相册'));
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (!received && DateTime.now().isBefore(deadline)) {
        await tester.pump();
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      expect(received, isTrue);
    });
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.byType(ScreenshotPreviewPage), findsOneWidget);
    expect(
      tester.widget<AppButton>(find.widgetWithText(AppButton, '分享')).onPressed,
      isNull,
    );

    saving.complete();
    await tester.pumpAndSettle();
    expect(await result, isTrue);
    await route.completed;
    expect(find.byType(ScreenshotPreviewPage), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
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
      expect(find.byType(ScreenshotCaption), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });

  for (final language in ['zh', 'en']) {
    testWidgets(
      'snapshot text and watermark stay separate on phones ($language)',
      (tester) async {
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPhysicalSize);
        final recorder = ui.PictureRecorder();
        Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
        final picture = recorder.endRecording();
        final map = (await tester.runAsync(() => picture.toImage(2, 2)))!;
        picture.dispose();
        addTearDown(map.dispose);
        final logo = (await tester.runAsync(
          () => vg.loadPicture(
            SvgAssetLoader('assets/screenshot_logo_$language.svg'),
            null,
          ),
        ))!;
        addTearDown(logo.picture.dispose);
        final journey = JourneyHeader(
          id: 'screenshot-text-layout',
          revision: '1',
          journeyDate: DateTime.utc(2026, 11, 5),
          createdAt: DateTime.utc(2026, 11, 5),
          start: DateTime(2026, 11, 5, 8, 30),
          end: DateTime(2026, 11, 5, 18, 45),
          note: '沿着湖边散步，记录旅途中的风景。 A long journey note beside the lake.',
          journeyType: JourneyType.vector,
          journeyKind: JourneyKind.defaultKind,
          hasRawData: false,
        );
        for (final (size, textScale) in [
          (const Size(320, 568), 1.0),
          (const Size(360, 640), 1.0),
          (const Size(393, 852), 1.0),
          (const Size(430, 932), 1.5),
          (const Size(844, 390), 1.0),
        ]) {
          tester.view.physicalSize = size;
          for (final timeline in [false, true]) {
            Rect? captionBounds;
            for (final position in ScreenshotLogoPosition.values) {
              await pumpApp(
                tester,
                Builder(
                  builder: (context) => MediaQuery(
                    data: MediaQuery.of(context)
                        .copyWith(textScaler: TextScaler.linear(textScale)),
                    child: ScreenshotComposition(
                      map: map,
                      logicalSize: size,
                      includeInfo: true,
                      journey: timeline ? null : journey,
                      timeRange: timeline
                          ? DateTimeRange(
                              start: DateTime(2024),
                              end: DateTime(2026, 11, 5),
                            )
                          : null,
                      logo: logo,
                      logoPosition: position,
                    ),
                  ),
                ),
              );
              await tester.runAsync(
                () => tester
                    .element(find.byType(ScreenshotComposition))
                    .setLocale(
                      language == 'zh' ? locale : const Locale('en', 'US'),
                    ),
              );
              await tester.pumpAndSettle();
              final caption = tester.getRect(find.byType(ScreenshotCaption));
              captionBounds ??= caption;
              expect(caption, captionBounds);
              final frame = Offset.zero & size;
              expect(frame.contains(caption.topLeft), isTrue);
              expect(frame.contains(caption.bottomRight), isTrue);
              if (position != ScreenshotLogoPosition.hidden) {
                final watermark = tester.getRect(
                  find.descendant(
                    of: find.byType(ScreenshotComposition),
                    matching: find.byType(CustomPaint),
                  ),
                );
                expect(caption.overlaps(watermark), isFalse);
                expect(watermark.width, lessThanOrEqualTo(96));
                if (position == ScreenshotLogoPosition.bottomRight) {
                  expect(
                    watermark.top - caption.bottom,
                    greaterThanOrEqualTo(12),
                  );
                }
              }
              expect(tester.takeException(), isNull);
            }
          }
        }
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }

  for (final scenario in [
    for (final position in ScreenshotLogoPosition.values)
      for (final scene in ['record', 'journey', 'timeMachine'])
        (position, scene),
  ]) {
    final (position, scene) = scenario;
    final timeline = scene == 'timeMachine';
    final hasInfo = scene != 'record';
    final includeLogo = position != ScreenshotLogoPosition.hidden;
    testWidgets('preview, gallery and share match for $position, scene $scene', (
      tester,
    ) async {
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
        start: DateTime(2026, 10, 19, 8, 30),
        end: DateTime(2026, 10, 19, 10, 15),
        note: '沿着湖边散步，记录这一天的风景。',
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
          journey: scene == 'journey' ? journey : null,
          timeRange: timeline
              ? DateTimeRange(
                  start: DateTime(2025, 1, 2),
                  end: DateTime(2025, 3, 4),
                )
              : null,
        ),
      );
      await waitForLogo(tester);
      final logoOptions = tester
          .widget<SegmentedButton<ScreenshotLogoPosition>>(
            find.byType(SegmentedButton<ScreenshotLogoPosition>),
          );
      expect(logoOptions.selected, {ScreenshotLogoPosition.bottomRight});
      final composition = find.byType(ScreenshotComposition);
      expect(
        tester.widget<ScreenshotComposition>(composition).logoPosition,
        ScreenshotLogoPosition.bottomRight,
      );
      final captionRect = hasInfo
          ? tester.getRect(find.byType(ScreenshotCaption))
          : null;
      // Exercise the default export without touching the watermark selector.
      if (position != ScreenshotLogoPosition.bottomRight) {
        logoOptions.onSelectionChanged!({position});
      }
      await tester.pumpAndSettle();
      if (hasInfo) {
        expect(tester.getRect(find.byType(ScreenshotCaption)), captionRect);
      }
      if (includeLogo) {
        final logo = find.descendant(
          of: composition,
          matching: find.byType(CustomPaint),
        );
        final logoRect = tester.getRect(logo);
        final frame = tester.getRect(composition);
        expect(frame.contains(logoRect.topLeft), isTrue);
        expect(frame.contains(logoRect.bottomRight), isTrue);
        expect(logoRect.center.dx, greaterThan(frame.center.dx));
        if (position == ScreenshotLogoPosition.bottomRight) {
          expect(logoRect.center.dy, greaterThan(frame.center.dy));
        } else {
          expect(logoRect.center.dy, lessThan(frame.center.dy));
        }
      }
      if (timeline) {
        expect(find.text('时光机带你来到了'), findsOneWidget);
        expect(find.text('2025.01.02 — 2025.03.04'), findsOneWidget);
      }

      const galleryChannel = MethodChannel('com.memolanes/screenshots');
      const shareChannel = MethodChannel('dev.fluttercommunity.plus/share');
      const pathChannel = MethodChannel('plugins.flutter.io/path_provider');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      // Exercise share_plus's actual PNG file creation up to the native bridge.
      final temp = Directory.systemTemp.createTempSync('memolanes_screenshot_');
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

      final cardOption = find.widgetWithText(
        FilterChip,
        timeline ? '时间信息' : '旅程信息',
      );
      for (final includeCard
          in hasInfo ? [true, false, true, false] : [false]) {
        // Deliberately export before pumping the option's new frame.
        if (hasInfo) {
          tester.widget<FilterChip>(cardOption).onSelected!(includeCard);
        }
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
          find.byType(ScreenshotCaption),
          includeCard ? findsOneWidget : findsNothing,
        );

        final preview = await capturePreview(tester);
        late Uint8List previewPixels;
        await tester.runAsync(() async {
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
    });
  }

  testWidgets('capture shows progress and ignores repeats until complete', (
    tester,
  ) async {
    var captures = 0;
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
