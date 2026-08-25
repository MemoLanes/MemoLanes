import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/simple_date_utils.dart';

void main() {
  test('FRB NaiveDate carrier converts to a domain date by fields', () {
    final carrier = DateTime.utc(2026, 8, 24);

    expect(carrier.isUtc, isTrue);
    expect(carrier.toSimpleDate(), SimpleDate(2026, 8, 24));
  });

  test('domain date converts to UTC midnight for FRB', () {
    final carrier = SimpleDate(2026, 8, 24).toFrbNaiveDate();

    expect(carrier, DateTime.utc(2026, 8, 24));
    expect(carrier.isUtc, isTrue);
  });

  test('domain date converts to local midnight for Flutter widgets', () {
    final widgetDate = SimpleDate(2026, 8, 24).toLocalDateTime();

    expect(widgetDate, DateTime(2026, 8, 24));
    expect(widgetDate.isUtc, isFalse);
  });

  test('calendar day distance is not affected by daylight saving time', () {
    expect(
      calendarDaysBetween(SimpleDate(2026, 3, 9), SimpleDate(2026, 3, 7)),
      2,
    );
    expect(
      calendarDaysBetween(SimpleDate(2026, 3, 7), SimpleDate(2026, 3, 9)),
      -2,
    );
  });
}
