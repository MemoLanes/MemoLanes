import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/app_checkbox.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/tiles/label_tile.dart';
import 'package:memolanes/common/component/tiles/label_tile_content.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

class MultiJourneyImportListItem {
  const MultiJourneyImportListItem({
    required this.keyValue,
    required this.label,
    required this.description,
    this.trailing,
  });

  final String keyValue;
  final String label;
  final String description;
  final Widget? trailing;
}

class MultiJourneyCollapsibleHeader {
  const MultiJourneyCollapsibleHeader({
    required this.expandedChild,
    required this.collapsedChild,
    this.expandedHeight = 220,
    this.collapsedHeight = 58,
  });

  factory MultiJourneyCollapsibleHeader.standard({
    required Widget expandedContent,
    required IconData collapsedIcon,
    required String collapsedText,
    double expandedHeight = 220,
    double collapsedHeight = 58,
  }) {
    return MultiJourneyCollapsibleHeader(
      expandedHeight: expandedHeight,
      collapsedHeight: collapsedHeight,
      expandedChild: Padding(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
        child: expandedContent,
      ),
      collapsedChild: _CollapsedImportHeader(
        icon: collapsedIcon,
        text: collapsedText,
      ),
    );
  }

  final Widget expandedChild;
  final Widget collapsedChild;
  final double expandedHeight;
  final double collapsedHeight;
}

class _CollapsedImportHeader extends StatelessWidget {
  const _CollapsedImportHeader({required this.icon, required this.text});

  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        decoration: BoxDecoration(
          color: StyleConstants.softGreen,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(color: StyleConstants.lineColor),
        ),
        child: Row(
          children: [
            Icon(icon, size: 18, color: StyleConstants.deepGreen),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                text,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppTypography.body.copyWith(
                  color: StyleConstants.mutedInkColor,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Shared selectable-list shell for importing multiple journeys.
///
/// Selection and import behavior stay controlled by the caller so archive
/// conflict handling and vector-track processing do not leak into each other.
class MultiJourneyImportPage extends StatelessWidget {
  const MultiJourneyImportPage({
    super.key,
    required this.title,
    required this.items,
    required this.selectedKeys,
    required this.listSectionTitle,
    required this.selectAllLabel,
    required this.deselectAllLabel,
    required this.confirmLabel,
    required this.onToggleItem,
    required this.onToggleAll,
    required this.onPreview,
    required this.onConfirm,
    required this.collapsibleHeader,
    this.confirmEnabled = true,
  });

  final String title;
  final List<MultiJourneyImportListItem> items;
  final Set<String> selectedKeys;
  final String listSectionTitle;
  final String selectAllLabel;
  final String deselectAllLabel;
  final String confirmLabel;
  final Future<void> Function(String key, bool selected) onToggleItem;
  final Future<void> Function(bool selectAll) onToggleAll;
  final Future<void> Function(String key) onPreview;
  final Future<void> Function() onConfirm;
  final bool confirmEnabled;
  final MultiJourneyCollapsibleHeader collapsibleHeader;

  bool get _allSelected =>
      items.isNotEmpty &&
      items.length == selectedKeys.length &&
      items.every((item) => selectedKeys.contains(item.keyValue));

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: StyleConstants.canvasColor,
      appBar: CapsuleStyleAppBar(title: title, showTitleBackground: false),
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(child: _buildCollapsibleList(context, collapsibleHeader)),
          SafeArea(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: AppButton(
                onPressed: confirmEnabled ? onConfirm : null,
                label: confirmLabel,
                expand: true,
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildCollapsibleList(
    BuildContext context,
    MultiJourneyCollapsibleHeader collapsibleHeader,
  ) {
    return CustomScrollView(
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      slivers: [
        SliverPersistentHeader(
          pinned: true,
          delegate: _CollapsibleHeaderDelegate(config: collapsibleHeader),
        ),
        if (items.isNotEmpty)
          SliverToBoxAdapter(child: _buildListSectionHeader(context)),
        SliverPadding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          sliver: SliverList.builder(
            itemCount: items.length,
            itemBuilder: (context, index) => _buildItem(index),
          ),
        ),
      ],
    );
  }

  Widget _buildListSectionHeader(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
      child: Row(
        children: [
          Text(listSectionTitle, style: Theme.of(context).textTheme.titleSmall),
          const Spacer(),
          TextButton.icon(
            onPressed: () => onToggleAll(!_allSelected),
            icon: AppCheckbox(
              value: _allSelected,
              onChanged: (value) => onToggleAll(value),
            ),
            label: Text(_allSelected ? deselectAllLabel : selectAllLabel),
          ),
        ],
      ),
    );
  }

  Widget _buildItem(int index) {
    final item = items[index];
    final selected = selectedKeys.contains(item.keyValue);
    return LabelTile(
      label: item.label,
      desc: item.description,
      prefix: AppCheckbox(
        value: selected,
        onChanged: (value) {
          onToggleItem(item.keyValue, value);
        },
      ),
      trailing: item.trailing ?? const LabelTileContent(showArrow: true),
      onTap: () => onPreview(item.keyValue),
    );
  }
}

class _CollapsibleHeaderDelegate extends SliverPersistentHeaderDelegate {
  const _CollapsibleHeaderDelegate({required this.config});

  final MultiJourneyCollapsibleHeader config;

  @override
  double get minExtent => config.collapsedHeight;

  @override
  double get maxExtent => config.expandedHeight;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    final range = maxExtent - minExtent;
    final progress = range == 0 ? 1.0 : (shrinkOffset / range).clamp(0.0, 1.0);
    final expandedOpacity = (1 - progress * 2).clamp(0.0, 1.0);
    final collapsedOpacity = ((progress - 0.5) * 2).clamp(0.0, 1.0);

    return Material(
      color: Theme.of(context).scaffoldBackgroundColor,
      elevation: overlapsContent ? 1 : 0,
      child: ClipRect(
        child: Stack(
          fit: StackFit.expand,
          children: [
            Positioned(
              top: 0,
              left: 0,
              right: 0,
              height: maxExtent,
              child: IgnorePointer(
                ignoring: progress > 0.4,
                child: Opacity(
                  opacity: expandedOpacity,
                  child: config.expandedChild,
                ),
              ),
            ),
            Positioned.fill(
              child: IgnorePointer(
                ignoring: progress < 0.6,
                child: Opacity(
                  opacity: collapsedOpacity,
                  child: config.collapsedChild,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  bool shouldRebuild(covariant _CollapsibleHeaderDelegate oldDelegate) =>
      oldDelegate.config != config;
}
