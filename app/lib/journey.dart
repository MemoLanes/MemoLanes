import 'dart:collection';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/journey_info.dart';
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:table_calendar/table_calendar.dart';
import 'package:month_year_picker/month_year_picker.dart';
import 'package:easy_localization/easy_localization.dart';

class JourneyUiBody extends StatefulWidget {
  const JourneyUiBody({super.key});

  @override
  State<JourneyUiBody> createState() => _JourneyUiBodyState();
}

class _JourneyUiBodyState extends State<JourneyUiBody> {
  final ValueNotifier<List<JourneyHeader>> _journeyHeaderList =
      ValueNotifier<List<JourneyHeader>>([]);

  late PageController _pageController;
  late final DateTime? firstDate;
  final lastDate = DateTime.now();
  final ValueNotifier<DateTime> _focusedDay = ValueNotifier(DateTime.now());
  bool _isLoadingFirstDate = true;
  DateTime? _selectedDay;
  LinkedHashMap<DateTime, int>? _daysWithJourney;

  @override
  void initState() {
    super.initState();
    _initializeFirstDate();
    _loadDaysWithJourneyForGivenMonth(_focusedDay.value);
  }

  Future<void> _initializeFirstDate() async {
    NaiveDate? earliestDate = await api.earliestJourneyDate();
    if (earliestDate != null) {
      firstDate = DateTime.parse(naiveDateToString(date: earliestDate));
    } else {
      firstDate = null;
    }
    setState(() {
      _isLoadingFirstDate = false;
    });
  }

  Future<DateTime?> _selectDate(
      BuildContext context, DateTime? datetime) async {
    DateTime? selectedDate = await showMonthYearPicker(
      context: context,
      initialDate: _focusedDay.value,
      firstDate: firstDate!,
      lastDate: lastDate,
      builder: (context, child) {
        return LayoutBuilder(
          builder: (context, constraints) {
            double dialogWidth = constraints.maxWidth * 0.9;
            double dialogHeight = constraints.maxHeight * 0.6;
            return Center(
              child: SizedBox(
                width: dialogWidth,
                height: dialogHeight,
                child: child,
              ),
            );
          },
        );
      },
    );
    if (selectedDate != null) {
      selectedDate = DateTime(
          selectedDate.year, selectedDate.month, _focusedDay.value.day);
      if (firstDate!.isAfter(selectedDate)) {
        selectedDate = firstDate;
      }
      if (lastDate.isBefore(selectedDate!)) {
        selectedDate = lastDate;
      }
    }
    return selectedDate;
  }

  Future<void> _loadDaysWithJourneyForGivenMonth(DateTime selectedDay) async {
    var data = await api.daysWithJourney(
      year: selectedDay.year,
      month: selectedDay.month,
    );
    setState(() {
      _daysWithJourney = LinkedHashMap<DateTime, int>.from({
        for (var day in data)
          DateTime.utc(_focusedDay.value.year, _focusedDay.value.month, day):
              day,
      });
    });
  }

  List<int> _eventsForGivenDay(DateTime day) {
    var event = _daysWithJourney?[day];
    if (event == null) {
      return [];
    } else {
      return [event];
    }
  }

  void _onDaySelected(DateTime selectedDay, DateTime focusedDay) async {
    if (!isSameDay(_selectedDay, selectedDay)) {
      setState(() {
        _selectedDay = selectedDay;
        _focusedDay.value = focusedDay;
      });

      _journeyHeaderList.value = await api.listJournyOnDate(
          year: selectedDay.year,
          month: selectedDay.month,
          day: selectedDay.day);
      _loadDaysWithJourneyForGivenMonth(selectedDay);
    }
  }

  void _updateJourneyHeaderList() async {
    _journeyHeaderList.value = await api.listJournyOnDate(
        year: _focusedDay.value.year,
        month: _focusedDay.value.month,
        day: _focusedDay.value.day);
  }

  Widget _buildCalendarHeader() {
    return ValueListenableBuilder<DateTime>(
      valueListenable: _focusedDay,
      builder: (context, value, _) {
        return _CalendarHeader(
          focusedDay: value,
          onSelectedDateTap: () async {
            var selectedDay = await _selectDate(context, _focusedDay.value) ??
                _focusedDay.value;
            _onDaySelected(selectedDay, selectedDay);
          },
          onReturnTodayTap: () {
            setState(() {
              _selectedDay = lastDate;
              _focusedDay.value = lastDate;
            });
          },
          onLeftArrowTap: () {
            DateTime nextDate = DateTime(_focusedDay.value.year,
                _focusedDay.value.month - 1, _focusedDay.value.day);
            if (nextDate.isBefore(firstDate!)) {
              setState(() {
                _focusedDay.value = firstDate!;
                _selectedDay = _focusedDay.value;
              });
              return;
            }
            _selectedDay = nextDate;
            _pageController.previousPage(
              duration: const Duration(milliseconds: 300),
              curve: Curves.easeOut,
            );
            _loadDaysWithJourneyForGivenMonth(_focusedDay.value);
          },
          onRightArrowTap: () {
            DateTime nextDate = DateTime(_focusedDay.value.year,
                _focusedDay.value.month + 1, _focusedDay.value.day);
            if (nextDate.isAfter(lastDate)) {
              setState(() {
                _focusedDay.value = lastDate;
                _selectedDay = _focusedDay.value;
              });
              return;
            }
            _selectedDay = nextDate;
            _pageController.nextPage(
              duration: const Duration(milliseconds: 300),
              curve: Curves.easeOut,
            );
          },
        );
      },
    );
  }

  Widget _buildTableCalendar() {
    return TableCalendar<int>(
      firstDay: firstDate!,
      lastDay: lastDate,
      focusedDay: _focusedDay.value,
      headerVisible: false,
      selectedDayPredicate: (day) => isSameDay(_selectedDay, day),
      eventLoader: _eventsForGivenDay,
      onCalendarCreated: (controller) async {
        _pageController = controller;
        _selectedDay = _focusedDay.value;
        _updateJourneyHeaderList();
      },
      onDaySelected: _onDaySelected,
      onPageChanged: (focusedDay) {
        _selectedDay =
            DateTime(focusedDay.year, focusedDay.month, _selectedDay!.day);
        if (lastDate.isBefore(_selectedDay!)) {
          _selectedDay = lastDate;
        }
        if (firstDate!.isAfter(_selectedDay!)) {
          _selectedDay = firstDate;
        }
        _focusedDay.value = _selectedDay!;
        _loadDaysWithJourneyForGivenMonth(_focusedDay.value);
        _updateJourneyHeaderList();
      },
    );
  }

  Widget _buildJourneyHeaderList() {
    return Expanded(
      child: ValueListenableBuilder<List<JourneyHeader>>(
        valueListenable: _journeyHeaderList,
        builder: (context, value, _) {
          return ListView.builder(
            itemCount: value.length,
            itemBuilder: (context, index) {
              return Container(
                margin: const EdgeInsets.symmetric(
                  horizontal: 12.0,
                  vertical: 4.0,
                ),
                decoration: BoxDecoration(
                  border: Border.all(),
                  borderRadius: BorderRadius.circular(12.0),
                ),
                child: ListTile(
                  title:
                      Text(naiveDateToString(date: value[index].journeyDate)),
                  onTap: () {
                    Navigator.push(context, MaterialPageRoute(
                      builder: (context) {
                        return JourneyInfoPage(
                          journeyHeader: value[index],
                        );
                      },
                    )).then((refresh) async {
                      if (refresh != null && refresh) {
                        _journeyHeaderList.value = await api.listJournyOnDate(
                            year: _focusedDay.value.year,
                            month: _focusedDay.value.month,
                            day: _focusedDay.value.day);
                        _loadDaysWithJourneyForGivenMonth(_focusedDay.value);
                      }
                    });
                  },
                ),
              );
            },
          );
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_isLoadingFirstDate) {
      // 渲染加载状态
      return const Center(child: CircularProgressIndicator());
    }
    if (firstDate == null) {
      return Scaffold(
          body: Center(
        child: Text(context.tr("journey.empty_journey_data")),
      ));
    } else {
      return Scaffold(
          body: Column(children: [
        _buildCalendarHeader(),
        _buildTableCalendar(),
        const SizedBox(height: 8.0),
        _buildJourneyHeaderList(),
      ]));
    }
  }
}

class _CalendarHeader extends StatelessWidget {
  final DateTime focusedDay;
  final VoidCallback onLeftArrowTap;
  final VoidCallback onRightArrowTap;
  final VoidCallback onSelectedDateTap;
  final VoidCallback onReturnTodayTap;

  const _CalendarHeader({
    required this.focusedDay,
    required this.onLeftArrowTap,
    required this.onRightArrowTap,
    required this.onSelectedDateTap,
    required this.onReturnTodayTap,
  });

  @override
  Widget build(BuildContext context) {
    final headerText = DateFormat.yMMM().format(focusedDay);

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: Row(
        children: [
          const SizedBox(width: 16.0),
          SizedBox(
            child: Text(
              headerText,
              style: const TextStyle(fontSize: 20.0),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.calendar_month, size: 20.0),
            visualDensity: VisualDensity.compact,
            onPressed: onSelectedDateTap,
          ),
          IconButton(
            icon: const Icon(Icons.calendar_today, size: 18.0),
            visualDensity: VisualDensity.compact,
            onPressed: onReturnTodayTap,
          ),
          const Spacer(),
          IconButton(
            icon: const Icon(Icons.chevron_left),
            onPressed: onLeftArrowTap,
          ),
          IconButton(
            icon: const Icon(Icons.chevron_right),
            onPressed: onRightArrowTap,
          ),
        ],
      ),
    );
  }
}
