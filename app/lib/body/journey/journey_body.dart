import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/constants/index.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';

class JourneyBody extends StatefulWidget {
  const JourneyBody({super.key});

  @override
  State<JourneyBody> createState() => _JourneyBodyState();
}

class _JourneyBodyState extends State<JourneyBody> {
  static const _landscapeContentPadding = 16.0;
  static const _landscapeColumnGap = 16.0;
  static const _landscapeCalendarMinWidth = 320.0;
  static const _landscapeCalendarMaxWidth = 360.0;
  static const _landscapeListMinWidth = 280.0;

  List<JourneyHeader> _journeyHeaderList = [];

  DateTime _selectedDate = DateTime.now();
  DateTime? _firstDate;
  final lastDate = DateTime.now();
  late List<int> _yearsWithJourneyList;
  late List<int> _monthsWithJourneyList;
  late List<int> _daysWithJourneyList;
  bool _isLoadingFirstDate = true;
  bool _isLoadingJourneyHeaderList = false;
  Object? _activeJourneyLoad;

  @override
  void initState() {
    super.initState();
    _initialize();
  }

  Future<void> _initialize() async {
    final earliestDate = await api.earliestJourneyDate();
    final firstDate =
        earliestDate == null ? null : naiveDateToDateTime(earliestDate);
    final yearsWithJourneyList = await api.yearsWithJourney();
    final monthsWithJourneyList =
        await api.monthsWithJourney(year: _selectedDate.year);
    final daysWithJourneyList = await api.daysWithJourney(
        year: _selectedDate.year, month: _selectedDate.month);
    final journeyHeaderList = await api.listJourneyOnDate(
        year: _selectedDate.year,
        month: _selectedDate.month,
        day: _selectedDate.day);
    if (!mounted) return;
    _firstDate = firstDate;
    setState(() {
      _yearsWithJourneyList = yearsWithJourneyList;
      _monthsWithJourneyList = monthsWithJourneyList;
      _daysWithJourneyList = daysWithJourneyList;
      _journeyHeaderList = journeyHeaderList.reversed.toList();
      _isLoadingFirstDate = false;
    });
  }

  Future<void> _loadJourneys(
    DateTime targetDate, {
    bool preferTargetDay = true,
    bool reloadYears = false,
  }) async {
    if (!mounted) return;
    final token = Object();
    _activeJourneyLoad = token;
    setState(() {
      _selectedDate = targetDate;
      _isLoadingJourneyHeaderList = true;
      _journeyHeaderList = [];
    });
    try {
      final yearsWithJourneyList =
          reloadYears ? await api.yearsWithJourney() : null;

      if (yearsWithJourneyList != null && yearsWithJourneyList.isEmpty) {
        if (!mounted || !identical(token, _activeJourneyLoad)) return;
        setState(() {
          _firstDate = null;
          _yearsWithJourneyList = [];
          _monthsWithJourneyList = [];
          _daysWithJourneyList = [];
          _journeyHeaderList = [];
        });
        return;
      }

      final monthsWithJourneyList =
          await api.monthsWithJourney(year: targetDate.year);
      final daysWithJourneyList = await api.daysWithJourney(
          year: targetDate.year, month: targetDate.month);
      var selectedDate = targetDate;
      if (daysWithJourneyList.isNotEmpty &&
          (!preferTargetDay ||
              !daysWithJourneyList.contains(selectedDate.day))) {
        selectedDate = DateTime(
          selectedDate.year,
          selectedDate.month,
          daysWithJourneyList.first,
        );
      }
      final journeyHeaderList = await api.listJourneyOnDate(
          year: selectedDate.year,
          month: selectedDate.month,
          day: selectedDate.day);
      if (!mounted || !identical(token, _activeJourneyLoad)) return;
      setState(() {
        if (yearsWithJourneyList != null) {
          _yearsWithJourneyList = yearsWithJourneyList;
        }
        _monthsWithJourneyList = monthsWithJourneyList;
        _daysWithJourneyList = daysWithJourneyList;
        _selectedDate = selectedDate;
        _journeyHeaderList = journeyHeaderList.reversed.toList();
      });
    } finally {
      if (mounted && identical(token, _activeJourneyLoad)) {
        _activeJourneyLoad = null;
        setState(() {
          _isLoadingJourneyHeaderList = false;
        });
      }
    }
  }

  Widget _buildDatePickerWithValue(DateTime firstDate) {
    final config = CalendarDatePicker2Config(
      firstDate: firstDate,
      lastDate: DateTime.now(),
      centerAlignModePicker: true,
      calendarType: CalendarDatePicker2Type.single,
      selectedDayHighlightColor: const Color(0xFFB6E13D).withAlpha(230),
      dayTextStyle: const TextStyle(
        color: Colors.white,
      ),
      weekdayLabelTextStyle: const TextStyle(
        color: Colors.white,
        fontWeight: FontWeight.bold,
      ),
      controlsTextStyle: const TextStyle(
        color: Colors.white,
        fontSize: 15,
        fontWeight: FontWeight.bold,
      ),
      selectableYearPredicate: (year) => _yearsWithJourneyList.contains(year),
      selectableMonthPredicate: (year, month) =>
          _monthsWithJourneyList.contains(month),
      selectableDayPredicate: (day) => _daysWithJourneyList.contains(day.day),
      dayBuilder: ({
        required date,
        textStyle,
        decoration,
        isSelected,
        isDisabled,
        isToday,
      }) {
        Widget? dayWidget;
        if (_daysWithJourneyList.contains(date.day)) {
          dayWidget = Container(
            decoration: decoration,
            child: Center(
              child: Stack(
                alignment: AlignmentDirectional.center,
                children: [
                  Text(
                    MaterialLocalizations.of(context).formatDecimal(date.day),
                    style: textStyle,
                  ),
                  Padding(
                    padding: const EdgeInsets.only(top: 27.5),
                    child: Container(
                      height: 4,
                      width: 4,
                      decoration: BoxDecoration(
                        borderRadius: BorderRadius.circular(5),
                        color: const Color(0xFFB6E13D),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          );
        }
        return dayWidget;
      },
      dynamicCalendarRows: true,
      disabledDayTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
      disabledMonthTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
      disabledYearTextStyle:
          const TextStyle(color: Colors.grey, fontWeight: FontWeight.w400),
      // Turn off CalendarDatePicker2's own vibration; it varies by platform.
      // We call AppHaptics on date/month changes instead.
      // TODO: Fix CalendarDatePicker2's built-in vibration being inconsistent
      //       across platforms (local patch or upstream), so we can rely on it.
      disableVibration: true,
    );
    return CalendarDatePicker2(
      config: config,
      displayedMonthDate: _selectedDate,
      value: _daysWithJourneyList.contains(_selectedDate.day)
          ? [_selectedDate]
          : [],
      onValueChanged: (dates) {
        AppHaptics.selection();
        _loadJourneys(dates.first);
      },
      onDisplayedMonthChanged: (value) {
        AppHaptics.selection();
        final selectedDate = _selectedDate;
        DateTime jumpToDate =
            DateTime(value.year, value.month, selectedDate.day);
        DateTime jumpToDateMonthLastDay =
            DateTime(value.year, value.month + 1, 0);
        if (selectedDate.day > jumpToDateMonthLastDay.day) {
          jumpToDate = jumpToDateMonthLastDay;
        }
        if (lastDate.isBefore(jumpToDate)) {
          jumpToDate = lastDate;
        }
        if (firstDate.isAfter(jumpToDate)) {
          jumpToDate = firstDate;
        }
        _loadJourneys(jumpToDate, preferTargetDay: false);
      },
    );
  }

  Widget _buildJourneyHeaderList() {
    if (_isLoadingJourneyHeaderList) {
      return const Center(child: CircularProgressIndicator());
    }
    return ListView.builder(
      padding: EdgeInsets.only(
        bottom: StyleConstants.navBarSafeArea + 5,
      ),
      itemCount: _journeyHeaderList.length,
      itemBuilder: (context, index) {
        return LabelTile(
          label: _journeyHeaderList[index].start != null
              ? DateFormat("yyyy-MM-dd HH:mm:ss")
                  .format(_journeyHeaderList[index].start!.toLocal())
              : naiveDateToString(date: _journeyHeaderList[index].journeyDate),
          trailing: LabelTileContent(showArrow: true),
          onTap: () {
            navigatorPush(
              context,
              page: JourneyInfoPage(
                journeyHeader: _journeyHeaderList[index],
              ),
            ).then((refresh) {
              if (refresh == true) {
                _loadJourneys(_selectedDate, reloadYears: true);
              }
            });
          },
        );
      },
    );
  }

  Widget _buildLandscapeBody(DateTime firstDate) {
    const bottomPadding = StyleConstants.navBarSafeArea + 5;

    return LayoutBuilder(
      builder: (context, constraints) {
        final availableWidth = constraints.maxWidth -
            _landscapeContentPadding * 2 -
            _landscapeColumnGap;
        final preferredCalendarWidth = availableWidth * 0.42;
        final maxCalendarWidth = (availableWidth - _landscapeListMinWidth)
            .clamp(0.0, _landscapeCalendarMaxWidth)
            .toDouble();
        final minCalendarWidth = maxCalendarWidth < _landscapeCalendarMinWidth
            ? maxCalendarWidth
            : _landscapeCalendarMinWidth;
        final calendarWidth = preferredCalendarWidth
            .clamp(minCalendarWidth, maxCalendarWidth)
            .toDouble();

        return Padding(
          padding: const EdgeInsets.all(_landscapeContentPadding),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                width: calendarWidth,
                child: SingleChildScrollView(
                  padding: EdgeInsets.only(bottom: bottomPadding),
                  child: _buildDatePickerWithValue(firstDate),
                ),
              ),
              const SizedBox(width: _landscapeColumnGap),
              Expanded(
                child: _buildJourneyHeaderList(),
              ),
            ],
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_isLoadingFirstDate) {
      return const Center(child: CircularProgressIndicator());
    }
    final firstDate = _firstDate;
    if (firstDate == null) {
      return Center(child: Text(context.tr("journey.no_data")));
    } else {
      final isLandscape =
          MediaQuery.of(context).orientation == Orientation.landscape;
      if (isLandscape) {
        return _buildLandscapeBody(firstDate);
      }
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _buildDatePickerWithValue(firstDate),
          const SizedBox(height: 16.0),
          Expanded(child: _buildJourneyHeaderList()),
        ],
      );
    }
  }
}
