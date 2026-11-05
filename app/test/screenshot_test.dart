import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/screenshot/screenshot_caption.dart';
import 'package:memolanes/body/map/screenshot/screenshot_composition.dart';
import 'package:memolanes/body/map/screenshot/screenshot_preview_page.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/screenshot.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('zh', 'CN');
  const english = Locale('en', 'US');
  const loader = AppTranslationLoader();
  const logicalSize = Size(320, 640);
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  setUpAll(() async {
    messenger.setMockMethodCallHandler(
      const MethodChannel('plugins.flutter.io/shared_preferences'),
      (call) async => call.method == 'getAll' ? <String, Object>{} : null,
    );
    await EasyLocalization.ensureInitialized();
    await loader.load('assets/translations', locale);
  });

  Future<void> pumpApp(WidgetTester tester, Widget home) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale, english],
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

  // SVG loading and image encoding need the real event loop and Flutter frames.
  Future<void> waitUntil(WidgetTester tester, bool Function() ready) async {
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (!ready() && DateTime.now().isBefore(deadline)) {
      await tester.pump();
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
    expect(ready(), isTrue);
  }

  Future<void> waitForLogo(WidgetTester tester) async {
    final options = find.byType(SegmentedButton<ScreenshotLogoPosition>);
    await tester.runAsync(
      () => waitUntil(
        tester,
        () =>
            tester
                .widget<SegmentedButton<ScreenshotLogoPosition>>(options)
                .onSelectionChanged !=
            null,
      ),
    );
  }

  Future<ui.Image> testMap(WidgetTester tester, int width, int height) async {
    final recorder = ui.PictureRecorder();
    // Leave transparent pixels beyond the map and a landmark away from center.
    Canvas(recorder)
      ..drawRect(
        Rect.fromLTRB(0, 16, width.toDouble(), height.toDouble()),
        Paint()..color = Colors.blue,
      )
      ..drawRect(
        const Rect.fromLTWH(90, 190, 20, 20),
        Paint()..color = Colors.red,
      );
    final picture = recorder.endRecording();
    try {
      final map = (await tester.runAsync(
        () => picture.toImage(width, height),
      ))!;
      addTearDown(map.dispose);
      return map;
    } finally {
      picture.dispose();
    }
  }

  Future<ui.Image> capturePreview(
    WidgetTester tester,
    double pixelRatio,
  ) async {
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find
          .ancestor(
            of: find.byType(ScreenshotComposition),
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

  testWidgets('preview, Photos and share export the same composition', (
    tester,
  ) async {
    final map = await testMap(tester, 640, 1280);
    await pumpApp(tester, const Scaffold(body: Text('Map')));
    final route = MaterialPageRoute<bool>(
      builder: (_) => ScreenshotPreviewPage(
        map: map,
        logicalSize: logicalSize,
        timeRange: DateTimeRange(
          start: DateTime(2025, 1, 2),
          end: DateTime(2025, 3, 4),
        ),
      ),
    );
    final result = tester
        .state<NavigatorState>(find.byType(Navigator))
        .push(route);
    await tester.pumpAndSettle();
    await waitForLogo(tester);

    const gallery = MethodChannel('com.memolanes/screenshots');
    const share = MethodChannel('dev.fluttercommunity.plus/share');
    const path = MethodChannel('plugins.flutter.io/path_provider');
    final temp = Directory.systemTemp.createTempSync('memolanes_screenshot_');
    final finishSave = Completer<void>();
    Uint8List? savedPng;
    Uint8List? sharedPng;
    var denySave = true;
    messenger.setMockMethodCallHandler(path, (_) async => temp.path);
    messenger.setMockMethodCallHandler(share, (call) async {
      expect(call.method, 'share');
      expect(call.arguments['mimeTypes'], ['image/png']);
      expect(call.arguments['originWidth'], greaterThan(0));
      final files = (call.arguments['paths'] as List).cast<String>();
      sharedPng = File(files.single).readAsBytesSync();
      return '';
    });
    messenger.setMockMethodCallHandler(gallery, (call) async {
      expect(call.method, 'saveToPhotos');
      savedPng = call.arguments['png'] as Uint8List;
      if (denySave) throw PlatformException(code: 'permission_denied');
      await finishSave.future;
      return null;
    });
    addTearDown(() {
      if (!finishSave.isCompleted) finishSave.complete();
      for (final channel in [gallery, share, path]) {
        messenger.setMockMethodCallHandler(channel, null);
      }
      temp.deleteSync(recursive: true);
    });

    Future<void> export(String label, bool Function() received) async {
      await tester.runAsync(() async {
        await tester.tap(find.widgetWithText(AppButton, label));
        await waitUntil(tester, received);
      });
    }

    Future<void> expectComposition(Uint8List png) async {
      final preview = await capturePreview(tester, 2);
      await tester.runAsync(() async {
        try {
          final image = await decodeScreenshot(png);
          try {
            expect(image.width, 640);
            expect(image.height, 1280);
            final rgba = await pixels(image);
            expect(rgba.sublist(0, 4), [0, 0, 0, 255]);
            const landmark = (200 * 640 + 100) * 4;
            expect(rgba.sublist(landmark, landmark + 4), [244, 67, 54, 255]);
            expect(rgba, orderedEquals(await pixels(preview)));
          } finally {
            image.dispose();
          }
        } finally {
          preview.dispose();
        }
      });
    }

    final info = find.widgetWithText(FilterChip, '时间信息');
    await tester.tap(info);
    await export('分享', () => sharedPng != null);
    await tester.pumpAndSettle();
    expect(find.byType(ScreenshotCaption), findsNothing);
    await expectComposition(sharedPng!);

    await tester.tap(info);
    await export('保存到相册', () => savedPng != null);
    await tester.pumpAndSettle();
    expect(find.text('2025.01.02 — 2025.03.04'), findsOneWidget);
    expect(find.text('没有保存相册的权限，请在系统设置中允许后重试'), findsOneWidget);
    await expectComposition(savedPng!);

    denySave = false;
    savedPng = null;
    await export('保存到相册', () => savedPng != null);
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.byType(ScreenshotPreviewPage), findsOneWidget);
    finishSave.complete();
    await tester.pumpAndSettle();
    expect(await result, isTrue);
    await route.completed;
  });

  testWidgets('localized watermarks render as vectors at export resolution', (
    tester,
  ) async {
    const size = Size(160, 80);
    const pixelRatio = 16.0;
    final map = await testMap(tester, 2560, 1280);
    await pumpApp(tester, ScreenshotPreviewPage(map: map, logicalSize: size));

    for (final language in [locale, english]) {
      await tester.runAsync(
        () => tester
            .element(find.byType(ScreenshotPreviewPage))
            .setLocale(language),
      );
      await tester.pump();
      await waitForLogo(tester);
      final actual = await capturePreview(tester, pixelRatio);
      await tester.runAsync(() async {
        final svg = await vg.loadPicture(
          SvgAssetLoader('assets/screenshot_logo_${language.languageCode}.svg'),
          null,
        );
        final recorder = ui.PictureRecorder();
        const logoWidth = 96.0;
        final logoHeight = logoWidth / svg.size.aspectRatio;
        Canvas(recorder)
          ..drawColor(Colors.black, BlendMode.src)
          ..drawImage(map, Offset.zero, Paint())
          ..scale(pixelRatio)
          ..translate(
            size.width - 20 - logoWidth,
            size.height - 20 - logoHeight,
          )
          ..scale(logoWidth / svg.size.width)
          ..drawPicture(svg.picture);
        final picture = recorder.endRecording();
        final reference = await picture.toImage(2560, 1280);
        picture.dispose();
        svg.picture.dispose();
        try {
          expect(actual.width, reference.width);
          expect(actual.height, reference.height);
          final actualPixels = await pixels(actual);
          final referencePixels = await pixels(reference);
          var maxDifference = 0;
          for (var i = 0; i < actualPixels.length; i++) {
            final difference = (actualPixels[i] - referencePixels[i]).abs();
            if (difference > maxDifference) maxDifference = difference;
          }
          expect(maxDifference, lessThanOrEqualTo(1));
        } finally {
          reference.dispose();
          actual.dispose();
        }
      });
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('large text keeps the caption, watermark and actions usable', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final map = await testMap(tester, 640, 1280);
    final journey = JourneyHeader(
      id: 'screenshot-layout',
      revision: '1',
      journeyDate: DateTime.utc(2025, 1, 2),
      createdAt: DateTime.utc(2025, 1, 2),
      start: DateTime(2025, 1, 2, 8, 30),
      end: DateTime(2025, 1, 2, 18, 45),
      note: '沿着湖边散步，记录旅途中的风景。',
      journeyType: JourneyType.vector,
      journeyKind: JourneyKind.defaultKind,
      hasRawData: false,
    );
    for (final size in [const Size(844, 390), const Size(320, 568)]) {
      tester.view.physicalSize = size;
      await pumpApp(
        tester,
        Builder(
          builder: (context) => MediaQuery(
            data: MediaQuery.of(context)
                .copyWith(textScaler: TextScaler.linear(2)),
            child: ScreenshotPreviewPage(
              map: map,
              logicalSize: logicalSize,
              journey: journey,
            ),
          ),
        ),
      );
      await waitForLogo(tester);
      final caption = find.byType(ScreenshotCaption);
      final logo = find.descendant(
        of: find.byType(ScreenshotComposition),
        matching: find.byType(CustomPaint),
      );
      final bounds = tester.getRect(caption);
      expect(bounds.overlaps(tester.getRect(logo)), isFalse);
      final topRight = find.text('右上角');
      await tester.ensureVisible(topRight);
      await tester.tap(topRight);
      await tester.pumpAndSettle();
      expect(tester.getRect(caption), bounds);
      expect(bounds.overlaps(tester.getRect(logo)), isFalse);

      final save = find.widgetWithText(AppButton, '保存到相册');
      await tester.ensureVisible(save);
      expect(tester.getRect(save).overlaps(Offset.zero & size), isTrue);
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });
}
