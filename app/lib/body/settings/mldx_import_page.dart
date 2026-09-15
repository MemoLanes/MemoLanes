import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/multi_journey_import_page.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class MldxImportPage extends StatefulWidget {
  const MldxImportPage({
    super.key,
    required this.journeys,
    required this.mldxReader,
  });

  final List<(JourneyHeader, MldxJourneyImportAnalyzeResult)> journeys;
  final OpaqueMldxReader mldxReader;

  @override
  State<MldxImportPage> createState() => _MldxImportPageState();
}

class _MldxImportPageState extends State<MldxImportPage> {
  late Set<String> _selectedIds;
  late final List<(JourneyHeader, MldxJourneyImportAnalyzeResult)>
  _sortedJourneyWithoutIgnored;
  final Map<String, JourneyHeader> _localHeadersById = {};
  static final _lastModifiedFormat = DateFormat('yyyy-MM-dd');
  late final int _unchangedCount;
  late final int _conflictCount;

  @override
  void initState() {
    super.initState();
    // Conflict items are unchecked by default
    _unchangedCount = widget.journeys
        .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.unchanged)
        .length;
    _conflictCount = widget.journeys
        .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.conflict)
        .length;
    _selectedIds = widget.journeys
        .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.new_)
        .map((j) => j.$1.id)
        .toSet();
    _sortedJourneyWithoutIgnored = _sortJourneys(
      widget.journeys
          .where((j) => j.$2 != MldxJourneyImportAnalyzeResult.unchanged)
          .toList(),
    );
    _loadLocalHeadersForConflicts();
  }

  static List<(JourneyHeader, MldxJourneyImportAnalyzeResult)> _sortJourneys(
    List<(JourneyHeader, MldxJourneyImportAnalyzeResult)> list,
  ) {
    final result = List<(JourneyHeader, MldxJourneyImportAnalyzeResult)>.from(
      list,
    );
    result.sort((a, b) {
      final aConflict = a.$2 == MldxJourneyImportAnalyzeResult.conflict;
      final bConflict = b.$2 == MldxJourneyImportAnalyzeResult.conflict;
      if (aConflict != bConflict) return aConflict ? -1 : 1;
      final aStr = a.$1.journeyDate.toSimpleDate().toString();
      final bStr = b.$1.journeyDate.toSimpleDate().toString();
      return aStr.compareTo(bStr);
    });
    return result;
  }

  Future<void> _loadLocalHeadersForConflicts() async {
    final conflictIds = _sortedJourneyWithoutIgnored
        .where((j) => j.$2 == MldxJourneyImportAnalyzeResult.conflict)
        .map((j) => j.$1.id)
        .toSet();
    if (conflictIds.isEmpty) return;

    final futures = conflictIds.map((id) async {
      try {
        final local = await api.getJourneyHeader(journeyId: id);
        return MapEntry(id, local);
      } catch (e) {
        return MapEntry<String, JourneyHeader?>(id, null);
      }
    });
    final results = await Future.wait(futures);
    if (!mounted) return;
    final updates = <String, JourneyHeader>{};
    for (final entry in results) {
      if (entry.value != null) updates[entry.key] = entry.value!;
    }
    if (updates.isEmpty) return;
    setState(() => _localHeadersById.addAll(updates));
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
      (j) => j.$2 == MldxJourneyImportAnalyzeResult.conflict,
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
      _selectedIds = _sortedJourneyWithoutIgnored.map((j) => j.$1.id).toSet();
    });
  }

  (JourneyHeader, MldxJourneyImportAnalyzeResult) _journeyForId(String id) =>
      _sortedJourneyWithoutIgnored.firstWhere((journey) => journey.$1.id == id);

  String _itemDesc(JourneyHeader importHeader, bool isConflict) {
    final importLastModified = _lastModifiedLabel(importHeader);
    if (!isConflict) {
      return context.tr(
        'import.mldx_preview.last_modified',
        args: [importLastModified],
      );
    }

    final localHeader = _localHeadersById[importHeader.id];
    return () {
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
    }();
  }

  String _conflictHintText(BuildContext context, int conflictCount) {
    return context.tr(
      'import.mldx_preview.conflict_hint',
      args: ['$conflictCount'],
    );
  }

  Future<void> _openJourneyPreview(
    (JourneyHeader, MldxJourneyImportAnalyzeResult) j,
  ) async {
    try {
      final loaded = await showLoadingDialog(
        asyncTask: widget.mldxReader.loadSingleJourney(journeyId: j.$1.id),
      );
      if (!mounted) return;
      if (loaded == null) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
        return;
      }
      await Navigator.of(context).push(
        MaterialPageRoute(
          builder: (context) => JourneyInfoPage(
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

  Future<void> _onToggleItem(
    (JourneyHeader, MldxJourneyImportAnalyzeResult) j,
    bool newValue,
  ) async {
    final isConflict = j.$2 == MldxJourneyImportAnalyzeResult.conflict;
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
        _selectedIds.add(j.$1.id);
      } else {
        _selectedIds.remove(j.$1.id);
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
        .where((j) => _selectedIds.contains(j.$1.id))
        .toList();
    try {
      await showLoadingDialog(
        asyncTask: widget.mldxReader.importJourneys(
          journeyIds: selected.map((j) => j.$1.id).toSet(),
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
        final header = j.$1;
        final isConflict = j.$2 == MldxJourneyImportAnalyzeResult.conflict;
        return MultiJourneyImportListItem(
          keyValue: header.id,
          label: _journeyDateLabel(header),
          description: _itemDesc(header, isConflict),
          trailing: isConflict
              ? Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Column(
                      mainAxisSize: MainAxisSize.min,
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(
                          Icons.error_outline,
                          size: 30,
                          color: StyleConstants.dangerColor,
                        ),
                        const SizedBox(height: 2),
                        Text(
                          context.tr('import.mldx_preview.conflict_label'),
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(color: StyleConstants.dangerColor),
                        ),
                      ],
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
