import 'package:calendar_date_picker2/calendar_date_picker2.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/constants/index.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyBody extends StatefulWidget {
  const JourneyBody({super.key});

  @override
  State<JourneyBody> createState() => _JourneyBodyState();
}

class _JourneyBodyState extends State<JourneyBody> {
  List<JourneyHeader> _journeyHeaderList = [];

  DateTime _selectedDate = DateTime.now();
  late final DateTime? _firstDate;
  final lastDate = DateTime.now();
  late List<int> _yearsWithJourneyList;
  late List<int> _monthsWithJourneyList;
  late List<int> _daysWithJourneyList;
  bool _isLoadingFirstDate = true;

  @override
  void initState() {
    super.initState();
    _initialize();
    _updateJourneyHeaderList();
  }

  Future<void> _initialize() async {
    NaiveDate? earliestDate = await api.earliestJourneyDate();
    if (earliestDate != null) {
      _firstDate = DateTime.parse(naiveDateToString(date: earliestDate));
    } else {
      _firstDate = null;
    }
    _yearsWithJourneyList = await api.yearsWithJourney();
    _monthsWithJourneyList =
        await api.monthsWithJourney(year: _selectedDate.year);
    _daysWithJourneyList = await api.daysWithJourney(
        year: _selectedDate.year, month: _selectedDate.month);
    setState(() {
      _isLoadingFirstDate = false;
    });
  }

  void _updateJourneyHeaderList() async {
    final journeyHeaderList = await api.listJournyOnDate(
        year: _selectedDate.year,
        month: _selectedDate.month,
        day: _selectedDate.day);
    setState(() {
      _journeyHeaderList = journeyHeaderList.reversed.toList();
    });
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
    );
    return CalendarDatePicker2(
      config: config,
      value: [_selectedDate],
      onValueChanged: (dates) {
        setState(() => _selectedDate = dates.first);
        _updateJourneyHeaderList();
      },
      onDisplayedMonthChanged: (value) async {
        DateTime jumpToDate =
            DateTime(value.year, value.month, _selectedDate.day);
        DateTime jumpToDateMonthLastDay =
            DateTime(value.year, value.month + 1, 0);
        if (_selectedDate.day > jumpToDateMonthLastDay.day) {
          jumpToDate = jumpToDateMonthLastDay;
        }
        if (lastDate.isBefore(jumpToDate)) {
          jumpToDate = lastDate;
        }
        if (firstDate.isAfter(jumpToDate)) {
          jumpToDate = firstDate;
        }
        if (value.year != _selectedDate.year) {
          _monthsWithJourneyList =
              await api.monthsWithJourney(year: jumpToDate.year);
        }

        _daysWithJourneyList = await api.daysWithJourney(
            month: jumpToDate.month, year: jumpToDate.year);
        setState(() {
          _selectedDate = jumpToDate;
        });
        _updateJourneyHeaderList();
      },
    );
  }

  Widget _buildJourneyHeaderList() {
    return Expanded(
      child: ListView.builder(
        padding: EdgeInsets.only(
          bottom: MediaQuery.of(context).padding.bottom +
              StyleConstants.navBarSafeArea +
              5,
        ),
        itemCount: _journeyHeaderList.length,
        itemBuilder: (context, index) {
          return LabelTile(
            label: _journeyHeaderList[index].start != null
                ? DateFormat("yyyy-MM-dd HH:mm:ss")
                    .format(_journeyHeaderList[index].start!.toLocal())
                : naiveDateToString(
                    date: _journeyHeaderList[index].journeyDate),
            trailing: LabelTileContent(showArrow: true),
            onTap: () {
              Navigator.push(context, MaterialPageRoute(
                builder: (context) {
                  return JourneyInfoPage(
                    journeyHeader: _journeyHeaderList[index],
                  );
                },
              )).then((refresh) async {
                if (refresh != null && refresh) {
                  _yearsWithJourneyList = await api.yearsWithJourney();
                  _monthsWithJourneyList =
                      await api.monthsWithJourney(year: _selectedDate.year);
                  _daysWithJourneyList = await api.daysWithJourney(
                      year: _selectedDate.year, month: _selectedDate.month);
                  _updateJourneyHeaderList();
                }
              });
            },
          );
        },
      ),
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
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _buildDatePickerWithValue(firstDate),
          const SizedBox(height: 16.0),
          _buildJourneyHeaderList(),
        ],
      );
    }
  }
}
