import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/import.dart';
import 'package:memolanes/src/rust/api/utils.dart';
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
    _sortedJourneyWithoutIgnored = _sortJourneys(widget.journeys
        .where((j) => j.$2 != MldxJourneyImportAnalyzeResult.unchanged)
        .toList());
    _loadLocalHeadersForConflicts();
  }

  static List<(JourneyHeader, MldxJourneyImportAnalyzeResult)> _sortJourneys(
    List<(JourneyHeader, MldxJourneyImportAnalyzeResult)> list,
  ) {
    final result =
        List<(JourneyHeader, MldxJourneyImportAnalyzeResult)>.from(list);
    result.sort((a, b) {
      final aConflict = a.$2 == MldxJourneyImportAnalyzeResult.conflict;
      final bConflict = b.$2 == MldxJourneyImportAnalyzeResult.conflict;
      if (aConflict != bConflict) return aConflict ? -1 : 1;
      final aStr = naiveDateToString(date: a.$1.journeyDate);
      final bStr = naiveDateToString(date: b.$1.journeyDate);
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
    return naiveDateToString(date: h.journeyDate);
  }

  DateTime _lastModifiedTime(JourneyHeader h) {
    return (h.updatedAt ?? h.createdAt).toLocal();
  }

  String _lastModifiedLabel(JourneyHeader h) {
    return _lastModifiedFormat.format(_lastModifiedTime(h));
  }

  bool get _allSelected {
    final newIds = _sortedJourneyWithoutIgnored.map((j) => j.$1.id).toSet();
    return newIds.length == _selectedIds.length &&
        newIds.every(_selectedIds.contains);
  }

  Future<void> _toggleSelectAll() async {
    if (_allSelected) {
      setState(() => _selectedIds.clear());
      return;
    }
    final hasConflict = _sortedJourneyWithoutIgnored
        .any((j) => j.$2 == MldxJourneyImportAnalyzeResult.conflict);
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
      (JourneyHeader, MldxJourneyImportAnalyzeResult) j) async {
    try {
      final loaded = await showLoadingDialog(
        asyncTask: widget.mldxReader.loadSingleJourney(
          journeyId: j.$1.id,
        ),
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
    } catch (_) {
      if (!mounted) return;
      await showCommonDialog(context, context.tr('import.parsing_failed'));
    }
  }

  Future<void> _onToggleItem(
      (JourneyHeader, MldxJourneyImportAnalyzeResult) j, bool newValue) async {
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
            context, context.tr('import.mldx_preview.select_at_least_one'));
      }
      return;
    }
    final selected = _sortedJourneyWithoutIgnored
        .where((j) => _selectedIds.contains(j.$1.id))
        .toList();
    final navigator = Navigator.of(context);
    try {
      await showLoadingDialog(
        asyncTask: widget.mldxReader.importJourneys(
          journeyIds: selected.map((j) => j.$1.id).toSet(),
        ),
      );
      if (mounted) {
        await showCommonDialog(context, context.tr('import.successful'));
        navigator.pop(true);
      }
    } catch (_) {
      if (mounted) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final journey = _sortedJourneyWithoutIgnored;

    return Scaffold(
      appBar: CapsuleStyleAppBar(
        title: context.tr('import.mldx_preview.title'),
      ),
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Column(
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
                if (_conflictCount > 0) ...[
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      _conflictHintText(context, _conflictCount),
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ),
                ],
                Text(
                  context.tr(
                    'import.mldx_preview.new_count',
                    args: ['${journey.length - _conflictCount}'],
                  ),
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
              ],
            ),
          ),
          if (journey.isNotEmpty) ...[
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
              child: Row(
                children: [
                  Text(
                    context.tr('import.mldx_preview.list_section_title'),
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                  const Spacer(),
                  TextButton.icon(
                    onPressed: () async => await _toggleSelectAll(),
                    icon: Icon(
                      _allSelected
                          ? Icons.check_box
                          : Icons.check_box_outline_blank,
                      size: 20,
                    ),
                    label: Text(
                      _allSelected
                          ? context.tr('import.mldx_preview.deselect_all')
                          : context.tr('import.mldx_preview.select_all'),
                    ),
                  ),
                ],
              ),
            ),
          ],
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              itemCount: journey.length,
              itemBuilder: (context, index) {
                final j = journey[index];
                final header = j.$1;
                final isConflict =
                    j.$2 == MldxJourneyImportAnalyzeResult.conflict;
                final selected = _selectedIds.contains(header.id);
                return LabelTile(
                  label: _journeyDateLabel(header),
                  desc: _itemDesc(header, isConflict),
                  prefix: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onTap: () => _onToggleItem(j, !selected),
                    child: Checkbox(
                      value: selected,
                      onChanged: (v) {
                        if (v != null) _onToggleItem(j, v);
                      },
                    ),
                  ),
                  trailing: isConflict
                      ? Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Column(
                              mainAxisSize: MainAxisSize.min,
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                const Icon(
                                  Icons.error_outline,
                                  size: 30,
                                  color: Colors.red,
                                ),
                                const SizedBox(height: 2),
                                Text(
                                  context
                                      .tr('import.mldx_preview.conflict_label'),
                                  style: Theme.of(context)
                                      .textTheme
                                      .bodySmall
                                      ?.copyWith(
                                        color: Colors.red,
                                      ),
                                ),
                              ],
                            ),
                            const SizedBox(width: 8),
                            const LabelTileContent(showArrow: true),
                          ],
                        )
                      : const LabelTileContent(showArrow: true),
                  onTap: () => _openJourneyPreview(j),
                );
              },
            ),
          ),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: FilledButton(
                onPressed: _confirmImport,
                child: Text(context.tr('import.mldx_preview.confirm_import')),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
