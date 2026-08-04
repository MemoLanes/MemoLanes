import 'package:flutter/foundation.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyListController extends ChangeNotifier {
  JourneyListController();

  DateTime? firstDate;
  final DateTime lastDate = DateTime.now();
  DateTime selectedDate = DateTime.now();
  Set<JourneyKind> selectedJourneyKinds = {
    JourneyKind.defaultKind,
    JourneyKind.flight,
  };
  List<DateTime> journeyDates = [];
  List<JourneyHeader> journeyHeaders = [];
  bool isInitialLoading = true;
  int _requestVersion = 0;
  bool _isDisposed = false;

  bool get hasFilteredJourneys => journeyDates.isNotEmpty;

  List<int> get yearsWithJourneys =>
      journeyDates.map((date) => date.year).toSet().toList();

  List<int> monthsForYear(int year) => journeyDates
      .where((date) => date.year == year)
      .map((date) => date.month)
      .toSet()
      .toList();

  List<int> daysForMonth(int year, int month) => journeyDates
      .where((date) => date.year == year && date.month == month)
      .map((date) => date.day)
      .toSet()
      .toList();

  Future<void> initialize() async {
    final earliestDate = await api.earliestJourneyDate();
    if (_isDisposed) return;
    firstDate = earliestDate == null ? null : naiveDateToDateTime(earliestDate);
    if (firstDate != null) {
      await refresh(adjustSelectedDate: true);
    }
    if (_isDisposed) return;
    isInitialLoading = false;
    notifyListeners();
  }

  Future<void> refresh({bool adjustSelectedDate = false}) async {
    final requestVersion = ++_requestVersion;
    final journeyKinds = Set<JourneyKind>.from(selectedJourneyKinds);
    journeyHeaders = [];
    notifyListeners();
    final dates = (await api.journeyDates(journeyKinds: journeyKinds))
        .map(naiveDateToDateTime)
        .toList();
    if (_isDisposed || requestVersion != _requestVersion) return;

    journeyDates = dates;
    if (adjustSelectedDate && !_hasJourneyOn(selectedDate)) {
      final nearestDate = _nearestDate(
        dates,
        DateTime(selectedDate.year, selectedDate.month, selectedDate.day),
      );
      if (nearestDate != null) selectedDate = nearestDate;
    }
    if (dates.isEmpty) {
      journeyHeaders = [];
      notifyListeners();
      return;
    }
    notifyListeners();
    await _loadJourneysOnSelectedDate(
      requestVersion: requestVersion,
      journeyKinds: journeyKinds,
    );
  }

  Future<void> selectDate(DateTime date) async {
    selectedDate = DateTime(date.year, date.month, date.day);
    journeyHeaders = [];
    notifyListeners();
    await _loadJourneysOnSelectedDate();
  }

  Future<void> displayMonth(DateTime displayedMonth) async {
    final earliest = firstDate;
    if (earliest == null) return;
    var date =
        DateTime(displayedMonth.year, displayedMonth.month, selectedDate.day);
    final lastDay = DateTime(displayedMonth.year, displayedMonth.month + 1, 0);
    if (selectedDate.day > lastDay.day) date = lastDay;
    if (lastDate.isBefore(date)) date = lastDate;
    if (earliest.isAfter(date)) date = earliest;
    selectedDate =
        _nearestDateInMonth(displayedMonth.year, displayedMonth.month, date) ??
            date;
    journeyHeaders = [];
    notifyListeners();
    await _loadJourneysOnSelectedDate();
  }

  Future<void> setJourneyKinds(Set<JourneyKind> kinds) async {
    if (kinds.isEmpty || setEquals(kinds, selectedJourneyKinds)) return;
    selectedJourneyKinds = Set<JourneyKind>.from(kinds);
    notifyListeners();
    await refresh(adjustSelectedDate: true);
  }

  Future<void> showAllJourneyKinds() => setJourneyKinds({
        JourneyKind.defaultKind,
        JourneyKind.flight,
      });

  Future<void> _loadJourneysOnSelectedDate({
    int? requestVersion,
    Set<JourneyKind>? journeyKinds,
  }) async {
    final version = requestVersion ?? ++_requestVersion;
    final kinds = journeyKinds ?? Set<JourneyKind>.from(selectedJourneyKinds);
    final date = selectedDate;
    final headers = await api.listJourneysOnDate(
      year: date.year,
      month: date.month,
      day: date.day,
      journeyKinds: kinds,
    );
    if (_isDisposed ||
        version != _requestVersion ||
        !setEquals(kinds, selectedJourneyKinds) ||
        date != selectedDate) {
      return;
    }
    journeyHeaders = headers.reversed.toList();
    notifyListeners();
  }

  bool _hasJourneyOn(DateTime date) => journeyDates.any((journeyDate) =>
      journeyDate.year == date.year &&
      journeyDate.month == date.month &&
      journeyDate.day == date.day);

  DateTime? _nearestDateInMonth(int year, int month, DateTime target) =>
      _nearestDate(
        journeyDates.where((date) => date.year == year && date.month == month),
        target,
      );

  DateTime? _nearestDate(Iterable<DateTime> dates, DateTime target) {
    final iterator = dates.iterator;
    if (!iterator.moveNext()) return null;
    var closest = iterator.current;
    while (iterator.moveNext()) {
      final candidate = iterator.current;
      final closestDistance = closest.difference(target).inDays.abs();
      final candidateDistance = candidate.difference(target).inDays.abs();
      if (candidateDistance < closestDistance ||
          (candidateDistance == closestDistance &&
              candidate.isAfter(closest))) {
        closest = candidate;
      }
    }
    return closest;
  }

  @override
  void dispose() {
    _isDisposed = true;
    super.dispose();
  }
}
