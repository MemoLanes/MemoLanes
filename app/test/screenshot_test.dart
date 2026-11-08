import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/screenshot/map_screenshot_flow.dart';
import 'package:memolanes/body/map/screenshot/screenshot_composition.dart';
import 'package:memolanes/body/map/screenshot/screenshot_preview_page.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('zh', 'CN');
  const loader = AppTranslationLoader();
  const logicalSize = Size(320, 640);
  const mapSize = Size(640, 1280);
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
          supportedLocales: const [locale],
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

  Future<ui.Image> renderImage(Size size, void Function(Canvas) draw) async {
    final recorder = ui.PictureRecorder();
    draw(Canvas(recorder));
    final picture = recorder.endRecording();
    try {
      final image = await picture.toImage(
        size.width.toInt(),
        size.height.toInt(),
      );
      addTearDown(image.dispose);
      return image;
    } finally {
      picture.dispose();
    }
  }

  Future<ui.Image> testMap(WidgetTester tester) async {
    // Leave transparent pixels beyond the map and a landmark away from center.
    return (await tester.runAsync(
      () => renderImage(mapSize, (canvas) {
        canvas
          ..drawRect(
            Rect.fromLTRB(0, 16, mapSize.width, mapSize.height),
            Paint()..color = Colors.blue,
          )
          ..drawRect(
            const Rect.fromLTWH(90, 190, 20, 20),
            Paint()..color = Colors.red,
          );
      }),
    ))!;
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
    final layer = boundary.debugLayer as OffsetLayer;
    final image = (await tester.runAsync(
      () => layer.toImage(
        Offset.zero & boundary.size,
        pixelRatio: pixelRatio,
      ),
    ))!;
    addTearDown(image.dispose);
    return image;
  }

  Future<Uint8List> pixels(ui.Image image) async {
    final bytes = (await image.toByteData(format: ui.ImageByteFormat.rawRgba))!;
    return bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes);
  }

  testWidgets('preview, Photos and share export the same composition', (
    tester,
  ) async {
    final map = await testMap(tester);
    await pumpApp(tester, const Scaffold(body: Text('Map')));
    final finishSave = Completer<void>();
    Uint8List? savedPng;
    var denySave = true;
    final route = MaterialPageRoute<bool>(
      builder: (_) => ScreenshotPreviewPage(
        map: map,
        logicalSize: logicalSize,
        timeRange: DateTimeRange(
          start: DateTime(2025, 1, 2),
          end: DateTime(2025, 3, 4),
        ),
        saveToGallery: (png) async {
          savedPng = png;
          if (denySave) throw PlatformException(code: 'permission_denied');
          await finishSave.future;
        },
      ),
    );
    final result = tester
        .state<NavigatorState>(find.byType(Navigator))
        .push(route);
    await tester.pumpAndSettle();
    await waitForLogo(tester);

    const share = MethodChannel('dev.fluttercommunity.plus/share');
    const path = MethodChannel('plugins.flutter.io/path_provider');
    final temp = Directory.systemTemp.createTempSync('memolanes_screenshot_');
    Uint8List? sharedPng;
    messenger.setMockMethodCallHandler(path, (_) async => temp.path);
    messenger.setMockMethodCallHandler(share, (call) async {
      expect(call.method, 'share');
      expect(call.arguments['mimeTypes'], ['image/png']);
      expect(call.arguments['originWidth'], greaterThan(0));
      final files = (call.arguments['paths'] as List).cast<String>();
      sharedPng = File(files.single).readAsBytesSync();
      return '';
    });
    addTearDown(() {
      if (!finishSave.isCompleted) finishSave.complete();
      temp.deleteSync(recursive: true);
    });

    Future<void> export(String label, bool Function() received) async {
      await tester.runAsync(() async {
        await tester.tap(find.widgetWithText(AppButton, label));
        await waitUntil(tester, received);
      });
    }

    final info = find.widgetWithText(FilterChip, '日期信息');
    final caption = find.text('2025.01.02 — 2025.03.04');
    await tester.tap(info);
    await tester.pump();
    expect(caption, findsNothing);

    await tester.tap(info);
    await export('分享', () => sharedPng != null);
    await tester.pumpAndSettle();
    expect(caption, findsOneWidget);
    final preview = await capturePreview(tester, 2);
    await tester.runAsync(() async {
      final exported = await decodeScreenshot(sharedPng!);
      addTearDown(exported.dispose);
      expect(
        Size(exported.width.toDouble(), exported.height.toDouble()),
        mapSize,
      );
      final rgba = await pixels(exported);
      expect(rgba.sublist(0, 4), [0, 0, 0, 255]);
      const landmark = (200 * 640 + 100) * 4;
      expect(rgba.sublist(landmark, landmark + 4), [244, 67, 54, 255]);
      expect(listEquals(rgba, await pixels(preview)), isTrue);
    });

    await export('保存到相册', () => savedPng != null);
    await tester.pumpAndSettle();
    expect(find.text('未获得保存图片的权限，请在系统设置中允许后重试'), findsOneWidget);

    denySave = false;
    savedPng = null;
    await export('保存到相册', () => savedPng != null);
    expect(savedPng, sharedPng);
    await tester.binding.handlePopRoute();
    await tester.pump();
    expect(find.byType(ScreenshotPreviewPage), findsOneWidget);
    finishSave.complete();
    await tester.pumpAndSettle();
    expect(await result, isTrue);
  });

  testWidgets('large text keeps the caption, watermark and actions usable', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    final map = await testMap(tester);
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
    await pumpApp(
      tester,
      ScreenshotPreviewPage(
        map: map,
        logicalSize: logicalSize,
        journey: journey,
      ),
    );
    await waitForLogo(tester);
    final caption = find.text(journey.note!);
    final logo = find.descendant(
      of: find.byType(ScreenshotComposition),
      matching: find.byType(CustomPaint),
    );
    final save = find.widgetWithText(AppButton, '保存到相册');
    for (final (size, label) in [
      (const Size(844, 390), '右上角'),
      (const Size(320, 568), '右下角'),
    ]) {
      tester.view.physicalSize = size;
      await tester.pumpAndSettle();
      final bounds = tester.getRect(caption);
      expect(bounds.overlaps(tester.getRect(logo)), isFalse);
      final position = find.text(label);
      await tester.ensureVisible(position);
      await tester.tap(position);
      await tester.pumpAndSettle();
      expect(tester.getRect(caption), bounds);
      expect(bounds.overlaps(tester.getRect(logo)), isFalse);

      await tester.ensureVisible(save);
      expect(tester.getRect(save).overlaps(Offset.zero & size), isTrue);
    }
  });
}
