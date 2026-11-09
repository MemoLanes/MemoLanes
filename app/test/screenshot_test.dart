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
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('zh', 'CN');
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
    await rootBundle.loadString('assets/translations/zh-CN.json');
  });

  Future<void> pumpApp(WidgetTester tester, Widget home) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale],
          path: 'assets/translations',
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

  final composition = find.byType(ScreenshotComposition);
  final logo = find.descendant(
    of: composition,
    matching: find.byType(CustomPaint),
  );
  Finder action(String label) => find.widgetWithText(AppButton, label);

  // Asset loading and PNG encoding need real time as well as Flutter frames.
  Future<void> pumpUntil(WidgetTester tester, bool Function() ready) async {
    final deadline = DateTime.now().add(const Duration(seconds: 10));
    while (!ready()) {
      if (!DateTime.now().isBefore(deadline)) fail('Screenshot did not finish');
      await tester.pump();
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
  }

  Future<void> waitForLogo(WidgetTester tester) async {
    await tester.runAsync(
      () => pumpUntil(tester, () => logo.evaluate().isNotEmpty),
    );
  }

  ui.Image testMap() {
    final recorder = ui.PictureRecorder();
    // Leave transparent pixels beyond the map and a landmark away from center.
    Canvas(recorder)
      ..drawRect(
        Rect.fromLTRB(0, 16, mapSize.width, mapSize.height),
        Paint()..color = Colors.blue,
      )
      ..drawRect(
        const Rect.fromLTWH(90, 190, 20, 20),
        Paint()..color = Colors.red,
      );
    final picture = recorder.endRecording();
    addTearDown(picture.dispose);
    final image = picture.toImageSync(
      mapSize.width.toInt(),
      mapSize.height.toInt(),
    );
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
    final map = testMap();
    await pumpApp(tester, const Scaffold());
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

    Future<void> export(String label, bool Function() ready) async {
      await tester.runAsync(() async {
        await tester.tap(action(label));
        await pumpUntil(tester, ready);
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
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find
          .ancestor(of: composition, matching: find.byType(RepaintBoundary))
          .first,
    );
    await tester.runAsync(() async {
      final preview = await boundary.toImage(pixelRatio: 2);
      addTearDown(preview.dispose);
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
    final map = testMap();
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
    final save = action('保存到相册');
    for (final (size, label) in [
      (const Size(844, 390), '右上角'),
      (const Size(320, 568), '右下角'),
    ]) {
      tester.view.physicalSize = size;
      await tester.pumpAndSettle();
      final bounds = tester.getRect(caption);
      final previousLogoBounds = tester.getRect(logo);
      final position = find.text(label);
      await tester.ensureVisible(position);
      await tester.tap(position);
      await tester.pumpAndSettle();
      final logoBounds = tester.getRect(logo);
      expect(logoBounds, isNot(previousLogoBounds));
      expect(tester.getRect(caption), bounds);
      expect(bounds.overlaps(logoBounds), isFalse);

      await tester.ensureVisible(save);
      expect(tester.getRect(save).overlaps(Offset.zero & size), isTrue);
    }
  });
}
