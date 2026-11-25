import 'dart:async';
import 'dart:convert';

import 'package:material_ui/material_ui.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/common/component/shared_map_host.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/map_fog_style.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/map_style_store.dart';
import 'package:provider/provider.dart';
import 'package:memolanes/common/service/location/location_service.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

const _a =
    '{"type":"Feature","id":1,"properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[0,1],[0,0]]]}}';
const _b =
    '{"type":"Feature","id":2,"properties":{},"geometry":{"type":"Polygon","coordinates":[[[2,2],[3,2],[2,3],[2,2]]]}}';
const _boundsA = MapBounds(west: 0, south: 0, east: 1, north: 1);
const _boundsB = MapBounds(west: 2, south: 2, east: 3, north: 3);

class _Renderer extends Fake implements api.MapRendererProxy {}

class _Gps extends ChangeNotifier implements GpsManager {
  int reads = 0;
  LocationData? position;
  @override
  LocationData? get latestPosition {
    reads++;
    return position;
  }

  @override
  LocationData? get lastKnownPosition => null;
  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Platform extends InAppWebViewPlatform {
  final controller = _Controller();
  int platformViews = 0;

  @override
  PlatformInAppWebViewWidget createPlatformInAppWebViewWidget(
    PlatformInAppWebViewWidgetCreationParams params,
  ) {
    return _WebView(params, () => platformViews++);
  }

  @override
  PlatformWebMessageListener createPlatformWebMessageListener(
    PlatformWebMessageListenerCreationParams params,
  ) => _Listener(params);
}

class _WebView extends PlatformInAppWebViewWidget {
  _WebView(super.params, this.onCreate) : super.implementation();
  final VoidCallback onCreate;

  @override
  Widget build(BuildContext context) => _NativeView(onCreate: onCreate);

  @override
  T controllerFromPlatform<T>(PlatformInAppWebViewController controller) =>
      InAppWebViewController.fromPlatform(platform: controller) as T;

  @override
  void dispose() {}
}

class _NativeView extends StatefulWidget {
  const _NativeView({required this.onCreate});
  final VoidCallback onCreate;
  @override
  State<_NativeView> createState() => _NativeViewState();
}

class _NativeViewState extends State<_NativeView> {
  @override
  void initState() {
    super.initState();
    widget.onCreate();
  }

  @override
  Widget build(BuildContext context) => const SizedBox.expand();
}

class _Listener extends PlatformWebMessageListener {
  _Listener(super.params) : super.implementation();
}

class _Reply extends Fake implements PlatformJavaScriptReplyProxy {}

class _Controller extends PlatformInAppWebViewController {
  _Controller()
    : super.implementation(
        const PlatformInAppWebViewControllerCreationParams(id: 0),
      );

  final listeners = <String, PlatformWebMessageListener>{};
  final commands = <Map<String, dynamic>>[];
  final scripts = <String>[];
  Future<bool>? nextAcknowledgment;
  bool failNextRefresh = false;

  @override
  Future<void> addWebMessageListener(
    PlatformWebMessageListener listener,
  ) async {
    listeners[listener.jsObjectName] = listener;
  }

  void push(String channel, Object data) => listeners[channel]!.onPostMessage!(
    WebMessage(data: jsonEncode(data)),
    null,
    true,
    _Reply(),
  );

  void ready() {
    listeners['readyForDisplay']!.onPostMessage!(null, null, true, _Reply());
  }

  int documentLoads = 0;
  @override
  Future<void> loadFile({required String assetFilePath}) async {
    documentLoads++;
    ready();
  }

  @override
  Future<dynamic> evaluateJavascript({
    required String source,
    ContentWorld? contentWorld,
  }) async {
    scripts.add(source);
    if (failNextRefresh && source.contains('refreshMapData()')) {
      failNextRefresh = false;
      throw StateError('refresh evaluation failed');
    }
    const prefix = 'window.applyAchievementRegionCommand(';
    if (source.contains('window.getMapAttribution')) return 'Provider credits';
    if (!source.startsWith(prefix)) return true;
    commands.add(
      jsonDecode(source.substring(prefix.length, source.length - 1)),
    );
    final command = commands.last;
    final acknowledgment = nextAcknowledgment;
    nextAcknowledgment = null;
    final accepted = await (acknowledgment ?? Future.value(true));
    return accepted ? command : false;
  }
}

class _Fixture {
  _Fixture(this.tester) {
    InAppWebViewPlatform.instance = platform;
    addTearDown(gps.dispose);
  }

  final WidgetTester tester;
  final platform = _Platform();
  final gps = _Gps();
  final defaultRenderer = _Renderer();
  final values = <String, String?>{};
  late final styles = MapStyleStore(
    readString: (key) => values[key],
    writeString: (key, value) {
      values[key] = value;
      return true;
    },
  );
  _Controller get controller => platform.controller;
  InAppWebView get webView => tester.widget(find.byType(InAppWebView));
  InAppWebViewController get webController =>
      InAppWebViewController.fromPlatform(platform: controller);

  Completer<bool> holdNextAcknowledgment() {
    final acknowledgment = Completer<bool>();
    controller.nextAcknowledgment = acknowledgment.future;
    return acknowledgment;
  }

  Future<void> show(
    int? revision,
    String? feature,
    MapBounds? bounds, {
    int owner = 0,
    api.MapRendererProxy? renderer,
    ValueChanged<MapView>? onView,
    MapView? view,
    VoidCallback? onMoved,
    TrackingMode tracking = TrackingMode.off,
    EdgeInsets? padding,
  }) async {
    await tester.pumpWidget(
      ChangeNotifierProvider<GpsManager>.value(
        value: gps,
        child: MaterialApp(
          home: BaseMapWebview(
            styleStore: styles,
            mapRendererProxy: renderer ?? defaultRenderer,
            onRoughMapViewUpdate: onView,
            flyToView: view,
            onMapMoved: onMoved,
            trackingMode: tracking,
            cameraRevision: revision,
            cameraOwner: owner,
            achievementRegionGeoJson: feature,
            flyToBounds: bounds,
            boundsPadding: padding,
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> connect() async {
    webView.platform.params.onWebViewCreated!(webController);
    await tester.pumpAndSettle();
    controller.push('readyForCommands', 0);
    await tester.pumpAndSettle();
  }

  Future<void> reload() async {
    webView.platform.params.onLoadStart!(webController, WebUri('about:blank'));
    controller.ready();
    await tester.pumpAndSettle();
  }
}

void main() {
  testWidgets(
    'Settings Record Explorer uses changed basemap and fog in the same document',
    (tester) async {
      final fixture = _Fixture(tester);
      final renderer = _Renderer();
      await fixture.show(null, null, null, renderer: renderer);
      await fixture.connect();
      fixture.controller.scripts.clear();
      // The pickers write to the same store while a settings route covers Record.
      final navigator = tester.state<NavigatorState>(find.byType(Navigator));
      unawaited(
        navigator.push<void>(
          MaterialPageRoute(builder: (_) => const Text('Settings')),
        ),
      );
      await tester.pumpAndSettle();
      fixture.styles.select(MapStyle.all.last.ref);
      fixture.styles.selectFogStyle(MapFogStyle.light);
      await tester.pumpAndSettle();
      navigator.pop();
      await tester.pumpAndSettle();
      final updates = fixture.controller.scripts.where(
        (s) => s.startsWith('window.applyMapAppearance'),
      );
      expect(updates, hasLength(1));
      expect(updates.single, contains(MapStyle.all.last.url));
      expect(updates.single, contains('"fog_style":"light"'));
      expect(fixture.controller.commands, isEmpty);
      expect(
        fixture.controller.scripts.where((s) => s.contains('flyTo')),
        isEmpty,
      );
      fixture.styles.select(MapStyle.all.last.ref);
      fixture.styles.selectFogStyle(MapFogStyle.light);
      await fixture.show(1, _a, _boundsA, owner: 1, renderer: renderer);
      expect(
        updates,
        hasLength(1),
        reason: 'same settings and Explorer entry do not reapply',
      );
      expect(fixture.controller.commands, hasLength(1));
      expect(fixture.controller.documentLoads, 1);
      expect(fixture.platform.platformViews, 1);
    },
  );

  testWidgets(
    'already-ready return retries a rejected or lost acknowledgment without another widget update',
    (tester) async {
      final fixture = _Fixture(tester);
      const home = (lng: 10.0, lat: 20.0, zoom: 4.0);
      final saved = <MapView>[];
      await fixture.show(1, _a, _boundsA, owner: 1);
      await fixture.connect();
      final rejected = fixture.holdNextAcknowledgment();
      await fixture.show(
        null,
        null,
        null,
        owner: 2,
        view: home,
        onView: saved.add,
      );
      final lost = fixture.holdNextAcknowledgment();
      rejected.complete(false);
      await tester.pumpAndSettle();
      expect(fixture.controller.commands, hasLength(3));
      final attempts = fixture.controller.commands.skip(1).toList();
      expect(
        attempts.last['generation'],
        greaterThan(attempts.first['generation']),
      );
      for (final command in attempts) {
        expect(command['view'], {'lng': 10, 'lat': 20, 'zoom': 4});
        expect(command['document'], 0);
        expect(command['owner'], 2);
      }
      expect(saved, isEmpty);
      // A fresh Explorer owner can retire an outstanding retry immediately.
      await fixture.show(1, _a, _boundsA, owner: 3);
      final next = fixture.holdNextAcknowledgment();
      await fixture.show(
        null,
        null,
        null,
        owner: 4,
        view: home,
        onView: saved.add,
        tracking: TrackingMode.displayAndTracking,
      );
      lost.complete(false);
      await tester.pumpAndSettle();
      expect(fixture.controller.commands, hasLength(5));
      expect(saved, isEmpty);
      expect(fixture.gps.reads, 0);
      await tester.pump(const Duration(seconds: 3));
      await tester.pumpAndSettle();
      expect(
        fixture.controller.commands,
        hasLength(6),
        reason: 'missing reply also retries',
      );
      expect(saved, [home]);
      expect(fixture.gps.reads, greaterThan(0));
      next.complete(true);
      await tester.pumpAndSettle();
      expect(saved, [
        home,
      ], reason: 'late acknowledgment cannot complete twice');
      expect(fixture.controller.documentLoads, 1);
    },
  );

  testWidgets(
    'repeated return failures are bounded and keep home and GPS protected',
    (tester) async {
      final fixture = _Fixture(tester);
      const home = (lng: 10.0, lat: 20.0, zoom: 4.0);
      final saved = <MapView>[];
      var moved = 0;
      await fixture.show(1, _a, _boundsA, owner: 1);
      await fixture.connect();
      final first = fixture.holdNextAcknowledgment();
      await fixture.show(
        null,
        null,
        null,
        owner: 2,
        view: home,
        onView: saved.add,
        onMoved: () => moved++,
        tracking: TrackingMode.displayAndTracking,
      );
      final second = fixture.holdNextAcknowledgment();
      first.complete(false);
      await tester.pumpAndSettle();
      expect(fixture.controller.commands, hasLength(3));
      second.complete(false);
      await tester.pumpAndSettle();
      await fixture.show(
        null,
        null,
        _boundsA,
        owner: 2,
        view: home,
        onView: saved.add,
        onMoved: () => moved++,
        tracking: TrackingMode.displayAndTracking,
      );
      fixture.controller.push('onMapViewChanged', {
        'document': 0,
        'owner': 2,
        'lng': 3,
        'lat': 4,
        'zoom': 5,
      });
      fixture.controller.push('onMapMoved', {'document': 0, 'owner': 2});
      fixture.gps.notifyListeners();
      await tester.pump(const Duration(seconds: 5));
      expect(saved, isEmpty);
      expect(moved, 0);
      expect(fixture.gps.reads, 0);
      expect(fixture.controller.commands, hasLength(3));
      expect(
        fixture.controller.scripts.where((s) => s.contains('flyToBounds(')),
        isEmpty,
      );
      expect(
        await tester
            .state<BaseMapWebviewState>(find.byType(BaseMapWebview))
            .getCurrentMapView(),
        home,
      );
      expect(fixture.controller.documentLoads, 1);
    },
  );

  testWidgets(
    'shared host retains one platform view and document and gates hidden callbacks',
    (tester) async {
      final fixture = _Fixture(tester);
      final shared = SharedMapController();
      addTearDown(shared.dispose);
      final page = ValueNotifier(0);
      addTearDown(page.dispose);
      var recordTaps = 0;
      await tester.pumpWidget(
        ChangeNotifierProvider<GpsManager>.value(
          value: fixture.gps,
          child: MaterialApp(
            home: ValueListenableBuilder<int>(
              valueListenable: page,
              builder: (context, selected, _) {
                shared.recordActive = selected == 0;
                shared.explorerActive = selected == 2;
                shared.publishRecord(
                  BaseMapWebview(
                    mapRendererProxy: fixture.defaultRenderer,
                    styleStore: fixture.styles,
                    onMapTap: () => recordTaps++,
                  ),
                );
                return AppNavigationShell(
                  retainedMap: SharedMapHost(controller: shared),
                  mapVisible: selected != 1,
                  navigationBuilder: (_, {required axis}) => const SizedBox(),
                  bodyBuilder: ({
                    required topSafeArea,
                    required bottomSafeArea,
                  }) => const SizedBox.expand(),
                );
              },
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await fixture.connect();
      final originalState = shared.mapKey.currentState;
      fixture.controller.push('onMapTap', {'document': 0, 'lease': 0});
      expect(recordTaps, 1);
      for (final selected in [1, 2, 1, 0]) {
        page.value = selected;
        await tester.pumpAndSettle();
        final map = tester.widget<BaseMapWebview>(find.byType(BaseMapWebview));
        expect(map.onMapTap, selected == 0 ? isNotNull : isNull);
        fixture.controller.push('onMapTap', {'document': 0, 'lease': 0});
        fixture.controller.push('onMapTap', {'document': 0, 'lease': 2});
        expect(recordTaps, 1, reason: 'retired Record and Explorer leases');
        expect(shared.mapKey.currentState, same(originalState));
        expect(fixture.platform.platformViews, 1);
        expect(fixture.controller.documentLoads, 1);
        expect(
          tester.getSize(find.byType(InAppWebView)).shortestSide,
          greaterThan(0),
        );
      }
      fixture.controller.push('onMapTap', {'document': -1, 'lease': 4});
      expect(recordTaps, 1, reason: 'retired document');
      fixture.controller.push('onMapTap', {'document': 0, 'lease': 4});
      expect(recordTaps, 2, reason: 'current Record lease');
      shared.returnView = (lng: 1, lat: 2, zoom: 3);
      tester.widget<BaseMapWebview>(find.byType(BaseMapWebview)).onMapMoved!();
      expect(
        shared.returnView,
        isNull,
        reason: 'a later home gesture must retire the return target',
      );
    },
  );

  testWidgets(
    'hidden attribution is lifecycle state retained on Explorer activation and reset on reload',
    (tester) async {
      final fixture = _Fixture(tester);
      final shared = SharedMapController();
      addTearDown(shared.dispose);
      var available = false;
      var taps = 0;
      shared.recordActive = false;
      shared.publishRecord(
        BaseMapWebview(
          mapRendererProxy: fixture.defaultRenderer,
          styleStore: fixture.styles,
          onMapAttributionChanged: (value) => available = value,
          onMapTap: () => taps++,
        ),
      );
      await tester.pumpWidget(
        ChangeNotifierProvider<GpsManager>.value(
          value: fixture.gps,
          child: MaterialApp(home: SharedMapHost(controller: shared)),
        ),
      );
      await tester.pumpAndSettle();
      await fixture.connect();
      fixture.controller.listeners['onMapTap']!.onPostMessage!(
        null,
        null,
        true,
        _Reply(),
      );
      expect(taps, 0, reason: 'hidden interactions remain gated');
      fixture.webView.platform.params.onLoadStart!(
        fixture.webController,
        WebUri('about:blank'),
      );
      fixture.controller.listeners['readyForCommands']!.onPostMessage!(
        WebMessage(data: '1'),
        null,
        true,
        _Reply(),
      );
      fixture.controller.listeners['onMapAttributionChanged']!.onPostMessage!(
        WebMessage(data: 'true'),
        null,
        true,
        _Reply(),
      );
      shared.explorerActive = true;
      shared.notifyListeners();
      await tester.pumpAndSettle();
      expect(
        available,
        isTrue,
        reason: 'activation must not need a duplicate JS credit event',
      );
      expect(
        await shared.mapKey.currentState!.getMapAttribution(),
        'Provider credits',
      );
      // A document reset while hidden must discard the previous document's credits.
      shared.explorerActive = false;
      shared.notifyListeners();
      await tester.pumpAndSettle();
      fixture.webView.platform.params.onLoadStart!(
        fixture.webController,
        WebUri('about:blank'),
      );
      shared.explorerActive = true;
      shared.notifyListeners();
      await tester.pumpAndSettle();
      expect(available, isFalse);
    },
  );

  testWidgets(
    'latest Journey fit waits for one pending atomic exit and refreshes once',
    (tester) async {
      final fixture = _Fixture(tester);
      const home = (lng: 10.0, lat: 20.0, zoom: 4.0);
      await fixture.show(1, _a, _boundsA, owner: 1);
      await fixture.connect();
      final exit = fixture.holdNextAcknowledgment();
      await fixture.show(null, null, null, owner: 2, view: home);
      fixture.controller.scripts.clear();
      final renderer = _Renderer();
      await fixture.show(
        null,
        null,
        _boundsA,
        owner: 2,
        view: home,
        renderer: renderer,
        padding: const EdgeInsets.all(24),
      );
      await fixture.show(
        null,
        null,
        _boundsB,
        owner: 2,
        view: home,
        renderer: renderer,
        padding: const EdgeInsets.fromLTRB(10, 20, 30, 40),
      );
      expect(
        fixture.controller.commands,
        hasLength(2),
        reason: 'normal updates must not replace the pending scoped exit',
      );
      expect(
        fixture.controller.scripts.where((s) => s.contains('flyToBounds(')),
        isEmpty,
      );
      exit.complete(true);
      await tester.pumpAndSettle();
      final fits = fixture.controller.scripts
          .where((s) => s.contains('flyToBounds('))
          .toList();
      expect(fits, hasLength(1));
      expect(
        fits.single,
        contains('"west":2.0,"south":2.0,"east":3.0,"north":3.0'),
      );
      expect(
        fits.single,
        contains('"top":20.0,"right":30.0,"bottom":40.0,"left":10.0'),
      );
      expect(
        fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
        hasLength(1),
      );
    },
  );

  testWidgets('detached camera refresh evaluation errors are contained', (
    tester,
  ) async {
    final fixture = _Fixture(tester);
    await fixture.show(null, null, null);
    await fixture.connect();
    fixture.controller.failNextRefresh = true;
    await fixture.show(null, null, _boundsA);
    expect(tester.takeException(), isNull);
    expect(
      fixture.controller.scripts.where((s) => s.contains('flyToBounds(')),
      hasLength(1),
    );
  });

  testWidgets('a new Explorer session refits a repeated revision', (
    tester,
  ) async {
    final fixture = _Fixture(tester);
    await fixture.show(1, _a, _boundsA, owner: 1);
    await fixture.connect();
    await fixture.show(1, _a, _boundsA, owner: 2);
    expect(fixture.controller.commands, hasLength(2));
    expect(fixture.controller.commands.last['owner'], 2);
    expect(fixture.controller.commands.last['bounds'], isNotNull);
  });

  testWidgets(
    'renderer swaps refresh independently from acknowledged explicit camera',
    (tester) async {
      final fixture = _Fixture(tester);
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      fixture.controller.scripts.clear();
      await fixture.show(1, _a, _boundsA, renderer: _Renderer());
      expect(
        fixture.controller.scripts.any((s) => s.contains('refreshMapData()')),
        isTrue,
      );
      expect(fixture.controller.commands, hasLength(1));
    },
  );

  testWidgets(
    'pending scoped return gates current-owner pushes and tracking across reload',
    (tester) async {
      final fixture = _Fixture(tester);
      const home = (lng: 10.0, lat: 20.0, zoom: 4.0);
      final saved = <MapView>[];
      var moved = 0;
      final shared = SharedMapController();
      addTearDown(shared.dispose);
      fixture.gps.position = LocationData(
        latitude: 20,
        longitude: 10,
        accuracy: 1,
        timestampMs: 0,
      );
      shared.returnView = home;
      shared.publishRecord(
        BaseMapWebview(
          mapRendererProxy: fixture.defaultRenderer,
          styleStore: fixture.styles,
          onRoughMapViewUpdate: saved.add,
          onMapMoved: () => moved++,
          trackingMode: TrackingMode.displayAndTracking,
        ),
      );
      await tester.pumpWidget(
        ChangeNotifierProvider<GpsManager>.value(
          value: fixture.gps,
          child: MaterialApp(home: SharedMapHost(controller: shared)),
        ),
      );
      await tester.pumpAndSettle();
      await fixture.connect();
      shared.recordActive = false;
      shared.explorerActive = true;
      final fit = fixture.holdNextAcknowledgment();
      shared.publishExplorer((
        featureGeojson: _a,
        flyToBounds: _boundsA,
        flyToView: null,
        boundsPadding: const EdgeInsets.all(24),
        cameraRevision: 1,
      ));
      await tester.pumpAndSettle();
      fixture.gps.reads = 0;
      fixture.controller.scripts.clear();
      final exit = fixture.holdNextAcknowledgment();
      shared.clearExplorer();
      shared.explorerActive = false;
      shared.recordActive = true;
      shared.notifyListeners();
      await tester.pumpAndSettle();
      final command = fixture.controller.commands.last;
      expect(command['view'], {'lng': 10, 'lat': 20, 'zoom': 4});
      expect(command['restore'], true);
      void push(String channel, int document) =>
          fixture.controller.listeners[channel]!.onPostMessage!(
            WebMessage(
              data: jsonEncode({
                'document': document,
                'owner': 2,
                'lng': 99,
                'lat': 33,
                'zoom': 8,
              }),
            ),
            null,
            true,
            _Reply(),
          );
      push('onMapViewChanged', 0);
      push('onMapMoved', 0);
      expect(saved, isEmpty);
      expect(moved, 0);
      expect(fixture.gps.reads, 0);
      expect(
        await tester
            .state<BaseMapWebviewState>(find.byType(BaseMapWebview))
            .getCurrentMapView(),
        home,
      );
      expect(
        await shared.readHomeView(),
        home,
        reason: 'quick reopen keeps the home snapshot',
      );
      expect(shared.returnPending, isTrue);
      // Repeated reloads reuse generation 1: document identity must guard acknowledgments.
      fixture.webView.platform.params.onLoadStart!(
        fixture.webController,
        WebUri('about:blank'),
      );
      final replacement = fixture.holdNextAcknowledgment();
      fixture.controller.listeners['readyForCommands']!.onPostMessage!(
        WebMessage(data: '1'),
        null,
        true,
        _Reply(),
      );
      await tester.pumpAndSettle();
      expect(fixture.controller.commands.last['document'], 1);
      expect(fixture.controller.commands.last['view'], command['view']);
      expect(
        fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
        hasLength(1),
      );
      fixture.controller.scripts.clear();
      fixture.webView.platform.params.onLoadStart!(
        fixture.webController,
        WebUri('about:blank'),
      );
      final newest = fixture.holdNextAcknowledgment();
      fixture.controller.listeners['readyForCommands']!.onPostMessage!(
        WebMessage(data: '2'),
        null,
        true,
        _Reply(),
      );
      await tester.pumpAndSettle();
      expect(fixture.controller.commands.last['document'], 2);
      expect(fixture.controller.commands.last['generation'], 1);
      exit.complete(true);
      fit.complete(true);
      replacement.complete(true);
      await tester.pumpAndSettle();
      push('onMapViewChanged', 2);
      push('onMapMoved', 2);
      expect(saved, isEmpty);
      expect(moved, 0);
      fixture.gps.notifyListeners();
      expect(fixture.gps.reads, 0);
      expect(
        fixture.controller.scripts.where(
          (s) =>
              s.contains('updateLocationMarker(') &&
              RegExp(r'true,\s+true').hasMatch(s),
        ),
        isEmpty,
      );
      newest.complete(true);
      await tester.pumpAndSettle();
      expect(saved, [home]);
      expect(shared.returnPending, isFalse);
      expect(
        fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
        hasLength(1),
        reason: 'reload return refreshes exactly once',
      );
      expect(fixture.gps.reads, greaterThan(0));
      expect(
        fixture.controller.scripts.where(
          (s) =>
              s.contains('updateLocationMarker(') &&
              RegExp(r'true,\s+true').hasMatch(s),
        ),
        hasLength(1),
        reason: 'resume the captured home follow mode only after valid return',
      );
      expect(
        fixture.controller.scripts.where(
          (s) => s.contains('else if (') && s.contains('flyToView('),
        ),
        isEmpty,
        reason: 'no separate unscoped return after acknowledgment',
      );
    },
  );

  testWidgets(
    'local Explorer can read credits but Record keeps full-display cover',
    (tester) async {
      final fixture = _Fixture(tester);
      await fixture.show(1, _a, _boundsA, owner: 1);
      await fixture.connect();
      fixture.webView.platform.params.onLoadStart!(
        fixture.webController,
        WebUri('about:blank'),
      );
      fixture.controller.listeners['readyForCommands']!.onPostMessage!(
        WebMessage(data: '1'),
        null,
        true,
        _Reply(),
      );
      await tester.pumpAndSettle();
      final state = tester.state<BaseMapWebviewState>(
        find.byType(BaseMapWebview),
      );
      expect(await state.getMapAttribution(), 'Provider credits');
      final pending = fixture.holdNextAcknowledgment();
      await fixture.show(
        null,
        null,
        null,
        owner: 2,
        view: (lng: 10, lat: 20, zoom: 4),
      );
      expect(
        tester.widget<AnimatedOpacity>(find.byType(AnimatedOpacity)).opacity,
        1,
      );
      expect(await state.getMapAttribution(), isNull);
      pending.complete(true);
      await tester.pumpAndSettle();
    },
  );

  testWidgets('normal renderer swap refreshes once', (tester) async {
    final fixture = _Fixture(tester);
    await fixture.show(null, null, null);
    await fixture.connect();
    fixture.controller.scripts.clear();
    await fixture.show(null, null, null, renderer: _Renderer());
    expect(
      fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
      hasLength(1),
    );
  });

  testWidgets(
    'exit renderer swap refreshes once while scoped acknowledgment is pending',
    (tester) async {
      final fixture = _Fixture(tester);
      await fixture.show(1, _a, _boundsA, owner: 1);
      await fixture.connect();
      fixture.controller.scripts.clear();
      final exit = fixture.holdNextAcknowledgment();
      await fixture.show(
        null,
        null,
        null,
        owner: 2,
        renderer: _Renderer(),
        view: (lng: 1, lat: 2, zoom: 3),
      );
      expect(
        fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
        hasLength(1),
      );
      exit.complete(true);
      await tester.pumpAndSettle();
      expect(
        fixture.controller.scripts.where((s) => s.contains('refreshMapData()')),
        hasLength(1),
      );
    },
  );

  testWidgets('delayed Explorer camera pushes never enter the home slot', (
    tester,
  ) async {
    final fixture = _Fixture(tester);
    final saved = <MapView>[];
    await fixture.show(1, _a, _boundsA, owner: 1);
    await fixture.connect();
    await fixture.show(null, null, null, owner: 2, onView: saved.add);
    void push(int owner) =>
        fixture.controller.listeners['onMapViewChanged']!.onPostMessage!(
          WebMessage(
            data: jsonEncode({
              'document': 0,
              'owner': owner,
              'lng': 3,
              'lat': 4,
              'zoom': 5,
            }),
          ),
          null,
          true,
          _Reply(),
        );
    push(1);
    expect(saved, isEmpty);
    push(2);
    expect(saved, [(lng: 3.0, lat: 4.0, zoom: 5.0)]);
  });

  testWidgets(
    'leaving Explorer clears its outline without remounting the view',
    (tester) async {
      final fixture = _Fixture(tester);
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      final state = tester.state(find.byType(BaseMapWebview));
      await fixture.show(null, null, null);
      expect(tester.state(find.byType(BaseMapWebview)), same(state));
      expect(fixture.controller.commands, hasLength(2));
      expect(fixture.controller.commands.last['featureGeoJson'], isNull);
      expect(fixture.controller.commands.last['bounds'], isNull);
    },
  );

  testWidgets(
    'explicit re-tap resends the outline and fit after a failed evaluation',
    (tester) async {
      final fixture = _Fixture(tester);
      final failed = fixture.holdNextAcknowledgment();
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      final commands = fixture.controller.commands;
      expect(commands, hasLength(1));
      expect(commands.single['featureGeoJson'], _a);
      failed.completeError(StateError('failed JavaScript evaluation'));
      await tester.pumpAndSettle();
      expect(commands, hasLength(1));

      await fixture.show(2, _a, _boundsA);
      expect(commands, hasLength(2));
      expect(commands.last['featureGeoJson'], _a);
      expect(commands.last['bounds'], {
        'west': 0,
        'south': 0,
        'east': 1,
        'north': 1,
      });
    },
  );

  testWidgets(
    'pending geometry retires A before its re-tap fit is acknowledged',
    (tester) async {
      final fixture = _Fixture(tester);
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      final commands = fixture.controller.commands;
      expect(commands.single['featureGeoJson'], _a);

      final oldFit = fixture.holdNextAcknowledgment();
      await fixture.show(2, _a, _boundsA);
      expect(commands, hasLength(2));
      expect(commands.last['bounds'], {
        'west': 0,
        'south': 0,
        'east': 1,
        'north': 1,
      });

      final clear = fixture.holdNextAcknowledgment();
      await fixture.show(2, null, null);
      expect(oldFit.isCompleted, isFalse);
      expect(commands, hasLength(3), reason: 'clear must not await the A fit');
      expect(commands.last, containsPair('featureGeoJson', null));
      expect(commands.last, containsPair('bounds', null));
      expect(
        commands.last['generation'],
        greaterThan(commands[1]['generation']),
      );

      await fixture.show(3, _b, _boundsB);
      expect(commands, hasLength(4));
      expect(commands.last['featureGeoJson'], _b);
      expect(commands.last['bounds'], {
        'west': 2,
        'south': 2,
        'east': 3,
        'north': 3,
      });
      expect(
        commands.last['generation'],
        greaterThan(commands[2]['generation']),
      );
      oldFit.complete(true);
      clear.complete(true);
      await tester.pumpAndSettle();
      expect(
        commands,
        hasLength(4),
        reason: 'obsolete acknowledgments must not resend B',
      );

      await fixture.reload();
      expect(commands, hasLength(5));
      expect(commands.last['featureGeoJson'], _b);
      expect(
        commands.last,
        containsPair('bounds', null),
        reason: 'only B’s fit was confirmed last',
      );
    },
  );

  testWidgets(
    'cached B replaces A without waiting or sending an intermediate clear',
    (tester) async {
      final fixture = _Fixture(tester);
      final a = fixture.holdNextAcknowledgment();
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      await fixture.show(2, _b, _boundsB);
      final commands = fixture.controller.commands;
      expect(commands.map((command) => command['featureGeoJson']), [_a, _b]);
      expect(commands.last['bounds'], {
        'west': 2,
        'south': 2,
        'east': 3,
        'north': 3,
      });
      a.complete(true);
      await tester.pumpAndSettle();
      expect(commands, hasLength(2));
    },
  );

  testWidgets(
    'an acknowledgment from a previous document cannot confirm its replacement',
    (tester) async {
      final fixture = _Fixture(tester);
      final original = fixture.holdNextAcknowledgment();
      await fixture.show(1, _a, _boundsA);
      await fixture.connect();
      final commands = fixture.controller.commands;
      final replacement = fixture.holdNextAcknowledgment();
      await fixture.reload();
      expect(commands, hasLength(2));
      expect(commands.last['document'], isNot(commands.first['document']));
      original.complete(true);
      replacement.complete(false);
      await tester.pumpAndSettle();
      expect(commands, hasLength(2));

      await fixture.reload();
      expect(commands, hasLength(3));
      expect(commands.last['featureGeoJson'], _a);
      expect(commands.last['bounds'], {
        'west': 0,
        'south': 0,
        'east': 1,
        'north': 1,
      });
    },
  );
}
