import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_fields.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/multi_journey_import_page.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart' as import_api;
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
  late Set<SimpleDate> _selectedDates;
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

  SimpleDate _dateKey(import_api.VectorImportPartSummary part) =>
      SimpleDate.parse(part.journeyDate);

  import_api.VectorImportPartSummary _partForKey(SimpleDate key) =>
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

  Future<void> _openPreview(String key) async {
    final date = SimpleDate.parse(key);
    final part = _partForKey(date);
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
              journeyDate: date.toFrbNaiveDate(),
              createdAt: DateTime.now(),
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
          journeyDates: selectedParts
              .map((part) => _dateKey(part).toString())
              .toList(),
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

  @override
  Widget build(BuildContext context) {
    final items = widget.parts
        .map(
          (part) => MultiJourneyImportListItem(
            keyValue: _dateKey(part).toString(),
            label: _dateKey(part).toString(),
            description: _description(part),
          ),
        )
        .toList();

    return MultiJourneyImportPage(
      title: context.tr('import.vector_multi.title'),
      items: items,
      selectedKeys: _selectedDates.map((date) => date.toString()).toSet(),
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
        final date = SimpleDate.parse(key);
        setState(() {
          selected ? _selectedDates.add(date) : _selectedDates.remove(date);
        });
      },
      onToggleAll: (selectAll) async {
        setState(() {
          _selectedDates = selectAll
              ? widget.parts.map(_dateKey).toSet()
              : <SimpleDate>{};
        });
      },
      onPreview: _openPreview,
      onConfirm: _confirmImport,
      confirmEnabled: !_isImporting,
      collapsibleHeader: MultiJourneyCollapsibleHeader.standard(
        expandedContent: OptionCard(
          useSafeArea: false,
          separators: false,
          children: [
            ImportPreprocessorTile(
              value: _preprocessor,
              position: LabelTilePosition.top,
              onSelected: (value) => setState(() => _preprocessor = value),
            ),
            JourneyKindTile(
              value: _journeyKind,
              position: LabelTilePosition.middle,
              onSelected: (value) => setState(() => _journeyKind = value),
            ),
            JourneyNoteTile(
              controller: _noteController,
              maxHeight: 100,
              maxLines: 2,
              widthFactor: 0.55,
              position: LabelTilePosition.bottom,
              bottom: false,
            ),
          ],
        ),
        collapsedIcon: Icons.tune,
        collapsedText:
            '${importPreprocessorLabel(context, _preprocessor)} · ${journeyKindLabel(context, _journeyKind)}',
      ),
    );
  }
}
