import 'package:simple_date/simple_date.dart';

export 'package:simple_date/simple_date.dart';

extension SimpleDateBridgeConversion on SimpleDate {
  /// Converts a domain date to FRB's UTC-midnight representation of
  /// `chrono::NaiveDate`.
  DateTime toFrbNaiveDate() => DateTime.utc(year, month, day);

  /// Converts a domain date to the local-midnight representation expected by
  /// Flutter date widgets.
  DateTime toLocalDateTime() => DateTime(year, month, day);
}
