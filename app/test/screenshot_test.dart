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

  Widget app({bool enabled = true, String scene = 'record'}) => Scaffold(
    body: Center(
      child: MapScreenshotButton(
        key: ValueKey(scene),
        onPressed: enabled ? () => captures++ : null,
      ),
    ),
  );

  Future<void> pumpApp(WidgetTester tester, Widget home) async {
    await tester.runAsync(() async {
      await tester.pumpWidget(
        EasyLocalization(
          supportedLocales: const [locale],
          path: 'assets/translations',
          assetLoader: loader,
          fallbackLocale: locale,
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

  Future<void> waitForLogo(WidgetTester tester) async {
    final logoSwitch = find.byType(SwitchListTile).last;
    await tester.runAsync(() async {
      final deadline = DateTime.now().add(const Duration(seconds: 10));
      while (tester.widget<SwitchListTile>(logoSwitch).onChanged == null &&
          DateTime.now().isBefore(deadline)) {
        await Future<void>.delayed(const Duration(milliseconds: 10));
        await tester.pump();
      }
    });
    expect(tester.widget<SwitchListTile>(logoSwitch).onChanged, isNotNull);
  }

  testWidgets(
    'journey preview can independently hide its card and default logo',
    (tester) async {
      final recorder = ui.PictureRecorder();
      Canvas(recorder).drawColor(Colors.blue, BlendMode.src);
      final picture = recorder.endRecording();
      final map = (await tester.runAsync(() => picture.toImage(640, 1280)))!;
      final centeredMap = (await tester.runAsync(
        () => picture.toImage(640, 1280),
      ))!;
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
          centeredJourneyMap: centeredMap,
        ),
      );
      await waitForLogo(tester);
      final cardSwitch = find.widgetWithText(SwitchListTile, '保留旅程信息');
      final logoSwitch = find.widgetWithText(SwitchListTile, '在右上角加入 Logo');
      expect(tester.widget<SwitchListTile>(cardSwitch).value, isTrue);
      expect(tester.widget<SwitchListTile>(logoSwitch).value, isTrue);
      expect(find.byType(ReadOnlyJourneyInfoCard), findsOneWidget);
      expect(find.byType(RawImage), findsNWidgets(2));
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(map),
      );

      await tester.tap(cardSwitch);
      await tester.pumpAndSettle();
      expect(find.byType(ReadOnlyJourneyInfoCard), findsNothing);
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(centeredMap),
      );
      expect(find.byType(RawImage), findsNWidgets(2));
      await tester.tap(logoSwitch);
      await tester.pumpAndSettle();
      expect(find.byType(RawImage), findsOneWidget);
      await tester.tap(cardSwitch);
      await tester.pumpAndSettle();
      expect(find.byType(ReadOnlyJourneyInfoCard), findsOneWidget);
      expect(
        tester.widget<RawImage>(find.byType(RawImage).first).image,
        same(map),
      );
      expect(tester.widget<SwitchListTile>(logoSwitch).value, isFalse);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
      map.dispose();
      centeredMap.dispose();
    },
  );

  testWidgets('preview retains export resolution and equal, readable actions', (
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
      tester.widget<SwitchListTile>(find.byType(SwitchListTile)).value,
      isTrue,
    );
    expect(find.text('保留旅程信息'), findsNothing);
    final actions = find.byType(AppButton);
    expect(actions, findsNWidgets(3));
    expect(tester.widget<AppButton>(actions.at(1)).label, '保存到相册');
    final width = tester.getSize(actions.first).width;
    for (final action in actions.evaluate()) {
      expect(tester.getSize(find.byWidget(action.widget)).width, width);
    }
    final saveLabel = find.descendant(
      of: actions.at(1),
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
      tester.widget<AppButton>(actions.at(1)).onPressed!();
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

  for (final includeLogo in [true, false]) {
    testWidgets(
      'journey preview, gallery and share retain centered pixels with logo $includeLogo',
      (tester) async {
        // Distinct route positions detect exporting the original map by mistake.
        Future<ui.Image> routeImage(double centerY) async {
          final recorder = ui.PictureRecorder();
          final canvas = Canvas(recorder);
          canvas.drawColor(Colors.white, BlendMode.src);
          canvas.drawRect(
            Rect.fromCenter(
              center: Offset(320, centerY),
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

        final map = (await tester.runAsync(() => routeImage(400)))!;
        final centeredMap = (await tester.runAsync(() => routeImage(640)))!;
        addTearDown(() {
          map.dispose();
          centeredMap.dispose();
        });
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
            centeredJourneyMap: centeredMap,
          ),
        );
        await waitForLogo(tester);
        if (!includeLogo) {
          await tester.tap(find.widgetWithText(SwitchListTile, '在右上角加入 Logo'));
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

        final cardSwitch = find.widgetWithText(SwitchListTile, '保留旅程信息');
        for (final includeCard in [false, true, false]) {
          // Deliberately export before pumping the switch's new frame.
          tester.widget<SwitchListTile>(cardSwitch).onChanged!(includeCard);
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
              expect(
                previewPixels.sublist(center, center + 4),
                includeCard ? white : blue,
              );
              expect(
                previewPixels.sublist(originalCenter, originalCenter + 4),
                includeCard ? blue : white,
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

  testWidgets('first tap reveals; second tap captures; every capture resets', (
    tester,
  ) async {
    await pumpApp(tester, app());
    final button = find.byType(MapScreenshotButton);
    await tester.tap(button);
    await tester.pump();
    expect(captures, 0);
    expect(
      tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity,
      1,
    );
    await tester.tap(button);
    expect(captures, 1);
    await tester.tap(button);
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('confirmation expires even when the button has focus', (
    tester,
  ) async {
    await pumpApp(tester, app());
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    final button = find.byType(MapScreenshotButton);
    await tester.tap(button);
    await tester.pump(const Duration(seconds: 3));
    expect(
      tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity,
      lessThan(1),
    );
    await tester.tap(button);
    expect(captures, 0);
    await tester.tap(button);
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('cancelled contact does not count as confirmation', (
    tester,
  ) async {
    await pumpApp(tester, app());
    final button = find.byType(MapScreenshotButton);
    final gesture = await tester.startGesture(tester.getCenter(button));
    await gesture.cancel();
    await tester.tap(button);
    expect(captures, 0);
    await tester.tap(button);
    expect(captures, 1);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('hover keeps the button visible without keeping it armed', (
    tester,
  ) async {
    await pumpApp(tester, app());
    final button = find.byType(MapScreenshotButton);
    await tester.tap(button);
    final mouse = await tester.createGesture(kind: ui.PointerDeviceKind.mouse);
    await mouse.addPointer(location: Offset.zero);
    await mouse.moveTo(tester.getCenter(button));
    await tester.pump(const Duration(seconds: 3));
    await tester.tap(button);
    expect(captures, 0);
    await mouse.removePointer();
    await tester.pumpWidget(const SizedBox.shrink());
  });

  for (final changeScene in [false, true]) {
    testWidgets(
      'confirmation resets after ${changeScene ? 'changing scene' : 'disabling'}',
      (tester) async {
        await pumpApp(tester, app());
        await tester.tap(find.byType(MapScreenshotButton));
        if (changeScene) {
          await pumpApp(tester, app(scene: 'timeline'));
        } else {
          await pumpApp(tester, app(enabled: false));
          await pumpApp(tester, app());
        }
        await tester.tap(find.byType(MapScreenshotButton));
        expect(captures, 0);
        await tester.tap(find.byType(MapScreenshotButton));
        expect(captures, 1);
        await tester.pumpWidget(const SizedBox.shrink());
      },
    );
  }
}
