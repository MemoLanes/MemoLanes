import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/foundation.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/body/achievement/achievement_explorer_map_controls.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/achievement/achievement_region_explorer.dart';
import 'package:memolanes/body/achievement/achievement_region_explorer_controller.dart';
import 'package:memolanes/body/achievement/achievement_region_sort.dart';
import 'package:memolanes/body/achievement/cards/achievement_countries_card.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/body/achievement/shared/achievement_region_rows.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart';
import 'package:memolanes/src/rust/api/api.dart' show LayerFilter;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/theme/app_theme.dart';

import 'support/achievement_region_fixtures.dart';
import 'support/duo_hardware_test_data.dart';

const _a = GeoEntityId(field0: 1);
const _b = GeoEntityId(field0: 2);
const _leaf = GeoEntityId(field0: 3);
const _otherSubdivision = GeoEntityId(field0: 4);
const _enUs = Locale('en', 'US');

RegionEntity _row(
  String name,
  int area, {
  bool children = false,
  int? total,
  String? flagCode,
}) => regionRow(
  name,
  children: children,
  area: area * 1000000,
  total: (total ?? area * 2) * 1000000,
  iso: flagCode,
);

Finder _panelHeaderText(String text) => find
    .descendant(
      of: find.byType(SliverPersistentHeader),
      matching: find.text(text),
    )
    .hitTestable();

const _map = ValueKey('test region map');
const _bounds = regionBounds;
const _polygon = regionPolygon;

class _WorldviewApi implements RustLibApi {
  @override
  String crateApiAchievementWorldviewId({required Worldview that}) => that.name;

  @override
  LayerFilter crateApiApiGetCurrentMainMapLayerFilter() =>
      LayerFilter(currentJourney: true, defaultKind: true, flightKind: true);

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Fixture {
  _Fixture({this.failOnce = false, this.failOnceOnParent = false});

  final bool failOnce;
  final bool failOnceOnParent;
  int geometryAttempts = 0;
  int attributionReads = 0;
  final geometryAttemptsById = <GeoEntityId, int>{};
  final manager = regionWorldviewManager();

  AchievementRegionMapState? mapState;

  AchievementRegionExplorer page({
    bool chrome = false,
    RegionRows? rootRows,
    bool longList = false,
    bool differingSort = false,
    bool longChildren = false,
    bool? rawChildren,
    VoidCallback? onClose,
    Key? key,
  }) => AchievementRegionExplorer(
    key: key,
    onClose: onClose ?? () {},
    createController: () => AchievementRegionExplorerController(
      worldviewManager: manager,
      loadRoot: () async =>
          rootRows ??
          {
            if (longList)
              for (var i = 10; i < 30; i++)
                GeoEntityId(field0: i): _row('Region $i', i),
            _a: _row('Canada', 3, total: differingSort ? 12 : null),
            _b: _row(
              'country.CHN',
              2,
              children: true,
              total: differingSort ? 2 : null,
            ),
          },
      loadChildren: (_) async => {
        if (longChildren)
          for (var i = 100; i < 125; i++)
            GeoEntityId(field0: i): _row('Subdivision $i', i),
        if (rawChildren != false)
          _leaf: _row(
            'Test subdivision',
            rawChildren == true ? 0 : 1,
            total: 10,
          ),
        if (differingSort)
          _otherSubdivision: _row('Other subdivision', 2, total: 40),
      },
      loadGeometry: (id) async {
        if (id == _b && !failOnceOnParent) return null;
        geometryAttempts++;
        final attempt = geometryAttemptsById.update(
          id,
          (n) => n + 1,
          ifAbsent: () => 1,
        );
        if (((failOnce && id == _a) || (failOnceOnParent && id == _b)) &&
            attempt == 1) {
          throw StateError('test');
        }
        return regionGeometryFixture(id);
      },
    ),
    controlsBuilder: chrome
        ? (context, region) => AchievementExplorerMapControls(
            region: region,
            readAttribution: () async {
              attributionReads++;
              return null;
            },
          )
        : null,
    mapBuilder: (context, state) {
      mapState = state;
      // In the shell the Explorer only publishes the map viewport.
      if (onClose != null) return const SizedBox.expand(key: _map);
      return Stack(
        key: _map,
        fit: StackFit.expand,
        children: [const ColoredBox(color: Colors.blue)],
      );
    },
  );
}

Future<void> _pumpApp(
  WidgetTester tester,
  Widget home, {
  double scale = 1,
  EdgeInsets padding = EdgeInsets.zero,
  double keyboardInset = 0,
}) async {
  const loader = AppTranslationLoader();
  await tester.runAsync(() async {
    await loader.load('assets/translations', _enUs);
    await tester.pumpWidget(
      EasyLocalization(
        supportedLocales: const [_enUs],
        path: 'assets/translations',
        assetLoader: loader,
        saveLocale: false,
        child: Builder(
          builder: (context) => MaterialApp(
            theme: AppTheme.light,
            localizationsDelegates: context.localizationDelegates,
            supportedLocales: context.supportedLocales,
            locale: context.locale,
            builder: (context, child) => MediaQuery.withClampedTextScaling(
              minScaleFactor: scale,
              maxScaleFactor: scale,
              child: MediaQuery(
                data: MediaQuery.of(context).copyWith(
                  padding: padding,
                  viewPadding: padding,
                  viewInsets: EdgeInsets.only(bottom: keyboardInset),
                ),
                child: child!,
              ),
            ),
            home: home,
          ),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 1));
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
    RustLib.initMock(api: _WorldviewApi());
  });
  tearDownAll(RustLib.dispose);

  for (final (size, keyboard, folded) in [
    (const Size(390, 800), 0.0, false),
    (const Size(900, 500), 0.0, false),
    (const Size(390, 800), 300.0, false),
    (const Size(900, 500), 0.0, true),
  ]) {
    testWidgets(
      'panel canvas reaches bottom inset at $size keyboard $keyboard folded $folded',
      (tester) async {
        await tester.binding.setSurfaceSize(size);
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final f = _Fixture();
        await f.manager.initialize();
        if (folded) {
          debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
          addTearDown(() => debugDefaultTargetPlatformOverride = null);
        }
        final capture = GlobalKey();
        var mapDowns = 0;
        await _pumpApp(
          tester,
          RepaintBoundary(
            key: capture,
            child: Stack(
              fit: StackFit.expand,
              children: [
                Listener(
                  onPointerDown: (_) => mapDowns++,
                  child: const ColoredBox(color: Colors.blue),
                ),
                DuoHardwareScope(
                  hardware: folded
                      ? const DuoHardware(
                          hasHinge: true,
                          status: DuoHingeStatus.partiallyOpen,
                          folds: [
                            DuoRegion(
                              Rect.fromLTWH(450, 0, 20, 500),
                              isActive: true,
                            ),
                          ],
                        )
                      : DuoHardware.none,
                  child: DuoBarScope(
                    edge: folded ? DuoBarEdge.horizontal : DuoBarEdge.unknown,
                    child: f.page(onClose: () {}),
                  ),
                ),
              ],
            ),
          ),
          padding: const EdgeInsets.only(bottom: 34),
          keyboardInset: keyboard,
        );
        final viewport = tester.getRect(find.byKey(_map));
        final revision = f.mapState!.cameraRevision;
        for (final collapsed in [false, true]) {
          if (collapsed) {
            await tester.tap(find.byTooltip('Collapse region list'));
            await tester.pumpAndSettle();
          }
          final panel = tester.getRect(find.byType(CustomScrollView));
          expect(panel.bottom, size.height - keyboard - 34);
          if (folded || size.width == 390) {
            if (collapsed) {
              expect(panel.height, greaterThan(56));
              expect(_panelHeaderText('Canada'), findsOneWidget);
            } else {
              expect(panel.height, closeTo(viewport.height * .55, .01));
            }
          }
          if (folded) {
            expect(panel.left, 470);
            expect(panel.right, 900);
          } else if (size.width == 390) {
            expect(
              f.mapState!.boundsPadding,
              EdgeInsets.fromLTRB(28, 28, 28, panel.height + 32),
            );
          } else {
            expect(panel.width, collapsed ? 56 : 414);
            expect(panel.height, viewport.height);
            expect(
              f.mapState!.boundsPadding,
              EdgeInsets.fromLTRB(28, 28, panel.width + 32, 28),
            );
          }
          expect(f.mapState!.cameraRevision, revision);
          final image = await tester.runAsync(() async {
            final boundary =
                capture.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary;
            return boundary.toImage(pixelRatio: 1);
          });
          final bytes = (await tester.runAsync(
            () => image!.toByteData(format: ui.ImageByteFormat.rawRgba),
          ))!;
          int pixel(int x, int y) =>
              bytes.getUint32((y * image!.width + x) * 4);
          final canvas = pixel(
            panel.right.toInt() - 4,
            panel.bottom.toInt() - 4,
          );
          for (final x in [panel.left.toInt() + 4, panel.right.toInt() - 4]) {
            expect(pixel(x, (size.height - keyboard).toInt() - 2), canvas);
          }
          if (size.width == 900) {
            expect(
              pixel(4, (size.height - keyboard).toInt() - 2),
              0x2196f3ff,
              reason: 'Bottom inset outside panel remains exposed map',
            );
          }
          if (folded) {
            expect(
              pixel(460, size.height.toInt() - 2),
              0x2196f3ff,
              reason: 'Canvas does not paint across fold',
            );
          }
          if (keyboard > 0) {
            expect(
              pixel(size.width.toInt() - 4, size.height.toInt() - 2),
              0x2196f3ff,
              reason: 'Canvas does not enter keyboard clearance',
            );
          }
          image!.dispose();
          final before = mapDowns;
          await tester.tapAt(Offset(20, viewport.top + 20));
          expect(mapDowns, before + 1);
          await tester.tapAt(panel.bottomRight - const Offset(4, 4));
          expect(
            mapDowns,
            before + 1,
            reason: 'Usable panel still intercepts hits',
          );
          await tester.tapAt(Offset(panel.center.dx, panel.bottom + 16));
          expect(
            mapDowns,
            before + 2,
            reason: 'Paint-only inset adds no hit blocker',
          );
        }
      },
    );
  }

  testWidgets(
    'entry publishes the first Area row outline and bounds without a tap',
    (tester) async {
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(
        tester,
        f.page(
          rootRows: {
            _leaf: _row('Zero', 0),
            _b: _row('China', 3),
            _otherSubdivision: _row('Small', 1),
            _a: _row('Canada', 3),
          },
        ),
      );
      expect(
        tester.getTopLeft(find.text('Canada')).dy,
        lessThan(tester.getTopLeft(find.text('China')).dy),
      );
      expect(find.text('Zero'), findsNothing);
      expect(f.mapState!.featureGeojson, _polygon);
      expect(f.mapState!.flyToBounds, _bounds);
      expect(f.mapState!.cameraRevision, 2);
      expect(f.geometryAttempts, 1);
      await tester.tap(find.text('Unlock rate'));
      await tester.pumpAndSettle();
      expect(f.mapState!.cameraRevision, 2);
      expect(f.geometryAttempts, 1);
    },
  );

  testWidgets('held multi-frame finger never snaps before release', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page());
    final revision = f.mapState!.cameraRevision;
    final initialHeight = tester.getSize(find.byType(CustomScrollView)).height;
    final snapSizes = tester
        .widget<DraggableScrollableSheet>(find.byType(DraggableScrollableSheet))
        .snapSizes;
    final gesture = await tester.startGesture(
      tester.getCenter(find.byType(CustomScrollView)),
    );
    await gesture.moveBy(const Offset(0, 20));
    await tester.pump(const Duration(milliseconds: 16));
    await gesture.moveBy(const Offset(0, 40));
    await tester.pump(const Duration(milliseconds: 16));
    final held = tester.getSize(find.byType(CustomScrollView)).height;
    expect(held, lessThan(initialHeight));
    await tester.pump(const Duration(milliseconds: 16));
    await tester.pump(const Duration(milliseconds: 400));
    expect(
      tester.getSize(find.byType(CustomScrollView)).height,
      closeTo(held, .1),
    );
    await gesture.moveBy(const Offset(0, 30));
    await tester.pump(const Duration(milliseconds: 16));
    expect(
      tester.getSize(find.byType(CustomScrollView)).height,
      lessThan(held - 20),
    );
    expect(
      tester
          .widget<DraggableScrollableSheet>(
            find.byType(DraggableScrollableSheet),
          )
          .snapSizes,
      same(snapSizes),
    );
    await gesture.up();
    await tester.pumpAndSettle();
    expect(f.mapState!.cameraRevision, revision);
  });

  testWidgets('near-bottom valid offset survives wide to portrait rotation', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(900, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page(longList: true));
    final scroll = tester.state<ScrollableState>(find.byType(Scrollable));
    scroll.position.jumpTo(scroll.position.maxScrollExtent - 10);
    await tester.pumpAndSettle();
    final offset = scroll.position.pixels;
    await tester.binding.setSurfaceSize(const Size(390, 800));
    await tester.pumpAndSettle();
    expect(scroll.position.maxScrollExtent, greaterThanOrEqualTo(offset));
    expect(scroll.position.pixels, closeTo(offset, .1));
  });

  for (final intermediate in [
    const Size(390, 800),
    const Size(390, 1800),
    const Size(390, 112),
  ]) {
    testWidgets(
      'rapid geometry through $intermediate preserves original valid offset',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(900, 500));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final f = _Fixture();
        await f.manager.initialize();
        await _pumpApp(tester, f.page(longList: true));
        final scroll = tester.state<ScrollableState>(find.byType(Scrollable));
        scroll.position.jumpTo(scroll.position.maxScrollExtent - 10);
        await tester.pumpAndSettle();
        final offset = scroll.position.pixels;
        final revision = f.mapState!.cameraRevision;
        final map = tester.element(find.byKey(_map));
        await tester.binding.setSurfaceSize(intermediate);
        await tester.pump();
        await tester.binding.setSurfaceSize(const Size(900, 500));
        await tester.pumpAndSettle();
        expect(scroll.position.maxScrollExtent, greaterThanOrEqualTo(offset));
        expect(scroll.position.pixels, closeTo(offset, .1));
        expect(f.mapState!.cameraRevision, revision);
        expect(tester.element(find.byKey(_map)), same(map));
      },
    );
  }

  testWidgets(
    'Back restoration wins over outgoing offset on same-frame resize',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      final key = GlobalKey<AchievementRegionExplorerState>();
      await _pumpApp(tester, f.page(longList: true, key: key));
      await tester.ensureVisible(find.text('China'));
      await tester.pumpAndSettle();
      final scroll = tester.state<ScrollableState>(find.byType(Scrollable));
      final offset = scroll.position.pixels;
      expect(offset, greaterThan(0));
      await tester.tap(find.byTooltip('Show subdivisions of China'));
      await tester.pumpAndSettle();
      expect(scroll.position.pixels, 0);
      key.currentState!.requestBack();
      await tester.binding.setSurfaceSize(const Size(900, 450));
      await tester.pumpAndSettle();
      expect(scroll.position.maxScrollExtent, greaterThanOrEqualTo(offset));
      expect(scroll.position.pixels, closeTo(offset, .1));
    },
  );

  for (final navigation in ['drill', 'worldview']) {
    testWidgets(
      '$navigation target wins over geometry snapshot in same frame',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(390, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final f = _Fixture();
        await f.manager.initialize();
        await _pumpApp(tester, f.page(longList: true, longChildren: true));
        await tester.ensureVisible(find.text('China'));
        await tester.pumpAndSettle();
        final scroll = tester.state<ScrollableState>(find.byType(Scrollable));
        expect(scroll.position.pixels, greaterThan(0));
        if (navigation == 'drill') {
          await tester.tap(find.byTooltip('Show subdivisions of China'));
        } else {
          await f.manager.update(Worldview.usa);
        }
        await tester.binding.setSurfaceSize(const Size(900, 450));
        await tester.pumpAndSettle();
        expect(scroll.position.maxScrollExtent, greaterThan(0));
        expect(scroll.position.pixels, 0);
      },
    );
  }

  testWidgets(
    'short stacked map retains both production controls beside usable list',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 212));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(tester, f.page(chrome: true));
      expect(find.byType(LayerButton), findsOneWidget);
      expect(find.byType(MapCopyrightButton), findsOneWidget);
      final layer = tester.getRect(find.byType(LayerButton));
      final attribution = tester.getRect(find.byType(MapCopyrightButton));
      final panel = tester.getRect(find.byType(CustomScrollView));
      expect(layer.height, 44);
      expect(layer.overlaps(attribution), isFalse);
      expect(layer.overlaps(panel), isFalse);
      expect(attribution.overlaps(panel), isFalse);
      expect(layer.center.dy, closeTo(attribution.center.dy, .1));
      expect(find.byType(LayerButton).hitTestable(), findsOneWidget);
      expect(find.byType(MapCopyrightButton).hitTestable(), findsOneWidget);
      expect(panel.height, greaterThan(56));
      await tester.ensureVisible(find.text('Canada'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Canada'));
      await tester.pumpAndSettle();
      expect(f.mapState!.featureGeojson, _polygon);
    },
  );

  testWidgets('collapsed stacked sheet shows the selected region stats', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page());
    await tester.tap(find.text('Canada').hitTestable());
    await tester.pumpAndSettle();
    final revision = f.mapState!.cameraRevision;
    await tester.tap(find.byTooltip('Collapse region list'));
    await tester.pumpAndSettle();

    expect(_panelHeaderText('Canada'), findsOneWidget);
    expect(_panelHeaderText('3.00 km²'), findsOneWidget);
    expect(_panelHeaderText('50.000%'), findsOneWidget);
    expect(f.mapState!.cameraRevision, revision);

    await tester.tap(find.byTooltip('Expand region list'));
    await tester.pumpAndSettle();
    expect(_panelHeaderText('Canada'), findsNothing);
  });

  testWidgets(
    'compact summary shows flag, name and stats on one tappable row',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(
        tester,
        f.page(rootRows: {_a: _row('country.NZL', 3, flagCode: 'NZL')}),
      );
      final scrollView = find.byType(CustomScrollView);
      final handle = find.byKey(
        const ValueKey('achievement-region-drag-handle'),
      );
      double grabberInset() =>
          tester.getCenter(handle).dy - tester.getTopLeft(scrollView).dy;
      final expandedGrabberInset = grabberInset();
      expect(
        tester
                .getTopLeft(find.byType(SegmentedButton<AchievementRegionSort>))
                .dy -
            tester.getTopLeft(scrollView).dy,
        lessThan(56),
      );
      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();

      final flag = find
          .descendant(
            of: find.byType(SliverPersistentHeader),
            matching: find.byType(AchievementCountryFlagBadge),
          )
          .hitTestable();
      expect(flag, findsOneWidget);
      expect(
        tester.widget<AchievementCountryFlagBadge>(flag).countryCode,
        'NZL',
      );
      final name = tester.getCenter(_panelHeaderText('New Zealand'));
      final area = tester.getCenter(_panelHeaderText('3.00 km²'));
      final coverage = tester.getCenter(_panelHeaderText('50.000%'));
      expect(tester.getCenter(flag).dx, lessThan(name.dx));
      expect(name.dx, lessThan(area.dx));
      expect(area.dx, lessThan(coverage.dx));
      expect(area.dy, closeTo(name.dy, 1));
      expect(coverage.dy, closeTo(name.dy, 1));
      expect(grabberInset(), lessThan(16));
      expect(expandedGrabberInset, closeTo(grabberInset(), .01));
      expect(tester.getSize(scrollView).height, lessThan(72));

      await tester.tap(_panelHeaderText('New Zealand'));
      await tester.pumpAndSettle();
      expect(find.byTooltip('Collapse region list'), findsOneWidget);

      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();
      await tester.drag(_panelHeaderText('50.000%'), const Offset(0, -300));
      await tester.pumpAndSettle();
      expect(find.byTooltip('Collapse region list'), findsOneWidget);
    },
  );

  testWidgets('collapsed summary follows the focused parent and subdivision', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page());
    final collapse = find.byTooltip('Collapse region list');
    await tester.tap(find.byTooltip('Show subdivisions of China'));
    await tester.pumpAndSettle();
    await tester.tap(collapse);
    await tester.pumpAndSettle();
    expect(_panelHeaderText('China'), findsOneWidget);
    expect(_panelHeaderText('2.00 km²'), findsOneWidget);
    expect(_panelHeaderText('50.000%'), findsOneWidget);

    await tester.tap(find.byTooltip('Expand region list'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Test subdivision'));
    await tester.pumpAndSettle();
    await tester.tap(collapse);
    await tester.pumpAndSettle();
    expect(_panelHeaderText('China'), findsNothing);
    expect(_panelHeaderText('Test subdivision'), findsOneWidget);
    expect(_panelHeaderText('1.00 km²'), findsOneWidget);
    expect(_panelHeaderText('10.000%'), findsOneWidget);
  });

  testWidgets('collapse with no selected region stays pill-only', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page(rootRows: {}));
    await tester.tap(find.byTooltip('Collapse region list'));
    await tester.pumpAndSettle();
    expect(
      tester.getSize(find.byType(CustomScrollView)).height,
      closeTo(56, .01),
    );
    expect(find.byTooltip('Expand region list').hitTestable(), findsOneWidget);
  });

  testWidgets('long region names leave room for stats at large text sizes', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(320, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    const name = 'A region with a very long localized name that cannot fit';
    await _pumpApp(tester, f.page(rootRows: {_a: _row(name, 3)}), scale: 2.5);
    await tester.tap(find.byTooltip('Collapse region list'));
    await tester.pumpAndSettle();
    expect(_panelHeaderText(name), findsOneWidget);
    expect(_panelHeaderText('3.00 km²'), findsOneWidget);
    expect(_panelHeaderText('50.000%'), findsOneWidget);
    final panel = tester.getRect(find.byType(CustomScrollView));
    for (final text in [name, '3.00 km²', '50.000%']) {
      final bounds = tester.getRect(_panelHeaderText(text));
      expect(panel.contains(bounds.topLeft), isTrue);
      expect(bounds.right, lessThanOrEqualTo(panel.right));
      expect(bounds.bottom, lessThanOrEqualTo(panel.bottom));
    }
  });

  testWidgets('resizing mid-drag preserves a usable expanded stop', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 772));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page());
    final handle = find.byKey(const ValueKey('achievement-region-drag-handle'));
    final panel = find.byType(CustomScrollView);
    await tester.tap(handle);
    await tester.pumpAndSettle();
    final collapsedHeight = tester.getSize(panel).height;
    await tester.tap(handle);
    await tester.pumpAndSettle();
    final drag = await tester.startGesture(tester.getCenter(handle));
    await drag.moveBy(const Offset(0, 24));
    await drag.moveBy(const Offset(0, 270));
    await tester.pump();
    expect(tester.getSize(panel).height, closeTo(126, 1));

    await tester.binding.setSurfaceSize(const Size(390, 372));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Collapse region list'), findsOneWidget);
    final expandedHeight = tester.getSize(panel).height;
    expect(expandedHeight, greaterThan(collapsedHeight));
    await drag.cancel();
    await tester.pumpAndSettle();
    await tester.tap(handle);
    await tester.pumpAndSettle();
    expect(_panelHeaderText('Canada'), findsOneWidget);
    expect(tester.getSize(panel).height, lessThan(expandedHeight));
  });

  testWidgets('collapsed summary stays physical height on same-axis resize', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 650));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page());
    await tester.tap(find.byTooltip('Collapse region list'));
    await tester.pumpAndSettle();
    final height = tester.getSize(find.byType(CustomScrollView)).height;
    final revision = f.mapState!.cameraRevision;
    await tester.binding.setSurfaceSize(const Size(390, 1000));
    await tester.pumpAndSettle();
    expect(
      tester.getSize(find.byType(CustomScrollView)).height,
      closeTo(height, .1),
    );
    expect(_panelHeaderText('Canada'), findsOneWidget);
    expect(f.mapState!.cameraRevision, revision);
    expect(find.byTooltip('Expand region list'), findsOneWidget);
  });

  testWidgets(
    'short narrow stacked action expands to reachable rows above real clear map',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 200));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(tester, f.page());
      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();
      expect(_panelHeaderText('Canada'), findsNothing);
      expect(
        tester.getSize(find.byType(CustomScrollView)).height,
        closeTo(56, .01),
      );
      await tester.tap(find.byTooltip('Expand region list'));
      await tester.pumpAndSettle();
      expect(
        tester.getSize(find.byType(CustomScrollView)).height,
        greaterThan(56),
      );
      await tester.ensureVisible(find.text('Canada'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Canada'));
      await tester.pumpAndSettle();
      expect(f.mapState!.featureGeojson, _polygon);
      final map = tester.getRect(find.byKey(_map));
      final panel = tester.getRect(find.byType(CustomScrollView));
      final padding = f.mapState!.boundsPadding;
      expect(map.height - padding.vertical, greaterThan(0));
      expect(map.bottom - padding.bottom, lessThanOrEqualTo(panel.top));
      expect(panel.top, greaterThan(map.top));
    },
  );

  testWidgets(
    'production-sized layer and attribution do not overlap at expanded portrait',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(tester, f.page(chrome: true));
      await tester.drag(find.byType(CustomScrollView), const Offset(0, -600));
      await tester.pumpAndSettle();
      final layer = tester.getRect(find.byType(LayerButton));
      final attribution = tester.getRect(find.byType(MapCopyrightButton));
      final panel = tester.getRect(find.byType(CustomScrollView));
      expect(layer.overlaps(attribution), isFalse);
      expect(layer.overlaps(panel), isFalse);
      expect(attribution.overlaps(panel), isFalse);
    },
  );

  testWidgets(
    'portrait handle drags to collapse without a camera command and reopens on tap',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(tester, f.page(longList: true));
      final semantics = tester.ensureSemantics();
      expect(find.bySemanticsLabel('Region list'), findsOneWidget);
      semantics.dispose();
      final sheet = tester.widget<DraggableScrollableSheet>(
        find.byType(DraggableScrollableSheet),
      );
      expect(sheet.shouldCloseOnMinExtent, isFalse);
      expect(sheet.snap, isTrue);
      expect(sheet.minChildSize, lessThan(sheet.initialChildSize));
      expect(sheet.initialChildSize, lessThan(sheet.maxChildSize));
      final revision = f.mapState!.cameraRevision;
      final initialPadding = f.mapState!.boundsPadding;
      final mapElement = tester.element(find.byKey(_map));
      final handle = find.byKey(
        const ValueKey('achievement-region-drag-handle'),
      );
      expect(handle, findsOneWidget);
      await tester.drag(handle, const Offset(0, 500));
      await tester.pumpAndSettle();
      expect(find.byTooltip('Expand region list'), findsOneWidget);
      expect(f.mapState!.cameraRevision, revision);
      expect(f.mapState!.boundsPadding.bottom, lessThan(initialPadding.bottom));
      await tester.tap(handle);
      await tester.pumpAndSettle();
      await tester.drag(find.byType(CustomScrollView), const Offset(0, -600));
      await tester.pumpAndSettle();
      await tester.drag(find.byType(CustomScrollView), const Offset(0, -600));
      await tester.pumpAndSettle();
      final scroll = tester.state<ScrollableState>(
        find.byType(Scrollable).first,
      );
      final offset = scroll.position.pixels;
      expect(offset, greaterThan(0));
      final padding = f.mapState!.boundsPadding;
      await tester.drag(find.byType(CustomScrollView), const Offset(0, 80));
      await tester.pumpAndSettle();
      expect(f.mapState!.boundsPadding, padding);
      final saved = scroll.position.pixels;
      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Expand region list'));
      await tester.pumpAndSettle();
      expect(scroll.position.pixels, saved);
      expect(tester.element(find.byKey(_map)), same(mapElement));
      expect(f.mapState!.cameraRevision, revision);
    },
  );

  testWidgets('wide action preserves scroll and rotation keeps map mounted', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(900, 500));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page(longList: true));
    final element = tester.element(find.byKey(_map));
    final revision = f.mapState!.cameraRevision;
    await tester.drag(find.byType(CustomScrollView), const Offset(0, -400));
    await tester.pumpAndSettle();
    final scroll = tester.state<ScrollableState>(find.byType(Scrollable).first);
    final offset = scroll.position.pixels;
    final before = f.mapState!.boundsPadding;
    await tester.tap(find.byTooltip('Collapse region list'));
    await tester.pumpAndSettle();
    expect(f.mapState!.boundsPadding.right, lessThan(before.right));
    expect(_panelHeaderText('Region 29'), findsNothing);
    await tester.tap(find.byTooltip('Expand region list'));
    await tester.pumpAndSettle();
    expect(scroll.position.pixels, offset);
    await tester.binding.setSurfaceSize(const Size(390, 800));
    await tester.pumpAndSettle();
    expect(tester.element(find.byKey(_map)), same(element));
    expect(f.mapState!.cameraRevision, revision);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'next selection uses resized padding, safe collapsed action stays reachable',
    (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final f = _Fixture();
      await f.manager.initialize();
      await _pumpApp(tester, f.page());
      await tester.tap(find.text('Canada'));
      await tester.pumpAndSettle();
      final revision = f.mapState!.cameraRevision;
      final before = f.mapState!.boundsPadding;
      await tester.drag(find.byType(CustomScrollView), const Offset(0, -220));
      await tester.pumpAndSettle();
      final resized = f.mapState!.boundsPadding;
      expect(resized.bottom, greaterThan(before.bottom));
      expect(f.mapState!.cameraRevision, revision);
      await tester.tap(find.text('Canada'));
      await tester.pumpAndSettle();
      expect(f.mapState!.boundsPadding, resized);
      expect(f.mapState!.cameraRevision, revision + 1);
      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();
      expect(f.mapState!.featureGeojson, _polygon);
      expect(find.byType(Scrollable), findsOneWidget);
      await tester.tap(find.byTooltip('Expand region list'));
      await tester.pumpAndSettle();
      expect(f.mapState!.featureGeojson, _polygon);
    },
  );

  for (final (size, safeInsets) in [
    (const Size(390, 800), const EdgeInsets.only(top: 60, bottom: 34)),
    (const Size(900, 500), const EdgeInsets.fromLTRB(47, 0, 51, 22)),
  ]) {
    testWidgets(
      'embedded Explorer leaves the retained map interactive at $size',
      (tester) async {
        await tester.binding.setSurfaceSize(size);
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final fixture = _Fixture();
        await fixture.manager.initialize();
        final explorerOpen = ValueNotifier(false);
        addTearDown(explorerOpen.dispose);
        var mapDowns = 0;
        var mapTaps = 0;
        var mapPan = Offset.zero;
        await _pumpApp(
          tester,
          AppNavigationShell(
            navigationVisible: false,
            navigationBuilder: (_, {required axis}) => const SizedBox.shrink(),
            retainedMap: Listener(
              onPointerDown: (_) => mapDowns++,
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () => mapTaps++,
                onPanUpdate: (details) => mapPan += details.delta,
              ),
            ),
            bodyBuilder: ({required topSafeArea, required bottomSafeArea}) =>
                ValueListenableBuilder(
                  valueListenable: explorerOpen,
                  builder: (context, open, _) => Stack(
                    fit: StackFit.expand,
                    children: [
                      const SizedBox.expand(),
                      if (open)
                        fixture.page(
                          chrome: true,
                          onClose: () => explorerOpen.value = false,
                        ),
                    ],
                  ),
                ),
          ),
          padding: safeInsets,
        );

        const mapPoint = Offset(100, 240);
        const forward = Offset(140, 0);
        await tester.dragFrom(mapPoint, forward);
        final homePan = mapPan.dx;
        expect(homePan, greaterThan(100));
        explorerOpen.value = true;
        await tester.pumpAndSettle();
        mapDowns = 0;
        mapPan = Offset.zero;

        final viewport = tester.getRect(find.byKey(_map));
        expect(viewport.left, safeInsets.left);
        expect(viewport.right, size.width - safeInsets.right);
        expect(viewport.bottom, size.height - safeInsets.bottom);
        final back = find.byIcon(Icons.arrow_back_ios_new);
        expect(tester.getRect(back).bottom, lessThan(viewport.top));
        expect(viewport.top, greaterThan(safeInsets.top));

        final revision = fixture.mapState!.cameraRevision;
        await tester.tap(find.byTooltip('Collapse region list'));
        await tester.pumpAndSettle();
        final expand = find.byTooltip('Expand region list');
        expect(expand, findsOneWidget);
        expect(mapDowns, 0);

        await tester.dragFrom(mapPoint, forward);
        final explorerPan = mapPan.dx;
        expect(
          explorerPan,
          closeTo(homePan, .01),
          reason: 'Hit path: ${tester.hitTestOnBinding(mapPoint)}',
        );
        await tester.dragFrom(mapPoint + forward, -forward);
        expect(mapPan.dx, closeTo(0, .01));
        await tester.tapAt(mapPoint);
        expect(mapTaps, 1);
        expect(mapDowns, 3);
        expect(fixture.mapState!.cameraRevision, revision);

        await tester.tap(find.byType(MapCopyrightButton));
        await tester.pumpAndSettle();
        expect(fixture.attributionReads, 1);
        await tester.tap(find.byType(LayerButton));
        await tester.pumpAndSettle();
        expect(find.byType(LayerPopupContent), findsOneWidget);
        await tester.tap(find.byType(LayerButton));
        await tester.pumpAndSettle();
        expect(find.byType(LayerPopupContent), findsNothing);
        await tester.tap(expand);
        await tester.pumpAndSettle();
        expect(expand, findsNothing);
        await tester.ensureVisible(find.text('Canada'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Canada'));
        await tester.pumpAndSettle();
        expect(fixture.mapState!.featureGeojson, _polygon);
        expect(mapDowns, 3);

        // The initially exposed map still works with the sheet open.
        await tester.tapAt(mapPoint);
        expect(mapTaps, 2);
        expect(mapDowns, 4);
        await tester.tap(back);
        await tester.pumpAndSettle();
        expect(explorerOpen.value, isFalse);
        expect(mapDowns, 4);
      },
    );
  }

  testWidgets('shell Back drills once before closing Explorer', (tester) async {
    final fixture = _Fixture();
    await fixture.manager.initialize();
    var closed = 0;
    final key = GlobalKey<AchievementRegionExplorerState>();
    await _pumpApp(
      tester,
      PopScope(
        canPop: false,
        onPopInvokedWithResult: (didPop, _) {
          if (!didPop) key.currentState?.requestBack();
        },
        child: fixture.page(key: key, onClose: () => closed++),
      ),
    );
    await tester.tap(find.byTooltip('Show subdivisions of China'));
    await tester.pumpAndSettle();
    expect(find.text('Test subdivision'), findsOneWidget);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.text('Canada'), findsOneWidget);
    expect(closed, 0);
    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(closed, 1);
  });

  testWidgets('View all opens Explorer in place; only the chevron drills', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final fixture = _Fixture();
    await fixture.manager.initialize();
    var open = false;
    await _pumpApp(
      tester,
      StatefulBuilder(
        builder: (context, setState) => open
            ? fixture.page(
                longList: true,
                onClose: () => setState(() => open = false),
              )
            : Scaffold(
                body: AchievementCountriesHeader(
                  count: 1,
                  hasCountries: true,
                  onOpenExplorer: () => setState(() => open = true),
                ),
              ),
      ),
    );
    await tester.tap(find.text('View all'));
    await tester.pumpAndSettle();
    final explorer = find.byType(AchievementRegionExplorer);
    expect(explorer, findsOneWidget);
    expect(Navigator.canPop(tester.element(explorer)), isFalse);
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(fixture.mapState!.flyToBounds, _bounds);

    await tester.ensureVisible(find.text('China'));
    await tester.pumpAndSettle();
    final scroll = tester.state<ScrollableState>(find.byType(Scrollable).first);
    final offset = scroll.position.pixels;
    expect(offset, greaterThan(0));
    await tester.tap(find.text('Canada'));
    await tester.pumpAndSettle();
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(fixture.mapState!.flyToBounds, _bounds);
    final revision = fixture.mapState!.cameraRevision;
    expect(find.text('China'), findsOneWidget);
    expect(find.text('Test subdivision'), findsNothing);

    await tester.tapAt(
      tester.getTopLeft(find.byKey(_map)) + const Offset(30, 30),
    );
    await tester.pumpAndSettle();
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(fixture.mapState!.cameraRevision, revision);
    expect(scroll.position.pixels, offset);

    await tester.tap(find.text('Canada'));
    await tester.pumpAndSettle();
    expect(fixture.mapState!.cameraRevision, revision + 1);
    expect(fixture.mapState!.flyToBounds, _bounds);
    expect(fixture.geometryAttempts, 2);

    final drill = find.byTooltip('Show subdivisions of China');
    await tester.tap(drill);
    await tester.pumpAndSettle();
    expect(find.text('Test subdivision'), findsOneWidget);
    expect(find.text('Canada'), findsNothing);
    expect(fixture.mapState!.featureGeojson, isNull);
    await tester.tap(find.byIcon(Icons.arrow_back_ios_new));
    await tester.pumpAndSettle();
    expect(find.text('Test subdivision'), findsNothing);
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(fixture.mapState!.cameraRevision, revision + 2);
    expect(scroll.position.pixels, offset);
    await tester.tap(find.byIcon(Icons.arrow_back_ios_new));
    await tester.pumpAndSettle();
    expect(find.text('View all'), findsOneWidget);
  });

  testWidgets('Back restores the parent ranking after sorting subdivisions', (
    tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(390, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final f = _Fixture();
    await f.manager.initialize();
    await _pumpApp(tester, f.page(differingSort: true));
    double y(String name) => tester.getTopLeft(find.text(name)).dy;
    expect(y('Canada'), lessThan(y('China')));
    await tester.tap(find.byTooltip('Show subdivisions of China'));
    await tester.pumpAndSettle();
    expect(y('Other subdivision'), lessThan(y('Test subdivision')));
    await tester.tap(find.text('Unlock rate'));
    await tester.pumpAndSettle();
    expect(y('Test subdivision'), lessThan(y('Other subdivision')));
    await tester.tap(find.byIcon(Icons.arrow_back_ios_new));
    await tester.pumpAndSettle();
    expect(y('Canada'), lessThan(y('China')));
  });

  for (final rawChildren in [false, true]) {
    testWidgets(
      'subdivision empty state distinguishes supported stats ($rawChildren)',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(390, 800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final f = _Fixture();
        await f.manager.initialize();
        await _pumpApp(tester, f.page(rawChildren: rawChildren));
        await tester.tap(find.byTooltip('Show subdivisions of China'));
        await tester.pumpAndSettle();
        expect(
          find.text(
            rawChildren ? 'No subdivisions unlocked yet.' : 'Subdivision statistics are not yet available for this region.',
          ),
          findsOneWidget,
        );
      },
    );
  }

  for (final size in [const Size(390, 800), const Size(640, 240)]) {
    testWidgets('map and list share $size with large text', (tester) async {
      await tester.binding.setSurfaceSize(size);
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final fixture = _Fixture();
      await fixture.manager.initialize();
      await _pumpApp(
        tester,
        fixture.page(),
        scale: 2.5,
        padding: const EdgeInsets.fromLTRB(12, 24, 12, 20),
      );
      await tester.tap(find.byTooltip('Collapse region list'));
      await tester.pumpAndSettle();
      final action = tester.getRect(find.byTooltip('Expand region list'));
      expect(action.bottom, lessThanOrEqualTo(size.height - 20));
      expect(action.top, greaterThanOrEqualTo(24));
      await tester.tap(find.byTooltip('Expand region list'));
      await tester.pumpAndSettle();
      final map = tester.getRect(find.byKey(_map));
      final list = tester.getRect(find.byType(Scrollable).first);
      final padding = fixture.mapState!.boundsPadding;
      expect(map.width - padding.horizontal, greaterThan(0));
      expect(map.height - padding.vertical, greaterThan(0));
      expect(list.width, greaterThan(0));
      expect(list.height, greaterThan(0));
      if (size.width > size.height) {
        expect(padding.right, greaterThan(0));
      } else {
        expect(padding.bottom, greaterThan(0));
      }
      expect(tester.takeException(), isNull);
    });
  }

  testWidgets('a missing outline or failed request does not block the list', (
    tester,
  ) async {
    final fixture = _Fixture(failOnce: true);
    await fixture.manager.initialize();
    await _pumpApp(tester, fixture.page());
    expect(find.text('Could not load the region outline.'), findsOneWidget);
    expect(fixture.mapState!.flyToBounds, isNull);
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(find.text('Could not load the region outline.'), findsNothing);
    await tester.tap(find.text('China'));
    await tester.pumpAndSettle();
    expect(find.text('Region outline unavailable.'), findsOneWidget);
    expect(fixture.mapState!.flyToBounds, isNull);
    await tester.tap(find.byTooltip('Show subdivisions of China'));
    await tester.pumpAndSettle();
    expect(find.text('Test subdivision'), findsOneWidget);
  });

  testWidgets('retry recovers the outline of the drilled parent', (
    tester,
  ) async {
    final fixture = _Fixture(failOnceOnParent: true);
    await fixture.manager.initialize();
    await _pumpApp(tester, fixture.page());
    await tester.tap(find.byTooltip('Show subdivisions of China'));
    await tester.pumpAndSettle();
    expect(find.text('Test subdivision'), findsOneWidget);
    expect(find.text('Could not load the region outline.'), findsOneWidget);
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(find.text('Test subdivision'), findsOneWidget);
    expect(find.text('Retry'), findsNothing);

    await tester.tap(find.byIcon(Icons.arrow_back_ios_new));
    await tester.pumpAndSettle();
    expect(find.text('Canada'), findsOneWidget);
    expect(fixture.mapState!.featureGeojson, _polygon);
    expect(fixture.mapState!.flyToBounds, _bounds);
  });
}
