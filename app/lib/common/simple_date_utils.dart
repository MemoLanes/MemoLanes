import 'package:simple_date/simple_date.dart';

export 'package:simple_date/simple_date.dart';

/// Returns the signed number of calendar days from [other] to [date].
///
/// UTC midnights make every calendar day exactly 24 hours. Do not replace this
/// with `SimpleDate.difference()`, which uses local midnights and can be 23 or
/// 25 hours across daylight-saving transitions.
int calendarDaysBetween(SimpleDate date, SimpleDate other) => DateTime.utc(
  date.year,
  date.month,
  date.day,
).difference(DateTime.utc(other.year, other.month, other.day)).inDays;

extension SimpleDateBridgeConversion on SimpleDate {
  /// Converts a domain date to FRB's UTC-midnight representation of
  /// `chrono::NaiveDate`.
  DateTime toFrbNaiveDate() => DateTime.utc(year, month, day);

  /// Converts a domain date to the local-midnight representation expected by
  /// Flutter date widgets.
  DateTime toLocalDateTime() => DateTime(year, month, day);
}
