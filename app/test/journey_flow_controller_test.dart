import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/map/journey_flow_controller.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/src/rust/journey_header.dart';

void main() {
  final backend = _JourneyApi();
  setUpAll(() => RustLib.initMock(api: backend));
  tearDownAll(RustLib.dispose);

  late JourneyFlowController flow;
  setUp(() {
    backend.requests.clear();
    backend.header = null;
    flow = JourneyFlowController();
  });
  tearDown(() => flow.dispose());

  final journey = JourneyHeader(
    id: 'journey',
    revision: 'initial',
    journeyDate: DateTime.utc(2024),
    createdAt: DateTime.utc(2024),
    journeyType: JourneyType.vector,
    journeyKind: JourneyKind.defaultKind,
  );
  const view = (lng: 113.0, lat: 22.0, zoom: 10.0);

  Future<void> startLoad() async {
    // Let the camera read finish so the backend request has started.
    await Future<void>.delayed(Duration.zero);
  }

  test('leaving during the camera read prevents a journey request', () async {
    final camera = Completer<MapView?>();
    final opening = flow.open(journey, readMapView: () => camera.future);
    expect(flow.phase, JourneyPhase.opening);
    expect(flow.appChromeVisible, isTrue);
    expect(flow.leave(), isTrue);
    camera.complete(view);
    await opening;
    expect(backend.requests, isEmpty);
    expect(flow.session, isNull);
    expect(flow.phase, JourneyPhase.picker);
  });

  for (final fail in [false, true]) {
    test(
      'a superseded load ${fail ? 'failure' : 'success'} cannot replace a newer detail',
      () async {
        final old = flow.open(journey, readMapView: () async => view);
        await startLoad();
        final oldRequest = backend.requests.single;
        flow.leave();
        final current = flow.open(journey, readMapView: () async => view);
        await startLoad();
        final currentMap = _MapRenderer();
        backend.requests.last.complete((currentMap, null));
        await current;
        final currentSession = flow.session;
        if (fail) {
          oldRequest.completeError(StateError('Old request failed'));
        } else {
          oldRequest.complete((_MapRenderer(), null));
        }
        await old;
        expect(flow.session, same(currentSession));
        expect(flow.session!.mapData.$1, same(currentMap));
        expect(flow.phase, JourneyPhase.viewing);
        expect(flow.appChromeVisible, isFalse);
      },
    );
  }

  test('an active load failure returns to a usable picker', () async {
    final opening = flow.open(journey, readMapView: () async => view);
    final failure = expectLater(opening, throwsStateError);
    await startLoad();
    backend.requests.single.completeError(StateError('Storage unavailable'));
    await failure;
    expect(flow.phase, JourneyPhase.picker);
    expect(flow.appChromeVisible, isTrue);
    expect(flow.requestBack(), JourneyBackResult.unhandled);
  });

  test(
    'back cancels a load without allowing its late completion to open details',
    () async {
      final opening = flow.open(journey, readMapView: () async => view);
      await startLoad();
      expect(flow.requestBack(), JourneyBackResult.handled);
      backend.requests.single.complete((_MapRenderer(), null));
      await opening;
      expect(flow.session, isNull);
      expect(flow.phase, JourneyPhase.picker);
      expect(flow.pickerRevision, 0);
    },
  );

  test('closing consumes one session and restores its original view', () async {
    final opening = flow.open(journey, readMapView: () async => view);
    await startLoad();
    backend.requests.single.complete((_MapRenderer(), null));
    await opening;
    flow.editInformation();
    expect(flow.requestBack(), JourneyBackResult.handled);
    expect(flow.phase, JourneyPhase.viewing);
    expect(flow.appChromeVisible, isFalse);
    expect(flow.pickerRevision, 0);
    expect(flow.requestBack(), JourneyBackResult.handled);
    expect(flow.returnToView, view);
    expect(flow.pickerRevision, 1);
    expect(flow.appChromeVisible, isTrue);
    expect(flow.requestBack(), JourneyBackResult.unhandled);
    expect(flow.pickerRevision, 1);
    flow.leave();
    expect(flow.returnToView, isNull);
  });

  test('disposing a flow invalidates outstanding loads', () async {
    final disposedFlow = JourneyFlowController();
    final opening = disposedFlow.open(journey, readMapView: () async => view);
    await startLoad();
    disposedFlow.dispose();
    backend.requests.single.complete((_MapRenderer(), null));
    await opening;
    expect(disposedFlow.session, isNull);
  });

  test('track refresh publishes header and map together and retains the return camera', () async {
    final opening = flow.open(journey, readMapView: () async => view);
    await startLoad();
    final oldMap = _MapRenderer();
    backend.requests.single.complete((oldMap, null));
    await opening;
    final updated = JourneyHeader(
      id: journey.id,
      revision: 'updated',
      journeyDate: journey.journeyDate,
      createdAt: journey.createdAt,
      journeyType: journey.journeyType,
      journeyKind: journey.journeyKind,
      note: 'Updated track',
    );
    backend.header = updated;
    final refreshing = flow.refreshTrack();
    await startLoad();
    flow.editInformation();
    await flow.deleteJourney();
    expect(flow.phase, JourneyPhase.refreshing);
    expect(flow.session!.journey, same(journey));
    expect(flow.session!.mapData.$1, same(oldMap));
    final updatedMap = _MapRenderer();
    backend.requests.last.complete((updatedMap, null));
    await refreshing;
    expect(flow.session!.journey, same(updated));
    expect(flow.session!.mapData.$1, same(updatedMap));
    expect(flow.phase, JourneyPhase.viewing);
    flow.requestBack();
    expect(flow.returnToView, view);
  });

  for (final fail in [false, true]) {
    test(
      'a dismissed track refresh cannot finish a newer refresh (${fail ? 'error' : 'success'})',
      () async {
        final opening = flow.open(journey, readMapView: () async => view);
        await startLoad();
        backend.requests.single.complete((_MapRenderer(), null));
        await opening;
        backend.header = journey;
        final oldRefresh = flow.refreshTrack();
        await startLoad();
        final oldRequest = backend.requests.last;
        expect(flow.requestBack(), JourneyBackResult.handled);

        final reopening = flow.open(journey, readMapView: () async => view);
        await startLoad();
        backend.requests.last.complete((_MapRenderer(), null));
        await reopening;
        final newRefresh = flow.refreshTrack();
        await startLoad();
        if (fail) {
          oldRequest.completeError(StateError('Old track load failed'));
        } else {
          oldRequest.complete((_MapRenderer(), null));
        }
        await oldRefresh;
        expect(flow.phase, JourneyPhase.refreshing);
        final newMap = _MapRenderer();
        backend.requests.last.complete((newMap, null));
        await newRefresh;
        expect(flow.phase, JourneyPhase.viewing);
        expect(flow.session!.mapData.$1, same(newMap));
      },
    );
  }
}

class _JourneyApi extends Fake implements RustLibApi {
  final requests = <Completer<(api.MapRendererProxy, MapBounds?)>>[];
  JourneyHeader? header;

  @override
  Future<JourneyHeader?> crateApiApiGetJourneyHeader({
    required String journeyId,
  }) async => header;

  @override
  Future<(api.MapRendererProxy, MapBounds?)>
  crateApiApiGetMapRendererProxyForJourney({required String journeyId}) {
    final request = Completer<(api.MapRendererProxy, MapBounds?)>();
    requests.add(request);
    return request.future;
  }
}

class _MapRenderer extends Fake implements api.MapRendererProxy {}
