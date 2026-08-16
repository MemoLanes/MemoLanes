import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/basic_bottom_sheet.dart';
import 'package:memolanes/common/component/cards/card_label_tile.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/multi_journey_import_page.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class VectorMultiImportPage extends StatefulWidget {
  const VectorMultiImportPage({
    super.key,
    required this.vectorData,
    required this.parts,
    required this.initialPreprocessor,
  });

  final import_api.RawVectorData vectorData;
  final List<import_api.VectorImportPartSummary> parts;
  final import_api.ImportPreprocessor initialPreprocessor;

  @override
  State<VectorMultiImportPage> createState() => _VectorMultiImportPageState();
}

class _VectorMultiImportPageState extends State<VectorMultiImportPage> {
  late Set<String> _selectedDates;
  late import_api.ImportPreprocessor _preprocessor;
  JourneyKind _journeyKind = JourneyKind.defaultKind;
  final TextEditingController _noteController = TextEditingController();
  final DateFormat _timeFormat = DateFormat('HH:mm');
  bool _isImporting = false;

  @override
  void initState() {
    super.initState();
    _preprocessor = widget.initialPreprocessor;
    _selectedDates = widget.parts.map(_dateKey).toSet();
  }

  @override
  void dispose() {
    _noteController.dispose();
    super.dispose();
  }

  String _dateKey(import_api.VectorImportPartSummary part) => part.journeyDate;

  import_api.VectorImportPartSummary _partForKey(String key) =>
      widget.parts.firstWhere((part) => _dateKey(part) == key);

  String _description(import_api.VectorImportPartSummary part) {
    final timeRange = part.startTime != null && part.endTime != null
        ? '${_timeFormat.format(part.startTime!.toLocal())}–${_timeFormat.format(part.endTime!.toLocal())}'
        : context.tr('import.vector_multi.unknown_time');
    final points = context.tr(
      'import.vector_multi.point_count',
      args: ['${part.pointCount}'],
    );
    final missing = part.missingTimestampCount > BigInt.zero
        ? context.tr(
            'import.vector_multi.missing_time_count',
            args: ['${part.missingTimestampCount}'],
          )
        : null;
    return [timeRange, points, missing].whereType<String>().join(' · ');
  }

  String _preprocessorLabel(import_api.ImportPreprocessor value) =>
      switch (value) {
        import_api.ImportPreprocessor.none =>
          context.tr('import.preprocessor.none'),
        import_api.ImportPreprocessor.generic =>
          context.tr('import.preprocessor.generic'),
        import_api.ImportPreprocessor.flightTrack =>
          context.tr('import.preprocessor.flight_track'),
        import_api.ImportPreprocessor.spare =>
          context.tr('import.preprocessor.spare'),
      };

  Future<void> _openPreview(String key) async {
    final part = _partForKey(key);
    try {
      final journeyData = await showLoadingDialog(
        asyncTask: import_api.processVectorDataForDate(
          vectorData: widget.vectorData,
          journeyDate: key,
          importProcessor: _preprocessor,
        ),
      );
      if (!mounted) return;
      await Navigator.of(context).push(
        MaterialPageRoute(
          builder: (context) => JourneyInfoPage(
            journeyHeader: JourneyHeader(
              id: key,
              revision: '',
              journeyDate: naiveDateOfString(str: part.journeyDate),
              createdAt: DateTime.now().toUtc(),
              start: part.startTime,
              end: part.endTime,
              journeyType: JourneyType.vector,
              journeyKind: _journeyKind,
              note: _noteController.text,
            ),
            previewJourneyData: journeyData,
          ),
        ),
      );
    } catch (error, stackTrace) {
      log.error('[VectorMultiImportPage] preview failed: $error', stackTrace);
      if (mounted) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
      }
    }
  }

  Future<void> _confirmImport() async {
    if (_isImporting) return;

    if (_selectedDates.isEmpty) {
      await showCommonDialog(
        context,
        context.tr('import.journey_selection.select_at_least_one'),
      );
      return;
    }

    setState(() => _isImporting = true);
    try {
      final selectedParts = widget.parts
          .where((part) => _selectedDates.contains(_dateKey(part)))
          .toList();
      final count = await showLoadingDialog(
        asyncTask: import_api.importVectorDataByDate(
          vectorData: widget.vectorData,
          journeyDates: selectedParts.map(_dateKey).toList(),
          importProcessor: _preprocessor,
          journeyKind: _journeyKind,
          note: _noteController.text,
        ),
      );
      if (!mounted) return;
      if (count == BigInt.zero) {
        await showCommonDialog(context, context.tr('import.empty_data'));
        return;
      }
      await showCommonDialog(
        context,
        context.tr('import.vector_multi.successful', args: ['$count']),
      );
      if (!mounted) return;
      popCurrentRoute(context, true);
    } catch (error, stackTrace) {
      log.error('[VectorMultiImportPage] import failed: $error', stackTrace);
      if (mounted) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
      }
    } finally {
      if (mounted) {
        setState(() => _isImporting = false);
      }
    }
  }

  void _showPreprocessorPicker() {
    showBasicCard(
      context,
      child: OptionCard(
        children: import_api.ImportPreprocessor.values.map((value) {
          return CardLabelTile(
            position: value == import_api.ImportPreprocessor.values.first
                ? CardLabelTilePosition.top
                : value == import_api.ImportPreprocessor.values.last
                    ? CardLabelTilePosition.bottom
                    : CardLabelTilePosition.middle,
            label: value == _preprocessor
                ? '✓ ${_preprocessorLabel(value)}'
                : _preprocessorLabel(value),
            onTap: () {
              setState(() => _preprocessor = value);
            },
          );
        }).toList(),
      ),
    );
  }

  void _showJourneyKindPicker() {
    showBasicCard(
      context,
      child: OptionCard(
        children: JourneyKind.values.map((value) {
          return CardLabelTile(
            position: value == JourneyKind.values.first
                ? CardLabelTilePosition.top
                : CardLabelTilePosition.bottom,
            label:
                '${value == _journeyKind ? '✓ ' : ''}${value == JourneyKind.defaultKind ? context.tr('journey_kind.default') : context.tr('journey_kind.flight')}',
            onTap: () {
              setState(() => _journeyKind = value);
            },
          );
        }).toList(),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final items = widget.parts
        .map(
          (part) => MultiJourneyImportListItem(
            keyValue: _dateKey(part),
            label: _dateKey(part),
            description: _description(part),
          ),
        )
        .toList();

    return MultiJourneyImportPage(
      title: context.tr('import.vector_multi.title'),
      items: items,
      selectedKeys: _selectedDates,
      listSectionTitle: context.tr(
        'import.journey_selection.list_section_title',
        args: ['${widget.parts.length}'],
      ),
      selectAllLabel: context.tr('import.journey_selection.select_all'),
      deselectAllLabel: context.tr('import.journey_selection.deselect_all'),
      confirmLabel: context.tr(
        'import.journey_selection.confirm_import',
        args: ['${_selectedDates.length}'],
      ),
      onToggleItem: (key, selected) async {
        setState(() {
          selected ? _selectedDates.add(key) : _selectedDates.remove(key);
        });
      },
      onToggleAll: (selectAll) async {
        setState(() {
          _selectedDates =
              selectAll ? widget.parts.map(_dateKey).toSet() : <String>{};
        });
      },
      onPreview: _openPreview,
      onConfirm: _confirmImport,
      confirmEnabled: !_isImporting,
      collapsibleHeader: MultiJourneyCollapsibleHeader.standard(
        expandedContent: Column(
          children: [
            LabelTile(
              label: context.tr('import.preprocessor.label'),
              trailing: LabelTileContent(
                content: _preprocessorLabel(_preprocessor),
                showArrow: true,
              ),
              onTap: _showPreprocessorPicker,
            ),
            LabelTile(
              label: context.tr('journey.journey_kind'),
              trailing: LabelTileContent(
                content: _journeyKind == JourneyKind.defaultKind
                    ? context.tr('journey_kind.default')
                    : context.tr('journey_kind.flight'),
                showArrow: true,
              ),
              onTap: _showJourneyKindPicker,
            ),
            LabelTile(
              label: context.tr('journey.note'),
              maxHeight: 100,
              trailing: SizedBox(
                width: MediaQuery.sizeOf(context).width * 0.55,
                child: TextField(
                  controller: _noteController,
                  keyboardType: TextInputType.multiline,
                  textInputAction: TextInputAction.newline,
                  maxLines: 2,
                  minLines: 1,
                  style: const TextStyle(
                    fontSize: 14,
                    color: Color(0x99FFFFFF),
                  ),
                  decoration: InputDecoration.collapsed(
                    border: InputBorder.none,
                    hintText: context.tr('common.please_enter'),
                    hintStyle: const TextStyle(
                      fontSize: 14,
                      color: Color(0x99FFFFFF),
                    ),
                  ),
                  textAlign: TextAlign.right,
                ),
              ),
            ),
          ],
        ),
        collapsedIcon: Icons.tune,
        collapsedText:
            '${_preprocessorLabel(_preprocessor)} · ${_journeyKind == JourneyKind.defaultKind ? context.tr('journey_kind.default') : context.tr('journey_kind.flight')}',
      ),
    );
  }
}
