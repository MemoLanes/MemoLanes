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
