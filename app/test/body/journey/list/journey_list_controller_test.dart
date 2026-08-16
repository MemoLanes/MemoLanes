import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class _FakeJourneyHeader extends Fake implements JourneyHeader {}

void main() {
  test('calendar input does not cancel a pending date refresh', () async {
    final dateRequestStarted = Completer<void>();
    final dateRequest = Completer<List<DateTime>>();
    final controller = JourneyListController(
      earliestJourneyDateLoader: () async => DateTime(2024),
      journeyDatesLoader: (journeyKinds) {
        dateRequestStarted.complete();
        return dateRequest.future;
      },
      journeyHeadersLoader: (date, journeyKinds) async => <JourneyHeader>[],
    );
    addTearDown(controller.dispose);

    final filterChange = controller.setJourneyKinds({JourneyKind.flight});
    await dateRequestStarted.future;
    await controller.selectDate(DateTime(2024, 1, 2));

    final filteredDate = DateTime(2024, 2, 3);
    dateRequest.complete([filteredDate]);
    await filterChange;

    expect(controller.journeyDates, [filteredDate]);
    expect(controller.selectedDate, filteredDate);
  });

  test('refresh clears the overall first date after the last deletion',
      () async {
    final controller = JourneyListController(
      earliestJourneyDateLoader: () async => null,
      journeyDatesLoader: (journeyKinds) async => <DateTime>[],
      journeyHeadersLoader: (date, journeyKinds) async => <JourneyHeader>[],
    );
    addTearDown(controller.dispose);
    controller.firstDate = DateTime(2024);

    await controller.refresh(adjustSelectedDate: true);

    expect(controller.firstDate, isNull);
    expect(controller.journeyDates, isEmpty);
    expect(controller.hasJourneyOnSelectedDate, isFalse);
    expect(controller.isJourneyListLoading, isFalse);
  });

  test('refresh selects the nearest remaining date after a deletion', () async {
    var dates = [DateTime(2024, 1, 10), DateTime(2024, 2, 3)];
    final loadedDates = <DateTime>[];
    final controller = JourneyListController(
      earliestJourneyDateLoader: () async => dates.isEmpty ? null : dates.first,
      journeyDatesLoader: (journeyKinds) async => dates,
      journeyHeadersLoader: (date, journeyKinds) async {
        loadedDates.add(date);
        return <JourneyHeader>[];
      },
    );
    addTearDown(controller.dispose);
    controller.selectedDate = dates.first;

    await controller.refresh(adjustSelectedDate: true);
    dates = [DateTime(2024, 2, 3)];
    await controller.refresh(adjustSelectedDate: true);

    expect(controller.selectedDate, DateTime(2024, 2, 3));
    expect(controller.hasJourneyOnSelectedDate, isTrue);
    expect(loadedDates.last, DateTime(2024, 2, 3));
  });

  test('list loading ends only after the current header request', () async {
    final headerRequestStarted = Completer<void>();
    final headerRequest = Completer<List<JourneyHeader>>();
    final date = DateTime(2024, 1, 2);
    final controller = JourneyListController(
      earliestJourneyDateLoader: () async => date,
      journeyDatesLoader: (journeyKinds) async => [date],
      journeyHeadersLoader: (date, journeyKinds) {
        headerRequestStarted.complete();
        return headerRequest.future;
      },
    );
    addTearDown(controller.dispose);

    final refresh = controller.refresh(adjustSelectedDate: true);
    await headerRequestStarted.future;

    expect(controller.isJourneyListLoading, isTrue);
    expect(controller.journeyHeaders, isEmpty);

    headerRequest.complete(<JourneyHeader>[]);
    await refresh;

    expect(controller.isJourneyListLoading, isFalse);
  });

  test('a stale header request cannot finish the current loading state',
      () async {
    final requests = <DateTime, Completer<List<JourneyHeader>>>{};
    final controller = JourneyListController(
      journeyHeadersLoader: (date, journeyKinds) {
        final request = Completer<List<JourneyHeader>>();
        requests[date] = request;
        return request.future;
      },
    );
    addTearDown(controller.dispose);
    final firstDate = DateTime(2024, 1, 1);
    final secondDate = DateTime(2024, 1, 2);

    final firstLoad = controller.selectDate(firstDate);
    final secondLoad = controller.selectDate(secondDate);

    requests[firstDate]!.complete(<JourneyHeader>[]);
    await firstLoad;

    expect(controller.isJourneyListLoading, isTrue);

    requests[secondDate]!.complete(<JourneyHeader>[]);
    await secondLoad;

    expect(controller.isJourneyListLoading, isFalse);
  });

  test('keeps the current list visible while another date is loading',
      () async {
    final headerRequest = Completer<List<JourneyHeader>>();
    final controller = JourneyListController(
      journeyHeadersLoader: (date, journeyKinds) => headerRequest.future,
    );
    addTearDown(controller.dispose);
    final currentHeaders = <JourneyHeader>[_FakeJourneyHeader()];
    controller.journeyHeaders = currentHeaders;

    final load = controller.selectDate(DateTime(2024, 1, 2));

    expect(controller.isJourneyListLoading, isTrue);
    expect(controller.journeyHeaders, same(currentHeaders));

    headerRequest.complete(<JourneyHeader>[]);
    await load;

    expect(controller.isJourneyListLoading, isFalse);
    expect(controller.journeyHeaders, isEmpty);
  });

  test('an empty month clears the list without loading headers', () async {
    var headerLoadCount = 0;
    final controller = JourneyListController(
      journeyHeadersLoader: (date, journeyKinds) async {
        headerLoadCount += 1;
        return <JourneyHeader>[];
      },
    );
    addTearDown(controller.dispose);
    controller.firstDate = DateTime(2024, 1, 1);
    controller.selectedDate = DateTime(2024, 1, 1);
    controller.journeyDates = [DateTime(2024, 1, 1)];
    controller.journeyHeaders = <JourneyHeader>[_FakeJourneyHeader()];

    await controller.displayMonth(DateTime(2024, 2, 1));

    expect(controller.selectedDate, DateTime(2024, 2, 1));
    expect(controller.hasJourneyOnSelectedDate, isFalse);
    expect(controller.journeyHeaders, isEmpty);
    expect(controller.isJourneyListLoading, isFalse);
    expect(headerLoadCount, 0);
  });
}
