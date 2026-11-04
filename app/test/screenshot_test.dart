import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/overlay/time_machine_overlay.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/map_controls/screenshot_button.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/screenshot.dart';
import 'package:memolanes/body/map/screenshot/screenshot_preview_page.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const locale = Locale('zh', 'CN');
  const loader = AppTranslationLoader();
  final journey = JourneyHeader(
    id: 'screenshot-test',
    revision: '1',
    journeyDate: DateTime.utc(2026, 10, 19),
    createdAt: DateTime.utc(2026, 10, 19),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
    hasRawData: false,
  );
  setUpAll(() async {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
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

  Future<void> pumpUntil(
    WidgetTester tester,
    bool Function() ready, {
    VoidCallback? start,
  }) async {
    await tester.runAsync(() async {
      start?.call();
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (!ready() && DateTime.now().isBefore(deadline)) {
        await tester.pump();
        await Future<void>.delayed(const Duration(milliseconds: 10));
      }
      expect(ready(), isTrue);
    });
  }

  Future<void> waitForLogo(WidgetTester tester, {String label = 'Logo 水印'}) =>
      pumpUntil(
        tester,
        () =>
            tester
                .widget<FilterChip>(find.widgetWithText(FilterChip, label))
                .onSelected !=
            null,
      );

  Future<ui.Image> createMapImage(
    WidgetTester tester, {
    int width = 640,
    int height = 1280,
    void Function(Canvas)? paint,
  }) async {
    final recorder = ui.PictureRecorder();
    final canvas = Canvas(recorder)..drawColor(Colors.blue, BlendMode.src);
    paint?.call(canvas);
    final picture = recorder.endRecording();
    try {
      final image = (await tester.runAsync(
        () => picture.toImage(width, height),
      ))!;
      addTearDown(image.dispose);
      return image;
    } finally {
      picture.dispose();
    }
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

  Future<Uint8List> previewPixels(WidgetTester tester) async {
    final image = await capturePreview(tester);
    try {
      return (await tester.runAsync(() => pixels(image)))!;
    } finally {
      image.dispose();
    }
  }

  test(
    'system screenshots are discarded while the map has no listener',
    () async {
      screenshotChannel.setMethodCallHandler(null);
      addTearDown(() => screenshotChannel.setMethodCallHandler(null));
      final received = <String>[];
      void sendScreenshot() => ui.channelBuffers.push(
        screenshotChannel.name,
        screenshotChannel.codec.encodeMethodCall(
          const MethodCall('onSystemScreenshot'),
        ),
        (_) {},
      );

      for (var visit = 0; visit < 2; visit++) {
        sendScreenshot();
        screenshotChannel.setMethodCallHandler((call) async {
          received.add(call.method);
        });
        await Future<void>.delayed(Duration.zero);
        expect(received, isEmpty, reason: 'Do not replay screenshots on entry');

        sendScreenshot();
        await Future<void>.delayed(Duration.zero);
        expect(received, ['onSystemScreenshot']);
        screenshotChannel.setMethodCallHandler(null);
        received.clear();
      }
    },
  );

  testWidgets('watermark follows app language and retains visibility choice', (
    tester,
  ) async {
    final map = await createMapImage(tester);
    await pumpApp(
      tester,
      ScreenshotPreviewPage(map: map, logicalSize: const Size(320, 640)),
    );
    await waitForLogo(tester);
    final page = find.byType(ScreenshotPreviewPage);
    final state = tester.state(page);

    final chinesePixels = await previewPixels(tester);
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
    final mapPixels = (await tester.runAsync(() => pixels(map)))!;
    expect(await previewPixels(tester), orderedEquals(mapPixels));
    await tester.tap(englishOption);
    await tester.pumpAndSettle();
    final englishPixels = await previewPixels(tester);
    expect(englishPixels, isNot(orderedEquals(mapPixels)));
    expect(englishPixels, isNot(orderedEquals(chinesePixels)));

    await tester.runAsync(() => tester.element(page).setLocale(locale));
    await tester.pumpAndSettle();
    await waitForLogo(tester);
    expect(tester.widget<FilterChip>(find.byType(FilterChip)).selected, isTrue);
    expect(await previewPixels(tester), orderedEquals(chinesePixels));
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('watermarks render as vectors at high export resolution', (
    tester,
  ) async {
    // A 1920px-wide watermark exceeds the old 960px raster asset's resolution.
    const logicalSize = Size(160, 80);
    const pixelRatio = 16.0;
    final map = await createMapImage(tester, width: 2560);
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
      await waitForLogo(
        tester,
        label: language == 'zh' ? 'Logo 水印' : 'Watermark',
      );
      final actual = await capturePreview(tester, pixelRatio: pixelRatio);
      await tester.runAsync(() async {
        final svg = await vg.loadPicture(
          SvgAssetLoader('assets/screenshot_logo_$language.svg'),
          null,
        );
        final referenceRecorder = ui.PictureRecorder();
        final canvas = Canvas(referenceRecorder)
          ..drawColor(Colors.blue, BlendMode.src);
        final logoHeight = 120 / svg.size.aspectRatio;
        canvas
          ..scale(pixelRatio)
          ..translate(20, logicalSize.height - 20 - logoHeight)
          ..scale(120 / svg.size.width)
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
              (((logicalSize.height - 20 - logoHeight) * pixelRatio + 100)
                          .floor() *
                      2560 +
                  480) *
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

  testWidgets('save blocks back until Photos completes and returns success', (
    tester,
  ) async {
    final map = await createMapImage(tester);
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
    await pumpUntil(
      tester,
      () => received,
      start: () => tester
          .widget<AppButton>(find.widgetWithText(AppButton, '保存到相册'))
          .onPressed!(),
    );
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
    final map = await createMapImage(tester);
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

  testWidgets('preview options keep the map fixed and exports match preview', (
    tester,
  ) async {
    final map = await createMapImage(
      tester,
      paint: (canvas) => canvas
        ..drawColor(Colors.white, BlendMode.src)
        ..drawRect(
          Rect.fromCenter(
            center: const Offset(320, 400),
            width: 100,
            height: 100,
          ),
          Paint()..color = const Color(0xff0000ff),
        ),
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
    for (final includeOverlays in [false, true]) {
      tester.widget<FilterChip>(cardOption).onSelected!(includeOverlays);
      tester.widget<FilterChip>(logoOption).onSelected!(includeOverlays);
      await tester.pump();
      expect(
        find.byType(ReadOnlyJourneyInfoCard),
        includeOverlays ? findsOneWidget : findsNothing,
      );
      final preview = await previewPixels(tester);
      const center = (640 * 640 + 320) * 4;
      const landmark = (400 * 640 + 320) * 4;
      expect(preview.sublist(center, center + 4), [255, 255, 255, 255]);
      expect(preview.sublist(landmark, landmark + 4), [0, 0, 255, 255]);
      if (!includeOverlays) {
        final original = (await tester.runAsync(() => pixels(map)))!;
        expect(preview, orderedEquals(original));
      }
    }

    const galleryChannel = MethodChannel('com.memolanes/screenshots');
    const shareChannel = MethodChannel('dev.fluttercommunity.plus/share');
    const pathChannel = MethodChannel('plugins.flutter.io/path_provider');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    final temp = Directory.systemTemp.createTempSync('memolanes_screenshot_');
    Uint8List? savedPng;
    Uint8List? sharedPng;
    messenger.setMockMethodCallHandler(pathChannel, (_) async => temp.path);
    messenger.setMockMethodCallHandler(shareChannel, (call) async {
      expect(call.method, 'share');
      expect(call.arguments['mimeTypes'], ['image/png']);
      final paths = (call.arguments['paths'] as List).cast<String>();
      sharedPng = File(paths.single).readAsBytesSync();
      expect(call.arguments['originWidth'], greaterThan(0));
      expect(call.arguments['originHeight'], greaterThan(0));
      return ''; // Dismiss the system sheet and retain the preview.
    });
    messenger.setMockMethodCallHandler(galleryChannel, (call) async {
      expect(call.method, 'saveToPhotos');
      savedPng = call.arguments['png'] as Uint8List;
      throw PlatformException(code: 'permission_denied');
    });
    addTearDown(() {
      for (final channel in [galleryChannel, shareChannel, pathChannel]) {
        messenger.setMockMethodCallHandler(channel, null);
      }
      temp.deleteSync(recursive: true);
    });

    // Export immediately after changing the card, before its new frame is painted.
    tester.widget<FilterChip>(cardOption).onSelected!(false);
    for (final label in ['分享', '保存到相册']) {
      await pumpUntil(
        tester,
        () => label == '分享' ? sharedPng != null : savedPng != null,
        start: () => tester
            .widget<AppButton>(find.widgetWithText(AppButton, label))
            .onPressed!(),
      );
      await tester.pumpAndSettle();
    }
    expect(find.byType(ReadOnlyJourneyInfoCard), findsNothing);
    final preview = await previewPixels(tester);
    // Rounded preview corners must not trim the exported image.
    expect(preview.sublist(0, 4), [255, 255, 255, 255]);
    await tester.runAsync(() async {
      for (final png in [sharedPng!, savedPng!]) {
        final image = await decodeScreenshot(png);
        try {
          expect(image.width, 640);
          expect(image.height, 1280);
          expect(await pixels(image), orderedEquals(preview));
        } finally {
          image.dispose();
        }
      }
    });
    expect(find.text('没有保存相册的权限，请在系统设置中允许后重试'), findsOneWidget);
    for (final action in tester.widgetList<AppButton>(find.byType(AppButton))) {
      expect(action.onPressed, isNotNull);
    }
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('capture button reflects the parent busy state', (tester) async {
    var captures = 0;
    for (final busy in [true, false]) {
      await pumpApp(
        tester,
        Scaffold(
          body: MapScreenshotButton(busy: busy, onPressed: () => captures++),
        ),
      );
      expect(
        find.byType(CircularProgressIndicator),
        busy ? findsOneWidget : findsNothing,
      );
      expect(find.byTooltip(busy ? '正在生成截图…' : '截图'), findsOneWidget);
      await tester.tap(find.byType(MapScreenshotButton));
      expect(captures, busy ? 0 : 1);
    }
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('timeline captures its visible frame while loading', (
    tester,
  ) async {
    RustLib.initMock(api: _TimelineApi());
    addTearDown(RustLib.dispose);
    final haptics = AppHaptics.debugHapticsEnabledOverride;
    AppHaptics.debugHapticsEnabledOverride = false;
    addTearDown(() => AppHaptics.debugHapticsEnabledOverride = haptics);
    final key = GlobalKey<TimeMachineOverlayState>();
    await pumpApp(
      tester,
      Scaffold(
        body: AppLayoutBuilder(
          builder: (_, _) =>
              TimeMachineOverlay(key: key, onJourneyRangeLoaded: (_) {}),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 100));
    expect(
      tester.widget<TimeRangePicker>(find.byType(TimeRangePicker)).loading,
      isTrue,
    );
    await tester.runAsync(() async {
      final capture = key.currentState!.captureScreenshot(1);
      await tester.pump();
      final image = await capture;
      expect(image.width, greaterThan(0));
      expect(image.height, greaterThan(0));
      image.dispose();
    });
    await tester.pumpWidget(const SizedBox.shrink());
  });
}

class _TimelineApi extends Fake implements RustLibApi {
  @override
  api.LayerFilter crateApiApiGetCurrentMainMapLayerFilter() => api.LayerFilter(
    defaultKind: true,
    flightKind: false,
    currentJourney: true,
  );

  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async => DateTime(2020);

  @override
  Future<api.MapRendererProxy>
  crateApiApiGetMapRendererProxyForJourneyDateRange({
    required DateTime fromDateInclusive,
    required DateTime toDateInclusive,
    required Set<JourneyKind> journeyKinds,
  }) => Completer<api.MapRendererProxy>().future;
}
