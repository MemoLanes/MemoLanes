import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/body/settings/import_data_page.dart' show ImportType;
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyInfoEditPage extends StatefulWidget {
  const JourneyInfoEditPage({
    super.key,
    required this.startTime,
    required this.endTime,
    required this.journeyDate,
    required this.note,
    required this.saveData,
    this.previewData,
    this.journeyKind,
    this.importType,
  });

  final DateTime? startTime;
  final DateTime? endTime;
  final NaiveDate journeyDate;
  final String? note;
  final JourneyKind? journeyKind;
  final Function saveData;
  final Function? previewData;
  final ImportType? importType;

  @override
  State<JourneyInfoEditPage> createState() => _JourneyInfoEditPageState();
}

class _JourneyInfoEditPageState extends State<JourneyInfoEditPage> {
  final DateFormat dateTimeFormat = DateFormat('yyyy-MM-dd HH:mm:ss');
  final DateFormat dateFormat = DateFormat("yyyy-MM-dd");
  final DateTime firstDate = DateTime(1990);
  DateTime? _startTime;
  DateTime? _endTime;
  DateTime? _journeyDate;
  String? _note;
  JourneyKind _journeyKind = JourneyKind.defaultKind;
  import_api.JourneyInfo? journeyInfo;
  final TextEditingController _noteController = TextEditingController();
  import_api.ImportProcessor _runPreprocessor = import_api.ImportProcessor.none;

  Future<DateTime?> selectDateAndTime(BuildContext context,
      DateTime? datetime) async {
    final now = DateTime.now();
    datetime ??= now;
    DateTime? selectedDateTime = await showDatePicker(
      context: context,
      initialDate: datetime,
      firstDate: firstDate,
      lastDate: now,
    );

    TimeOfDay initialTime =
    TimeOfDay(hour: datetime.hour, minute: datetime.minute);

    if (selectedDateTime != null) {
      if (!context.mounted) return null;
      TimeOfDay? selectedTime = await showTimePicker(
        context: context,
        initialTime: initialTime,
      );

      if (selectedTime != null) {
        selectedDateTime = DateTime(
          selectedDateTime.year,
          selectedDateTime.month,
          selectedDateTime.day,
          selectedTime.hour,
          selectedTime.minute,
        );
        return selectedDateTime;
      }
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    setState(() {
      _startTime = widget.startTime;
      _endTime = widget.endTime;
      _journeyDate =
          dateFormat.parse(naiveDateToString(date: widget.journeyDate));
      _note = widget.note;
      _journeyKind = widget.journeyKind ?? _journeyKind;
      _noteController.text = _note ?? "";
      _noteController.addListener(() {
        setState(() {
          _note = _noteController.text;
        });
      });
    });
    if (widget.previewData != null) {
      widget.previewData!(_runPreprocessor);
    }
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  void _saveData(BuildContext context) async {
    if (_journeyDate == null) {
      Fluttertoast.showToast(msg: context.tr("journey.journey_date_is_empty"));
      return;
    }
    _note ??= "";
    import_api.JourneyInfo journeyInfo = import_api.JourneyInfo(
        journeyDate: naiveDateOfString(str: dateFormat.format(_journeyDate!)),
        startTime: _startTime,
        endTime: _endTime,
        note: _note,
        journeyKind: _journeyKind);
    if (widget.importType != null) {
      await widget.saveData(journeyInfo, _runPreprocessor);
    } else {
      await widget.saveData(journeyInfo);
    }
    if (context.mounted) {
      Navigator.pop(context, true);
    }
  }

  @override
  Widget build(BuildContext context) {
    final width = MediaQueryData
        .fromView(View.of(context))
        .size
        .width;
    return ConstrainedBox(
      constraints: BoxConstraints(
        maxHeight: 440,
        minHeight: 420,
      ),
      child: MlSingleChildScrollView(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          LabelTile(
            label: context.tr("journey.start_time"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
                content: _startTime != null
                    ? dateTimeFormat.format(_startTime!.toLocal())
                    : ""),
            onTap: () async {
              DateTime? time = await selectDateAndTime(context, _startTime);
              if (time != null) {
                setState(() {
                  _startTime = time;
                });
              }
            },
          ),
          LabelTile(
            label: context.tr("journey.end_time"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
                content: _endTime != null
                    ? dateTimeFormat.format(_endTime!.toLocal())
                    : ""),
            onTap: () async {
              DateTime? time = await selectDateAndTime(context, _endTime);
              if (time != null) {
                setState(() {
                  _endTime = time;
                });
              }
            },
          ),
          LabelTile(
            label: context.tr("journey.journey_date"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
                content: _journeyDate != null
                    ? dateFormat.format(_journeyDate!)
                    : ''),
            onTap: () async {
              DateTime? time = await showDatePicker(
                context: context,
                initialDate: _journeyDate,
                firstDate: firstDate,
                lastDate: DateTime.now(),
              );
              if (time != null) {
                setState(() {
                  _journeyDate = time;
                });
              }
            },
          ),
          if (widget.importType != null)
            widget.importType == ImportType.fow
                ? SizedBox.shrink()
                : LabelTile(
              label: context.tr("journey.preprocessor"),
              labelOnTap: () =>
                showCommonDialog(
                  context,
                  context.tr("preprocessor.description_md"),
                  markdown: true,
                )
            ,
              position: LabelTilePosition.single,
              trailing: LabelTileContent(
                content: switch (_runPreprocessor) {
                  import_api.ImportProcessor.none =>
                      context.tr("preprocessor.none"),
                  import_api.ImportProcessor.generic =>
                      context.tr("preprocessor.generic"),
                  import_api.ImportProcessor.flightTrack =>
                      context.tr("preprocessor.flightTrack"),
                },
                showArrow: true,
              ),
              onTap: () => _showJourneyPreprocessorCard(context),
            ),
          LabelTile(
            label: context.tr("journey.journey_kind"),
            position: LabelTilePosition.single,
            trailing: LabelTileContent(
                content: _journeyKind == JourneyKind.defaultKind
                    ? context.tr("journey_kind.default")
                    : context.tr("journey_kind.flight"),
                showArrow: true),
            onTap: () => _showJourneyKindCard(context),
          ),
          LabelTile(
            label: context.tr("journey.note"),
            position: LabelTilePosition.single,
            maxHeight: 150,
            trailing: SizedBox(
              width: width * 0.6,
              child: TextField(
                controller: _noteController,
                keyboardType: TextInputType.multiline,
                textInputAction: TextInputAction.newline,
                maxLines: 5,
                minLines: 1,
                decoration: InputDecoration(
                  border: InputBorder.none,
                  counterText: '',
                  hintText: context.tr("common.please_enter"),
                  hintStyle: TextStyle(
                    fontSize: 14.0,
                  ),
                ),
                textAlign: TextAlign.right,
              ),
            ),
          ),
          ElevatedButton(
            onPressed: () => _saveData(context),
            style: ElevatedButton.styleFrom(
              backgroundColor: const Color(0xFFB6E13D),
              foregroundColor: Colors.black,
              fixedSize: Size(280, 42),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(25.0),
              ),
            ),
            child: Text(context.tr("common.save")),
          ),
        ],
      ),
    );
  }

  void _showJourneyKindCard(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("journey_kind.default"),
            onTap: () {
              setState(() {
                _journeyKind = JourneyKind.defaultKind;
              });
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("journey_kind.flight"),
            onTap: () {
              setState(() {
                _journeyKind = JourneyKind.flight;
              });
            },
          ),
        ],
      ),
    );
  }

  void _showJourneyPreprocessorCard(BuildContext context) {
    showBasicCard(
      context,
      child: OptionCard(
        children: [
          CardLabelTile(
            position: CardLabelTilePosition.top,
            label: context.tr("preprocessor.none"),
            onTap: () {
              setState(() {
                _runPreprocessor = import_api.ImportProcessor.none;
              });
            },
            top: false,
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("preprocessor.generic"),
            onTap: () {
              setState(() {
                _runPreprocessor = import_api.ImportProcessor.generic;
              });
            },
          ),
          CardLabelTile(
            position: CardLabelTilePosition.bottom,
            label: context.tr("preprocessor.flightTrack"),
            onTap: () {
              setState(() {
                _runPreprocessor = import_api.ImportProcessor.flightTrack;
              });
            },
          )
        ],
      ),
    );
  }
}
