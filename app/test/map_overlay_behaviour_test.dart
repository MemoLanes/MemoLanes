import 'package:wolt_modal_sheet/wolt_modal_sheet.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'support/duo_hardware_test_data.dart';

import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/body/journey/journey_body.dart';
import 'package:memolanes/body/journey/editor/journey_track_edit_mode_bar.dart';
import 'package:memolanes/body/journey/list/journey_list_calendar.dart';
import 'package:memolanes/body/map/overlay/journey_overlay.dart';
import 'package:memolanes/body/map/overlay/journey_detail_card.dart';
import 'package:memolanes/body/map/overlay/normal_map_overlay.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/component/app_calendar_mode_picker.dart';
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/component/map_controls/layer_button.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/app_layout.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_theme.dart';
import 'package:provider/provider.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

void main() {
  final originalWakelock = wakelockPlusPlatformInstance;
  final originalHaptics = AppHaptics.debugHapticsEnabledOverride;
  final fixtureApi = _FixtureApi();
  setUpAll(() async {
    wakelockPlusPlatformInstance = _NoWakelock();
    AppHaptics.debugHapticsEnabledOverride = false;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(
          const MethodChannel('plugins.flutter.io/shared_preferences'),
          (call) async => call.method == 'getAll' ? <String, Object>{} : null,
        );
    await EasyLocalization.ensureInitialized();
    await const AppTranslationLoader().load(
      'assets/translations',
      const Locale('en', 'US'),
    );
    RustLib.initMock(api: fixtureApi);
  });
  tearDownAll(() {
    wakelockPlusPlatformInstance = originalWakelock;
    AppHaptics.debugHapticsEnabledOverride = originalHaptics;
    RustLib.dispose();
  });

  testWidgets(
    'recording state changes keep the primary controls inside a clear pane',
    (tester) async {
      final gps = _NoLocation();
      addTearDown(gps.dispose);
      final layout = await _pumpScene(
        tester,
        _viewports.firstWhere((v) => v.name == 'book'),
        _OverlayMode.record,
        overlayBuilder: (_) => ChangeNotifierProvider<GpsManager>.value(
          value: gps,
          child: NormalMapOverlay(
            trackingMode: TrackingMode.off,
            onTrackingPressed: () {},
          ),
        ),
      );
      for (final status in [
        GpsRecordingStatus.recording,
        GpsRecordingStatus.paused,
      ]) {
        gps.setStatus(status);
        await tester.pumpAndSettle();
        final action = find.byIcon(
          status == GpsRecordingStatus.recording
              ? Icons.pause_rounded
              : Icons.play_arrow_rounded,
        );
        final surface = find
            .ancestor(of: action, matching: find.byType(LiquidGlassSurface))
            .first;
        final bounds = tester.getRect(surface);
        expect(
          layout.regions.any((region) => _contains(region, bounds)),
          isTrue,
        );
        expect(action.hitTestable(), findsOneWidget);
        expect(find.byIcon(Icons.stop_rounded).hitTestable(), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'toolbar leaves map gestures available beside its visible controls',
    (tester) async {
      var mapTaps = 0;
      var saves = 0;
      await _pumpScene(
        tester,
        const _Viewport('ordinary-toolbar', Size(844, 600), DuoBarEdge.unknown),
        _OverlayMode.record,
        navigationVisible: false,
        overlayBuilder: (layout) => Stack(
          children: [
            Positioned.fill(
              child: GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTap: () => mapTaps++,
              ),
            ),
            _editorModeBar(layout, false, onSave: () => saves++),
          ],
        ),
      );
      final surface = tester.getRect(
        find
            .descendant(
              of: find.byType(ModeSwitchBar),
              matching: find.byType(LiquidGlassSurface),
            )
            .first,
      );
      await tester.tapAt(Offset(20, surface.center.dy));
      expect(mapTaps, 1);
      await tester.tap(find.text(tr('journey.editor.save')).hitTestable());
      await tester.pumpAndSettle();
      expect(saves, 1);
      expect(mapTaps, 1);
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets(
    'journey picker retains its floating card and calendar/list arrangement',
    (tester) async {
      final fixture = fixtureApi;
      fixture.journeyCount = 25;
      addTearDown(() => fixture.journeyCount = 1);
      for (final (viewport, horizontal) in [
        (
          const _Viewport(
            'portrait',
            Size(393, 852),
            DuoBarEdge.unknown,
            padding: EdgeInsets.fromLTRB(0, 59, 0, 34),
          ),
          false,
        ),
        (_viewports.firstWhere((scene) => scene.name == 'book'), false),
        (
          const _Viewport(
            'landscape',
            Size(844, 390),
            DuoBarEdge.unknown,
            padding: EdgeInsets.fromLTRB(44, 0, 22, 21),
          ),
          true,
        ),
      ]) {
        final layout = await _pumpScene(
          tester,
          viewport,
          _OverlayMode.journeys,
        );
        final card = tester.getRect(find.byType(JourneyInfoPanelSurface));
        _expectInRect(card, layout.pickerBounds, 'floating picker');
        final calendar = tester.getRect(find.byType(JourneyListCalendar));
        final list = find.descendant(
          of: find.byType(JourneyBody),
          matching: find.byType(ListView),
        );
        final listBounds = tester.getRect(list);
        if (horizontal) {
          expect(listBounds.left, greaterThanOrEqualTo(calendar.right));
        } else {
          expect(listBounds.top, greaterThanOrEqualTo(calendar.bottom));
          expect(find.text(tr('journey.records_title')), findsOneWidget);
        }
        final scrollable = find.descendant(
          of: list,
          matching: find.byType(Scrollable),
        );
        final position = tester.state<ScrollableState>(scrollable).position;
        expect(position.maxScrollExtent, greaterThan(0));
        position.jumpTo(position.maxScrollExtent);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      }
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  const textScale = 1.8;
  testWidgets(
    'Chinese picker keeps controls and rows reachable at $textScale',
    (tester) async {
      final portrait = _Viewport(
        'inner-portrait',
        const Size(669, 951),
        DuoBarEdge.horizontal,
        padding: const EdgeInsets.fromLTRB(0, 24, 0, 34),
        textScale: textScale,
      );
      JourneyHeader? selected;
      Widget picker(AppLayout _) => JourneyOverlay(
        onJourneySelected: (journey) async => selected = journey,
        isLoading: false,
        refreshRevision: 0,
      );
      await _pumpScene(
        tester,
        portrait,
        _OverlayMode.journeys,
        locale: const Locale('zh', 'CN'),
        overlayBuilder: picker,
      );
      final calendar = find.byType(JourneyListCalendar);
      final controller = tester
          .widget<JourneyListCalendar>(calendar)
          .controller;
      final date = controller.selectedDate;
      final next = find.descendant(
        of: calendar,
        matching: find.byIcon(Icons.chevron_right),
      );
      await tester.ensureVisible(next);
      await tester.pumpAndSettle();
      expect(next.hitTestable(), findsOneWidget);
      final year = find
          .descendant(
            of: calendar,
            matching: find.byType(AppCalendarModePicker),
          )
          .first;
      await tester.ensureVisible(year);
      await tester.pumpAndSettle();
      await tester.tap(year);
      await tester.pumpAndSettle();
      expect(
        tester
            .widget<CalendarDatePicker2>(find.byType(CalendarDatePicker2))
            .config
            .calendarViewMode,
        CalendarDatePicker2Mode.year,
      );
      await _pumpScene(
        tester,
        _Viewport(
          'keyboard',
          portrait.size,
          portrait.edge,
          padding: portrait.padding,
          keyboard: 300,
          textScale: textScale,
        ),
        _OverlayMode.journeys,
        locale: const Locale('zh', 'CN'),
        overlayBuilder: picker,
      );
      expect(
        tester.widget<JourneyListCalendar>(calendar).controller,
        same(controller),
      );
      expect(controller.selectedDate, date);
      final row = find
          .descendant(
            of: find.byType(JourneyBody),
            matching: find.byType(LabelTile),
          )
          .first;
      await tester.ensureVisible(row);
      await tester.pumpAndSettle();
      expect(row.hitTestable(), findsOneWidget);
      await tester.tap(row);
      await tester.pumpAndSettle();
      expect(selected?.id, 'fixture');
      expect(tester.takeException(), isNull);
    },
    variant: const TargetPlatformVariant({TargetPlatform.iOS}),
  );

  testWidgets('short side navigation keeps hidden tabs reachable', (
    tester,
  ) async {
    int? selected;
    final layout = await _pumpScene(
      tester,
      const _Viewport(
        'short-window',
        Size(600, 240),
        DuoBarEdge.right,
        padding: EdgeInsets.all(12),
      ),
      _OverlayMode.record,
      onNavigationSelected: (index) => selected = index,
    );
    await tester.tap(find.byIcon(Icons.more_horiz_rounded));
    await tester.pumpAndSettle();
    final cardBounds = tester.getRect(find.byType(AppDialogContent));
    _expectInRect(cardBounds, Offset.zero & layout.size, 'overflow menu');
    expect(find.text('Achievements'), findsOneWidget);
    expect(find.text('Settings'), findsOneWidget);
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();
    expect(selected, 4);
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('open layer popup follows book and tabletop folds', (
    tester,
  ) async {
    final layout = await _pumpScene(
      tester,
      _viewports.firstWhere((v) => v.name == 'book'),
      _OverlayMode.record,
    );
    await tester.tap(find.byType(LayerButton));
    await tester.pumpAndSettle();
    final popup = tester.getRect(find.byType(LayerPopupContent));
    expect(layout.regions.any((region) => _contains(region, popup)), isTrue);
    expect(
      popup.overlaps(
        _viewports.firstWhere((v) => v.name == 'book').regions.single.frame,
      ),
      isFalse,
    );
    final tabletopLayout = await _pumpScene(
      tester,
      _viewports.firstWhere((v) => v.name == 'tabletop'),
      _OverlayMode.record,
    );
    final movedPopup = tester.getRect(find.byType(LayerPopupContent));
    expect(
      tabletopLayout.regions.any((region) => _contains(region, movedPopup)),
      isTrue,
    );
    expect(
      movedPopup.overlaps(
        _viewports.firstWhere((v) => v.name == 'tabletop').regions.single.frame,
      ),
      isFalse,
    );
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('journey date and time modals follow book and tabletop regions', (
    tester,
  ) async {
    final book = _viewports.firstWhere((v) => v.name == 'book');
    final tabletop = _viewports.firstWhere((v) => v.name == 'tabletop');
    const locale = Locale('zh', 'CN');
    await _pumpScene(
      tester,
      book,
      _OverlayMode.journeys,
      locale: locale,
      overlayBuilder: _editingCard,
    );
    await tester.enterText(find.byType(TextField), 'Duo draft');
    FocusManager.instance.primaryFocus?.unfocus();
    await tester.pumpAndSettle();
    await tester.tap(find.text(tr('journey.start_time')));
    await tester.pumpAndSettle();
    final dateSurface = find.descendant(
      of: find.byWidgetPredicate((widget) => widget is WoltModalSheet),
      matching: find.byType(AppDialogSurface),
    );
    _expectModalFits(tester, book, dateSurface);
    await _pumpScene(
      tester,
      tabletop,
      _OverlayMode.journeys,
      locale: locale,
      overlayBuilder: _editingCard,
    );
    _expectModalFits(tester, tabletop, dateSurface);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'Duo draft',
    );
    final ok = MaterialLocalizations.of(
      tester.element(
        find.byWidgetPredicate((widget) => widget is WoltModalSheet),
      ),
    ).okButtonLabel;
    await tester.tap(find.text(ok));
    await tester.pumpAndSettle();
    final timeSurface = find.descendant(
      of: find.byWidgetPredicate((widget) => widget is WoltModalSheet),
      matching: find.byType(JourneyInfoPanelSurface),
    );
    _expectModalFits(tester, tabletop, timeSurface);
    await _pumpScene(
      tester,
      book,
      _OverlayMode.journeys,
      locale: locale,
      overlayBuilder: _editingCard,
    );
    _expectModalFits(tester, book, timeSurface);
    expect(
      tester.widget<TextField>(find.byType(TextField)).controller!.text,
      'Duo draft',
    );
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));

  testWidgets('editor actions remain reachable when every region is short', (
    tester,
  ) async {
    const viewport = _Viewport(
      'all-regions-short',
      Size(600, 600),
      DuoBarEdge.right,
      status: DuoHingeStatus.partiallyOpen,
      keyboard: 300,
      folds: [DuoRegion(Rect.fromLTWH(0, 142, 600, 16), isActive: true)],
    );
    var saves = 0;
    DrawEntryMode? selectedDrawMode;
    final layout = await _pumpScene(
      tester,
      viewport,
      _OverlayMode.journeys,
      navigationVisible: false,
      locale: const Locale('zh', 'CN'),
      overlayBuilder: (layout) => _editorModeBar(
        layout,
        true,
        onSave: () => saves++,
        onDrawModeChanged: (mode) => selectedDrawMode = mode,
      ),
    );
    final naturalHeight =
        ModeSwitchBar.preferredHeight(isDrawMenuOpen: true) +
        ModeSwitchBar.safeAreaMinimum * 2;
    expect(layout.regions.every((r) => r.height < naturalHeight), isTrue);
    final selectedRegion = _editorRegion(layout, true);
    final bar = find.byType(ModeSwitchBar);
    _expectInRect(tester.getRect(bar), selectedRegion, 'short editor viewport');
    final save = find.text(tr('journey.editor.save')).hitTestable();
    expect(save, findsOneWidget);
    await tester.tap(save);
    await tester.pumpAndSettle();
    expect(saves, 1);
    final verticalScroll = find.descendant(
      of: bar,
      matching: find.byWidgetPredicate(
        (widget) =>
            widget is Scrollable && widget.axisDirection == AxisDirection.up,
      ),
    );
    expect(verticalScroll, findsOneWidget);
    await tester.drag(verticalScroll, const Offset(0, 100));
    await tester.pumpAndSettle();
    final freehand = find.text(tr('journey.editor.free_draw')).hitTestable();
    expect(freehand, findsOneWidget);
    await tester.tap(freehand);
    await tester.pumpAndSettle();
    expect(selectedDrawMode, DrawEntryMode.freehand);
    _expectInRect(
      tester.getRect(bar),
      selectedRegion,
      'scrolled editor viewport',
    );
    expect(tester.takeException(), isNull);
  }, variant: const TargetPlatformVariant({TargetPlatform.iOS}));
}

// Mirrors JourneyTrackEditPage's parent placement around the real mode bar.
// It verifies the resulting widget bounds rather than its internal arithmetic.
Rect _editorRegion(AppLayout layout, bool menuOpen) => layout.controlRegionFor(
  minimumHeight:
      ModeSwitchBar.preferredHeight(isDrawMenuOpen: menuOpen) +
      ModeSwitchBar.safeAreaMinimum * 2,
);

Widget _editorModeBar(
  AppLayout layout,
  bool menuOpen, {
  VoidCallback? onSave,
  ValueChanged<DrawEntryMode>? onDrawModeChanged,
}) {
  return MapOverlayPanel.toolbar(
    minimumHeight:
        ModeSwitchBar.preferredHeight(isDrawMenuOpen: menuOpen) +
        ModeSwitchBar.safeAreaMinimum * 2,
    minimumPadding: ModeSwitchBar.safeAreaMinimum,
    child: ModeSwitchBar(
      currentMode: OperationMode.edit,
      onModeChanged: (_) {},
      currentDrawMode: DrawEntryMode.linked,
      isDrawMenuOpen: menuOpen,
      onDrawPressed: () {},
      onDrawModeChanged: onDrawModeChanged ?? (_) {},
      onUndo: () {},
      onSave: onSave ?? () {},
    ),
  );
}

Widget _editingCard(
  AppLayout layout, {
  bool isEditing = true,
  VoidCallback? onEdit,
}) {
  final date = DateTime.now();
  return Positioned.fromRect(
    rect: layout.panelBounds,
    child: JourneyInfoPanelSurface(
      child: SingleChildScrollView(
        child: JourneyDetailCard(
          journey: JourneyHeader(
            id: 'modal-fixture',
            revision: 'modal-fixture',
            journeyDate: date,
            createdAt: date,
            journeyType: JourneyType.vector,
            journeyKind: JourneyKind.defaultKind,
            hasRawData: false,
          ),
          isEditing: isEditing,
          onExport: () {},
          onEdit: onEdit ?? () {},
          onMore: () {},
          onCancel: () {},
          onSave: (_) async {},
        ),
      ),
    ),
  );
}

void _expectModalFits(WidgetTester tester, _Viewport viewport, Finder surface) {
  final bounds = tester.getRect(surface);
  _expectInRect(
    bounds,
    Rect.fromLTRB(
      viewport.padding.left,
      viewport.padding.top,
      viewport.size.width - viewport.padding.right,
      viewport.size.height - viewport.padding.bottom,
    ),
    'modal safe area',
  );
  for (final obstacle in viewport.regions) {
    expect(
      bounds.overlaps(obstacle.frame),
      isFalse,
      reason: 'Open modal crosses native region ${obstacle.frame}: $bounds',
    );
  }
}

bool _contains(Rect outer, Rect inner) =>
    inner.left >= outer.left - 0.1 &&
    inner.right <= outer.right + 0.1 &&
    inner.top >= outer.top - 0.1 &&
    inner.bottom <= outer.bottom + 0.1;

void _expectInRect(Rect inner, Rect outer, String reason) => expect(
  _contains(outer, inner),
  isTrue,
  reason: '$reason: $inner outside $outer',
);

enum _OverlayMode { record, journeys }

class _Viewport {
  const _Viewport(
    this.name,
    this.size,
    this.edge, {
    this.padding = const EdgeInsets.fromLTRB(12, 24, 12, 20),
    this.folds = const [],
    this.keyboard = 0,
    this.textScale = 1,
    this.status = DuoHingeStatus.fullyOpen,
  });

  final String name;
  final Size size;
  final DuoBarEdge edge;
  final EdgeInsets padding;
  final List<DuoRegion> folds;
  List<DuoRegion> get regions => folds;
  final double keyboard;
  final double textScale;
  final DuoHingeStatus status;
}

// Synthetic clear-region fixtures; hardware decoding has its own unit tests.
const _viewports = [
  _Viewport('inner-portrait', Size(793, 852), DuoBarEdge.horizontal),
  _Viewport(
    'book',
    Size(852, 793),
    DuoBarEdge.right,
    status: DuoHingeStatus.partiallyOpen,
    folds: [DuoRegion(Rect.fromLTWH(416, 0, 20, 793), isActive: true)],
  ),
  _Viewport(
    'tabletop',
    Size(852, 793),
    DuoBarEdge.right,
    status: DuoHingeStatus.partiallyOpen,
    folds: [DuoRegion(Rect.fromLTWH(0, 382, 852, 28), isActive: true)],
  ),
];

DuoHardware _foldFor(_Viewport viewport) => DuoHardware(
  supported: true,
  hasHinge: true,
  status: viewport.status,
  angle: null,
  folds: viewport.folds,
);

Future<AppLayout> _pumpScene(
  WidgetTester tester,
  _Viewport viewport,
  _OverlayMode mode, {
  ValueChanged<int>? onNavigationSelected,
  Widget Function(AppLayout)? overlayBuilder,
  Locale locale = const Locale('en', 'US'),
  bool navigationVisible = true,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = viewport.size;
  addTearDown(tester.view.resetDevicePixelRatio);
  addTearDown(tester.view.resetPhysicalSize);
  final media = MediaQueryData(
    size: viewport.size,
    viewPadding: viewport.padding,
    padding: viewport.padding.copyWith(
      bottom: viewport.keyboard > 0 ? 0 : viewport.padding.bottom,
    ),
    viewInsets: EdgeInsets.only(bottom: viewport.keyboard),
    textScaler: TextScaler.linear(viewport.textScale),
  );
  late AppLayout lastLayout;
  await tester.runAsync(() async {
    await const AppTranslationLoader().load('assets/translations', locale);
    await tester.pumpWidget(
      EasyLocalization(
        key: ValueKey(locale),
        supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
        startLocale: locale,
        path: 'assets/translations',
        assetLoader: const AppTranslationLoader(),
        fallbackLocale: const Locale('en', 'US'),
        child: Builder(
          builder: (context) => MaterialApp(
            theme: AppTheme.light,
            localizationsDelegates: context.localizationDelegates,
            supportedLocales: context.supportedLocales,
            locale: context.locale,
            builder: (context, child) => MediaQuery(
              data: media,
              child: DuoHardwareScope(
                hardware: _foldFor(viewport),
                child: DuoBarScope(
                  edge: viewport.edge,
                  child: RepaintBoundary(child: child!),
                ),
              ),
            ),
            home: RepaintBoundary(
              child: Scaffold(
                body: AppLayoutBuilder(
                  navigationVisible: navigationVisible,
                  builder: (context, layout) {
                    lastLayout = layout;
                    return ChangeNotifierProvider<GpsManager>(
                      create: (_) => _NoLocation(),
                      child: Stack(
                        children: [
                          const Positioned.fill(
                            child: ColoredBox(color: Color(0xFFE5EBDD)),
                          ),
                          overlayBuilder?.call(layout) ??
                              switch (mode) {
                                _OverlayMode.record => NormalMapOverlay(
                                  trackingMode: TrackingMode.off,
                                  onTrackingPressed: () {},
                                ),
                                _OverlayMode.journeys => JourneyOverlay(
                                  onJourneySelected: (_) async {},
                                  isLoading: false,
                                  refreshRevision: 0,
                                ),
                              },
                          if (navigationVisible)
                            Positioned.fromRect(
                              rect: layout.navigationBounds,
                              child: BottomNavBar(
                                axis: layout.navigationAxis,
                                allowOverflow: true,
                                selectedIndex: mode == _OverlayMode.record
                                    ? 0
                                    : 2,
                                onIndexChanged: onNavigationSelected ?? (_) {},
                                hasUpdateNotification: () => false,
                              ),
                            ),
                        ],
                      ),
                    );
                  },
                ),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(seconds: 1));
  });
  await tester.pumpAndSettle();
  expect(
    lastLayout.size.height,
    closeTo(viewport.size.height - viewport.keyboard, 0.1),
  );
  return lastLayout;
}

class _NoLocation extends ChangeNotifier implements GpsManager {
  GpsRecordingStatus _status = GpsRecordingStatus.none;
  void setStatus(GpsRecordingStatus status) {
    _status = status;
    notifyListeners();
  }

  @override
  GpsRecordingStatus get recordingStatus => _status;
  @override
  Null get latestPosition => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _FixtureApi extends Fake implements RustLibApi {
  final fixtureDate = DateTime.now();
  int journeyCount = 1;
  @override
  api.LayerFilter crateApiApiGetCurrentMainMapLayerFilter() => api.LayerFilter(
    currentJourney: true,
    defaultKind: true,
    flightKind: true,
  );
  @override
  Future<DateTime?> crateApiApiEarliestJourneyDate() async => fixtureDate;
  @override
  Future<List<DateTime>> crateApiApiJourneyDates({
    Set<JourneyKind>? journeyKinds,
  }) async => [fixtureDate];
  @override
  Future<List<JourneyHeader>> crateApiApiListJourneysOnDate({
    required int year,
    required int month,
    required int day,
    Set<JourneyKind>? journeyKinds,
  }) async => [
    for (var index = 0; index < journeyCount; index++)
      JourneyHeader(
        id: index == 0 ? 'fixture' : 'fixture-$index',
        revision: 'fixture',
        journeyDate: fixtureDate,
        createdAt: fixtureDate,
        journeyType: JourneyType.vector,
        journeyKind: JourneyKind.defaultKind,
        hasRawData: false,
      ),
  ];
}

class _NoWakelock extends WakelockPlusMacOSPlugin {
  @override
  Future<void> toggle({required bool enable}) async {}
  @override
  Future<bool> get enabled async => false;
}
