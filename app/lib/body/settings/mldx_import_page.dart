import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/journey/journey_info_page.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:memolanes/src/rust/archive.dart';
import 'package:memolanes/src/rust/journey_data.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class MldxImportPage extends StatefulWidget {
  const MldxImportPage({
    super.key,
    required this.preview,
  });

  final MldxImportPreview preview;

  @override
  State<MldxImportPage> createState() => _MldxImportPageState();
}

class _MldxImportPageState extends State<MldxImportPage> {
  late Set<String> _selectedIds;

  @override
  void initState() {
    super.initState();
    // Conflict items are unchecked by default
    _selectedIds =
        widget.preview.journey.where((j) => !j.$3).map((j) => j.$1.id).toSet();
  }

  String _journeyDateLabel(JourneyHeader h) {
    return naiveDateToString(date: h.journeyDate);
  }

  bool get _allSelected {
    final newIds = widget.preview.journey.map((j) => j.$1.id).toSet();
    return newIds.length == _selectedIds.length &&
        newIds.every(_selectedIds.contains);
  }

  Future<void> _toggleSelectAll() async {
    if (_allSelected) {
      setState(() => _selectedIds.clear());
      return;
    }
    final hasConflict = widget.preview.journey.any((j) => j.$3);
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
      _selectedIds = widget.preview.journey.map((j) => j.$1.id).toSet();
    });
  }

  String _itemDesc(JourneyHeader header, bool isConflict) {
    if (isConflict) {
      return '${header.revision} · ${context.tr('import.mldx_preview.conflict_label')}';
    }
    return header.revision;
  }

  String _conflictHintText(BuildContext context, MldxImportPreview preview) {
    return context
        .tr('import.mldx_preview.conflict_hint')
        .replaceAll('{}', '${preview.conflictCount}');
  }

  void _openJourneyPreview((JourneyHeader, JourneyData, bool) j) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => JourneyInfoPage(
          journeyHeader: j.$1,
          previewJourneyData: j.$2,
        ),
      ),
    );
  }

  Future<void> _onToggleItem(
      (JourneyHeader, JourneyData, bool) j, bool newValue) async {
    final isConflict = j.$3;
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
    final selected = widget.preview.journey
        .where((j) => _selectedIds.contains(j.$1.id))
        .toList();
    final navigator = Navigator.of(context);
    try {
      await showLoadingDialog(
        context: context,
        asyncTask: api.importJourneys(journeys: selected),
      );
      if (mounted) {
        await showCommonDialog(context, context.tr('import.successful'));
        navigator.pop(true);
      }
    } catch (e) {
      if (mounted) {
        await showCommonDialog(context, context.tr('import.parsing_failed'));
      }
    }
  }

  // Conflicts first, then the rest sorted by journey date
  List<(JourneyHeader, JourneyData, bool)> _sortedJourney() {
    final list =
        List<(JourneyHeader, JourneyData, bool)>.from(widget.preview.journey);
    list.sort((a, b) {
      final aConflict = a.$3;
      final bConflict = b.$3;
      if (aConflict != bConflict) return aConflict ? -1 : 1;
      final aStr = naiveDateToString(date: a.$1.journeyDate);
      final bStr = naiveDateToString(date: b.$1.journeyDate);
      return aStr.compareTo(bStr);
    });
    return list;
  }

  @override
  Widget build(BuildContext context) {
    final preview = widget.preview;
    final journey = _sortedJourney();

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
                if (preview.skippedCount > 0)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      context
                          .tr('import.mldx_preview.skipped_identical')
                          .replaceAll('{}', '${preview.skippedCount}'),
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ),
                if (preview.conflictCount > 0) ...[
                  Padding(
                    padding: const EdgeInsets.only(bottom: 8),
                    child: Text(
                      _conflictHintText(context, preview),
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ),
                ],
                Text(
                  context
                      .tr('import.mldx_preview.new_count')
                      .replaceAll('{}', '${journey.length}'),
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
                    onPressed: () => _toggleSelectAll(),
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
                final isConflict = j.$3;
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
                            const Icon(
                              Icons.error_outline,
                              size: 30,
                              color: Colors.red,
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
