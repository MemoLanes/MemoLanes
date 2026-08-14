import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/body/journey/list/journey_list_controller.dart';
import 'package:memolanes/src/rust/journey_header.dart';

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
  });
}
