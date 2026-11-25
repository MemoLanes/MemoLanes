import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/achievement/achievement_region_explorer_controller.dart';
import 'package:memolanes/src/rust/achievement/region.dart';
import 'package:memolanes/src/rust/api/achievement.dart';
import 'package:memolanes/src/rust/frb_generated.dart';

import 'support/achievement_region_fixtures.dart';

const a = GeoEntityId(field0: 1);
const b = GeoEntityId(field0: 2);
const leaf = GeoEntityId(field0: 3);

const row = regionRow;
const geometry = regionGeometryFixture;

Future<void> flush() async {
  await Future<void>.delayed(Duration.zero);
}

class Fixture {
  final root = <GeoEntityId, RegionEntity>{
    a: row('A'),
    b: row('B', children: true),
  };
  final child = <GeoEntityId, RegionEntity>{leaf: row('Leaf')};
  final geometryRequests = <GeoEntityId>[];
  final geometryPending = <GeoEntityId, Completer<RegionGeometry?>>{};
  final rootPending = <Completer<Map<GeoEntityId, RegionEntity>>>[];
  final childPending = <Completer<Map<GeoEntityId, RegionEntity>>>[];
  final activation = <Completer<void>>[];
  late final manager = regionWorldviewManager(
    activate: (_) {
      final pending = Completer<void>();
      activation.add(pending);
      return pending.future;
    },
  );

  AchievementRegionExplorerController controller() =>
      AchievementRegionExplorerController(
        worldviewManager: manager,
        loadRoot: () {
          final pending = Completer<Map<GeoEntityId, RegionEntity>>();
          rootPending.add(pending);
          return pending.future;
        },
        loadChildren: (id) {
          expect(id, b);
          final pending = Completer<Map<GeoEntityId, RegionEntity>>();
          childPending.add(pending);
          return pending.future;
        },
        loadGeometry: (id) {
          geometryRequests.add(id);
          final pending = Completer<RegionGeometry?>();
          geometryPending[id] = pending;
          return pending.future;
        },
      );

  Future<void> initialize() async {
    final init = manager.initialize();
    await flush();
    activation.last.complete();
    await init;
  }
}

class _PendingGeometryApi implements RustLibApi {
  final reads = <GeoEntityId>[];
  final pending = <Completer<RegionGeometry?>>[];

  @override
  Future<RegionGeometry?> crateApiAchievementRegionGeometry({
    required GeoEntityId entityId,
  }) {
    reads.add(entityId);
    final reply = Completer<RegionGeometry?>();
    pending.add(reply);
    return reply.future;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'initial root selects the first displayed Area rank and fits once',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      addTearDown(c.dispose);
      c.openRoot();
      expect(c.selectedId, isNull);
      expect(f.geometryRequests, isEmpty);
      f.rootPending.last.complete({
        leaf: row('Zero', area: 0),
        b: row('B', area: 20),
        const GeoEntityId(field0: 4): row('Small', area: 1),
        a: row('Z name', area: 20, iso: 'AAA'),
      });
      await flush();
      expect(c.selectedId, a);
      expect(f.geometryRequests, [a]);
      expect(c.cameraRevision, 0);
      f.geometryPending[a]!.complete(geometry(a));
      await flush();
      expect(c.outline?.entityId, a);
      expect(c.cameraRevision, 1);
      c.retryList();
      f.rootPending.last.complete({b: row('B', area: 100)});
      await flush();
      expect(c.selectedId, a);
      expect(f.geometryRequests, [a]);
    },
  );

  test(
    'successful root load notifies once, including an empty visited root',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      addTearDown(c.dispose);
      var notifications = 0;
      c.addListener(() => notifications++);

      c.openRoot();
      notifications = 0;
      f.rootPending.last.complete(f.root);
      await flush();
      expect(notifications, 1);
      expect(c.selectedId, a);
      expect(c.isLoading, isFalse);
      expect(c.cameraRevision, 0);

      f.geometryPending[a]!.complete(geometry(a));
      await flush();
      expect(notifications, 2);
      expect(c.cameraRevision, 1);

      c.openRoot();
      notifications = 0;
      f.rootPending.last.complete({leaf: row('Zero', area: 0)});
      await flush();
      expect(notifications, 1);
      expect(c.selectedId, isNull);
      expect(c.isLoading, isFalse);
    },
  );

  test(
    'initial root error retries selection; empty visited root never selects',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      addTearDown(c.dispose);
      c.openRoot();
      f.rootPending.last.completeError(StateError('root failed'));
      await flush();
      expect(c.selectedId, isNull);
      c.retryList();
      f.rootPending.last.complete(f.root);
      await flush();
      expect(c.selectedId, a);
      expect(f.geometryRequests, [a]);
      c.openRoot();
      f.rootPending.last.complete({leaf: row('Zero', area: 0)});
      await flush();
      expect(c.selectedId, isNull);
      expect(f.geometryRequests, [a]);
    },
  );

  test('chevron focuses its parent; Back restores prior selection, rows and offset', () async {
    final f = Fixture();
    await f.initialize();
    final c = f.controller();
    c.openRoot();
    expect(c.outline, isNull);
    expect(c.initialCameraRevision, 1);
    f.rootPending.last.complete(f.root);
    await flush();
    f.geometryPending[a]!.complete(geometry(a));
    await flush();
    final firstFit = c.cameraRevision;
    c.drillInto(b, 42);
    expect(c.selectedId, b);
    expect(c.outline, isNull);
    expect(c.rows, isEmpty);
    f.geometryPending[b]!.complete(geometry(b));
    f.childPending.last.complete(f.child);
    await flush();
    expect(c.rows.keys, [leaf]);
    expect(c.outline?.entityId, b);
    expect(c.cameraRevision, greaterThan(firstFit));
    c.back();
    expect(c.rows.keys, [a, b]);
    expect(c.selectedId, a);
    expect(c.restoredScrollOffset, 42);
    expect(c.outline?.entityId, a);
    expect(c.cameraRevision, greaterThan(firstFit));
    c.dispose();
  });

  test('leaf focuses without a drill action; repeat selection refits cached result', () async {
    final f = Fixture();
    await f.initialize();
    final c = f.controller();
    c.openRoot();
    f.rootPending.last.complete(f.root);
    await flush();
    c.drillInto(b, 0);
    f.geometryPending[b]!.complete(geometry(b));
    f.childPending.last.complete(f.child);
    await flush();
    expect(c.rows[leaf]!.hasChildren, isFalse);
    c.select(leaf);
    f.geometryPending[leaf]!.complete(geometry(leaf));
    await flush();
    final revision = c.cameraRevision;
    c.select(leaf);
    expect(c.cameraRevision, revision + 1);
    expect(f.geometryRequests, [a, b, leaf]);
    expect(c.outline?.entityId, leaf);
    c.dispose();
  });

  test(
    'unvisited stats rows are not navigable or fetched for geometry',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      c.openRoot();
      f.rootPending.last.complete({
        ...f.root,
        leaf: RegionEntity(
          kind: RegionKind.admin0,
          nameKey: const RegionNameKey(value: 'Not visited'),
          hasChildren: true,
          visitedAreaM2: BigInt.zero,
          totalAreaM2: BigInt.from(20),
        ),
      });
      await flush();
      expect(c.rows.keys, [a, b]);
      c.select(leaf);
      expect(c.selectedId, a);
      expect(f.geometryRequests, [a]);
      c.dispose();
    },
  );

  test('out-of-order completions never restore an old outline', () async {
    final f = Fixture();
    await f.initialize();
    final c = f.controller();
    c.openRoot();
    f.rootPending.last.complete(f.root);
    await flush();
    c.select(b);
    f.geometryPending[a]!.complete(geometry(a));
    await flush();
    expect(c.outline, isNull);
    f.geometryPending[b]!.complete(geometry(b));
    await flush();
    expect(c.outline?.entityId, b);
    c.dispose();
  });

  test(
    'missing or failed geometry keeps rows and lets current selection retry',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      c.openRoot();
      f.rootPending.last.complete(f.root);
      await flush();
      f.geometryPending[a]!.complete(null);
      await flush();
      expect(c.outline, isNull);
      expect(c.rows.keys, [a, b]);
      c.select(a);
      f.geometryPending[a]!.completeError(StateError('unreadable'));
      await flush();
      expect(c.geometryError, isNotNull);
      expect(c.rows.keys, [a, b]);
      c.retrySelectedGeometry();
      await flush();
      expect(f.activation, hasLength(2));
      expect(f.geometryRequests, [a, a]);
      f.activation.last.complete();
      await flush();
      expect(f.geometryRequests, [a, a, a]);
      f.geometryPending[a]!.complete(geometry(a));
      await flush();
      expect(c.outline?.entityId, a);
      c.dispose();
    },
  );

  test('a new selection waits for display repair and keeps the rows', () async {
    final f = Fixture();
    await f.initialize();
    final c = f.controller();
    addTearDown(c.dispose);
    c.openRoot();
    f.rootPending.last.complete(f.root);
    await flush();
    f.geometryPending[a]!.completeError(StateError('unreadable'));
    await flush();
    c.retrySelectedGeometry();
    c.select(b);
    await flush();
    expect(c.rows.keys, [a, b]);
    expect(f.geometryRequests, [a]);
    f.activation.last.complete();
    await flush();
    expect(f.geometryRequests, [a, b]);
    f.geometryPending[b]!.complete(geometry(b));
    await flush();
    expect(c.outline?.entityId, b);
    expect(c.geometryError, isNull);
  });

  for (final fails in [false, true]) {
    test('late explorer waits for activation (failure: $fails)', () async {
      final f = Fixture();
      await f.initialize();
      final change = f.manager.update(Worldview.usa);
      await flush();
      final c = f.controller();
      addTearDown(c.dispose);
      c.openRoot();
      expect(f.rootPending, isEmpty);
      if (fails) {
        f.activation.last.completeError(StateError('activation failed'));
        await expectLater(change, throwsStateError);
      } else {
        f.activation.last.complete();
        await change;
      }
      expect(f.rootPending, hasLength(1));
    });
  }

  test(
    'default geometry bridge ignores stale replies across worldview switches',
    () async {
      final api = _PendingGeometryApi();
      RustLib.initMock(api: api);
      addTearDown(RustLib.dispose);
      final fixtures = [Fixture(), Fixture(), Fixture()];
      final controllers = <AchievementRegionExplorerController>[];
      final changes = <Future<void>>[];
      for (final f in fixtures) {
        await f.initialize();
        final c = AchievementRegionExplorerController(
          worldviewManager: f.manager,
          loadRoot: () async => f.root,
        );
        controllers.add(c);
        addTearDown(c.dispose);
        c.openRoot();
        await flush();
        expect(c.selectedId, a);
        expect(c.isGeometryLoading, isTrue);
        changes.add(f.manager.update(Worldview.usa));
        await flush();
      }
      expect(api.reads, [a, a, a]);

      fixtures[0].activation.last.complete();
      await changes[0];
      fixtures[1].activation.last.completeError(
        StateError('activation failed'),
      );
      await expectLater(changes[1], throwsStateError);
      api.pending[0].completeError(StateError('stale geometry request'));
      api.pending[1].complete(geometry(a)); // the failed switch kept ISO
      api.pending[2].complete(geometry(a)); // switch still pending
      await flush();
      for (final c in controllers) {
        expect(c.outline, isNull);
        expect(c.geometryError, isNull);
        expect(c.cameraRevision, 0);
      }
      fixtures[2].activation.last.complete();
      await changes[2];
      await flush();
      expect(api.reads, [a, a, a, a, a, a]);
      for (final c in controllers) {
        expect(c.rows.keys, [a, b]);
        c.select(b);
        api.pending.last.complete(
          geometry(b, c.worldviewManager.currentWorldview),
        );
      }
      await flush();
      expect(api.reads, [a, a, a, a, a, a, b, b, b]);
      for (final c in controllers) {
        expect(c.selectedId, b);
        expect(c.outline?.entityId, b);
        expect(c.outline?.worldview, c.worldviewManager.currentWorldview);
        expect(c.isGeometryLoading, isFalse);
        expect(c.cameraRevision, 1);
      }
    },
  );

  for (final fails in [false, true]) {
    test(
      'activation ${fails ? 'failure' : 'success'} invalidates pending reads and reloads settled root',
      () async {
        final f = Fixture();
        await f.initialize();
        final c = f.controller();
        c.openRoot();
        f.rootPending.last.complete(f.root);
        await flush();
        final switchWorldview = f.manager.update(Worldview.usa);
        await flush();
        expect(c.rows, isEmpty);
        expect(c.outline, isNull);
        expect(c.selectedId, isNull);
        expect(f.rootPending.length, 1);
        f.geometryPending[a]!.complete(geometry(a));
        await flush();
        expect(c.outline, isNull);
        if (fails) {
          f.activation.last.completeError(StateError('activation failed'));
          await expectLater(switchWorldview, throwsStateError);
        } else {
          f.activation.last.complete();
          await switchWorldview;
        }
        await flush();
        expect(f.rootPending.length, 2);
        f.rootPending.last.complete({leaf: f.child[leaf]!});
        await flush();
        expect(c.rows.keys, [leaf]);
        expect(c.selectedId, leaf);
        expect(f.geometryRequests, [a, leaf]);
        expect(
          f.manager.currentWorldview,
          fails ? Worldview.iso : Worldview.usa,
        );
        c.dispose();
      },
    );
  }

  test(
    'old stats completion after activation cannot publish stale rows',
    () async {
      final f = Fixture();
      await f.initialize();
      final c = f.controller();
      c.openRoot();
      final old = f.rootPending.last;
      final change = f.manager.update(Worldview.usa);
      await flush();
      old.complete(f.root);
      await flush();
      expect(c.rows, isEmpty);
      f.activation.last.complete();
      await change;
      await flush();
      f.rootPending.last.complete({leaf: f.child[leaf]!});
      await flush();
      expect(c.rows.keys, [leaf]);
      c.dispose();
    },
  );
}
