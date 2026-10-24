import 'package:memolanes/theme/app_colors.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/journey_import_preview_page.dart';
import 'package:memolanes/common/component/multi_journey_import_page.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class MldxImportPage extends StatefulWidget {
  const MldxImportPage({
    super.key,
    required this.journeys,
    required this.mldxReader,
  });

  final List<MldxJourneyAnalysis> journeys;
  final OpaqueMldxReader mldxReader;

  @override
  State<MldxImportPage> createState() => _MldxImportPageState();
}

class _MldxImportPageState extends State<MldxImportPage> {
  late Set<String> _selectedIds;
  late final List<MldxJourneyAnalysis> _sortedJourneyWithoutIgnored;
  static final _lastModifiedFormat = DateFormat('yyyy-MM-dd');
  late final int _unchangedCount;
  late final int _conflictCount;

  @override
  void initState() {
    super.initState();
    // Conflict items are unchecked by default
    _unchangedCount = widget.journeys
        .where((j) => j.status == MldxJourneyImportAnalyzeResult.unchanged)
        .length;
    _conflictCount = widget.journeys
        .where((j) => j.status == MldxJourneyImportAnalyzeResult.conflict)
        .length;
    _selectedIds = widget.journeys
        .where((j) => j.status == MldxJourneyImportAnalyzeResult.new_)
        .map((j) => j.importedHeader.id)
        .toSet();
    _sortedJourneyWithoutIgnored = _sortJourneys(
      widget.journeys
          .where((j) => j.status != MldxJourneyImportAnalyzeResult.unchanged)
          .toList(),
    );
  }

  static List<MldxJourneyAnalysis> _sortJourneys(
    List<MldxJourneyAnalysis> list,
  ) {
    final result = list.toList();
    result.sort((a, b) {
      final aConflict = a.status == MldxJourneyImportAnalyzeResult.conflict;
      final bConflict = b.status == MldxJourneyImportAnalyzeResult.conflict;
      if (aConflict != bConflict) return aConflict ? -1 : 1;
      final aStr = a.importedHeader.journeyDate.toSimpleDate().toString();
      final bStr = b.importedHeader.journeyDate.toSimpleDate().toString();
      return aStr.compareTo(bStr);
    });
    return result;
  }

  String _journeyDateLabel(JourneyHeader h) {
    return h.journeyDate.toSimpleDate().toString();
  }

  DateTime _lastModifiedTime(JourneyHeader h) {
    return (h.updatedAt ?? h.createdAt).toLocal();
  }

  String _lastModifiedLabel(JourneyHeader h) {
    return _lastModifiedFormat.format(_lastModifiedTime(h));
  }

  Future<void> _toggleSelectAll(bool selectAll) async {
    if (!selectAll) {
      setState(() => _selectedIds.clear());
      return;
    }
    final hasConflict = _sortedJourneyWithoutIgnored.any(
      (j) => j.status == MldxJourneyImportAnalyzeResult.conflict,
    );
    if (hasConflict) {
      final ok = await showCommonDialog(
        context,
        context.tr('import.mldx_preview.conflict_force_confirm'),
        hasCancel: true,
        confirmButtonText: context.tr('common.ok'),
        cancelButtonText: context.tr('common.cancel'),
      );
      if (!ok || !mounted) return;
    }
    setState(() {
      _selectedIds = _sortedJourneyWithoutIgnored
          .map((j) => j.importedHeader.id)
          .toSet();
    });
  }

  MldxJourneyAnalysis _journeyForId(String id) => _sortedJourneyWithoutIgnored
      .firstWhere((journey) => journey.importedHeader.id == id);

  String _itemDesc(MldxJourneyAnalysis journey) {
    final importHeader = journey.importedHeader;
    final importLastModified = _lastModifiedLabel(importHeader);
    if (journey.status != MldxJourneyImportAnalyzeResult.conflict) {
      return context.tr(
        'import.mldx_preview.last_modified',
        args: [importLastModified],
      );
    }

    final localHeader = journey.localHeader;
    if (localHeader == null) {
      return context.tr('import.mldx_preview.conflict_desc_unknown');
    }
    final localT = _lastModifiedTime(localHeader);
    final importT = _lastModifiedTime(importHeader);
    if (importT.isAfter(localT)) {
      return context.tr('import.mldx_preview.conflict_desc_import_newer');
    }
    if (localT.isAfter(importT)) {
      return context.tr('import.mldx_preview.conflict_desc_local_newer');
    }
    return context.tr('import.mldx_preview.conflict_desc_same_time');
  }

  String _conflictHintText(BuildContext context, int conflictCount) {
    return context.tr(
      'import.mldx_preview.conflict_hint',
      args: ['$conflictCount'],
    );
  }

  Future<void> _openJourneyPreview(MldxJourneyAnalysis j) async {
    try {
      final loaded = await showLoadingDialog(
        asyncTask: widget.mldxReader.loadSingleJourney(
          journeyId: j.importedHeader.id,
        ),
      );
      if (!mounted) return;
      if (loaded == null) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
        return;
      }
      await Navigator.of(context).push(
        MaterialPageRoute(
          builder: (context) => JourneyImportPreviewPage(
            journeyHeader: loaded.$1,
            previewJourneyData: loaded.$2,
          ),
        ),
      );
    } catch (error, stackTrace) {
      log.error(
        '[MldxImportPage] open journey preview failed: $error',
        stackTrace,
      );
      if (!mounted) return;
      await showCommonDialog(context, context.tr('import.parsing_failed'));
    }
  }

  Future<void> _onToggleItem(MldxJourneyAnalysis j, bool newValue) async {
    final isConflict = j.status == MldxJourneyImportAnalyzeResult.conflict;
    if (newValue && isConflict) {
      final ok = await showCommonDialog(
        context,
        context.tr('import.mldx_preview.conflict_force_confirm'),
        hasCancel: true,
        confirmButtonText: context.tr('common.ok'),
        cancelButtonText: context.tr('common.cancel'),
      );
      if (!ok || !mounted) return;
    }
    setState(() {
      if (newValue) {
        _selectedIds.add(j.importedHeader.id);
      } else {
        _selectedIds.remove(j.importedHeader.id);
      }
    });
  }

  Future<void> _confirmImport() async {
    if (_selectedIds.isEmpty) {
      if (mounted) {
        await showCommonDialog(
          context,
          context.tr('import.journey_selection.select_at_least_one'),
        );
      }
      return;
    }
    final selected = _sortedJourneyWithoutIgnored
        .where((j) => _selectedIds.contains(j.importedHeader.id))
        .toList();
    try {
      await showLoadingDialog(
        asyncTask: widget.mldxReader.importJourneys(
          journeyIds: selected.map((j) => j.importedHeader.id).toSet(),
        ),
      );
      if (mounted) {
        await showCommonDialog(context, context.tr('import.successful'));
        if (!mounted) return;
        popCurrentRoute(context, true);
      }
    } catch (error, stackTrace) {
      log.error('[MldxImportPage] import journeys failed: $error', stackTrace);
      if (mounted) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final journey = _sortedJourneyWithoutIgnored;
    final newCount = journey.length - _conflictCount;
    return MultiJourneyImportPage(
      title: context.tr('import.mldx_preview.title'),
      items: journey.map((j) {
        final header = j.importedHeader;
        final isConflict = j.status == MldxJourneyImportAnalyzeResult.conflict;
        return MultiJourneyImportListItem(
          keyValue: header.id,
          label: _journeyDateLabel(header),
          description: _itemDesc(j),
          trailing: isConflict
              ? Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Flexible(
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Icon(
                            Icons.error_outline,
                            size: 30,
                            color: context.appColors.dangerColor,
                          ),
                          const SizedBox(height: 2),
                          Text(
                            context.tr('import.mldx_preview.conflict_label'),
                            style: Theme.of(context).textTheme.bodySmall
                                ?.copyWith(
                                  color: context.appColors.dangerColor,
                                ),
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    const LabelTileContent(showArrow: true),
                  ],
                )
              : null,
        );
      }).toList(),
      selectedKeys: _selectedIds,
      listSectionTitle: context.tr(
        'import.journey_selection.list_section_title',
        args: ['${journey.length}'],
      ),
      selectAllLabel: context.tr('import.journey_selection.select_all'),
      deselectAllLabel: context.tr('import.journey_selection.deselect_all'),
      confirmLabel: context.tr(
        'import.journey_selection.confirm_import',
        args: ['${_selectedIds.length}'],
      ),
      onToggleItem: (id, selected) =>
          _onToggleItem(_journeyForId(id), selected),
      onToggleAll: _toggleSelectAll,
      onPreview: (id) => _openJourneyPreview(_journeyForId(id)),
      onConfirm: _confirmImport,
      collapsibleHeader: MultiJourneyCollapsibleHeader.standard(
        expandedHeight: 120,
        expandedContent: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (_unchangedCount > 0)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Text(
                  context.tr(
                    'import.mldx_preview.skipped_identical',
                    args: ['$_unchangedCount'],
                  ),
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
              ),
            if (_conflictCount > 0)
              Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: Text(
                  _conflictHintText(context, _conflictCount),
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
              ),
            Text(
              context.tr('import.mldx_preview.new_count', args: ['$newCount']),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ],
        ),
        collapsedIcon: Icons.summarize_outlined,
        collapsedText: context.tr(
          'import.mldx_preview.summary_counts',
          args: ['$newCount', '$_conflictCount', '$_unchangedCount'],
        ),
      ),
    );
  }
}
