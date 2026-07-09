import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/capsule_style_app_bar.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/constants/style_constants.dart';

class AchievementRegionItem {
  const AchievementRegionItem({
    required this.name,
    this.description,
    this.children = const [],
  });

  final String name;
  final String? description;
  final List<AchievementRegionItem> children;
}

class AchievementRegionListPage extends StatelessWidget {
  const AchievementRegionListPage({
    super.key,
    required this.title,
    required this.items,
    required this.kind,
    required this.countryCode,
  });

  final String title;
  final List<AchievementRegionItem> items;
  final AchievementRegionListKind kind;
  final String countryCode;

  @override
  Widget build(BuildContext context) {
    final subtitleKey = switch (kind) {
      AchievementRegionListKind.province =>
        'achievement.region_list.province_count',
      AchievementRegionListKind.city => 'achievement.region_list.city_count',
    };

    return Scaffold(
      backgroundColor: const Color(0xFF0D0D0F),
      appBar: CapsuleStyleAppBar(title: title),
      body: SafeAreaWrapper(
        child: ListView(
          padding: const EdgeInsets.fromLTRB(
              0, 12, 0, StyleConstants.navBarSafeArea),
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
              child: Text(
                context.tr(subtitleKey, args: [items.length.toString()]),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.58),
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ),
            OptionCard(
              children: [
                for (var i = 0; i < items.length; i++)
                  _RegionListTile(
                    item: items[i],
                    kind: kind,
                    countryCode: countryCode,
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

enum AchievementRegionListKind {
  province,
  city,
}

class _RegionListTile extends StatelessWidget {
  const _RegionListTile({
    required this.item,
    required this.kind,
    required this.countryCode,
  });

  final AchievementRegionItem item;
  final AchievementRegionListKind kind;
  final String countryCode;

  @override
  Widget build(BuildContext context) {
    final children = item.children;
    final canOpen =
        kind == AchievementRegionListKind.province && children.isNotEmpty;
    final description = item.description ??
        switch (kind) {
          AchievementRegionListKind.province => context.tr(
              'achievement.region_list.city_count',
              args: [children.length.toString()],
            ),
          AchievementRegionListKind.city =>
            context.tr('achievement.region_list.city_unlocked'),
        };

    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: canOpen
            ? () {
                Navigator.push(
                  context,
                  MaterialPageRoute<void>(
                    builder: (context) => AchievementRegionListPage(
                      title: item.name,
                      items: children,
                      kind: AchievementRegionListKind.city,
                      countryCode: countryCode,
                    ),
                  ),
                );
              }
            : null,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(16, 14, 14, 14),
          child: Row(
            children: [
              _RegionFlagIcon(countryCode: countryCode),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      item.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 16,
                        fontWeight: FontWeight.w700,
                        height: 1.1,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      description,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: TextStyle(
                        color: Colors.white.withValues(alpha: 0.54),
                        fontSize: 13,
                        fontWeight: FontWeight.w500,
                        height: 1.1,
                      ),
                    ),
                  ],
                ),
              ),
              if (canOpen) ...[
                const SizedBox(width: 12),
                Icon(
                  Icons.chevron_right_rounded,
                  color: Colors.white.withValues(alpha: 0.45),
                  size: 22,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _RegionFlagIcon extends StatelessWidget {
  const _RegionFlagIcon({required this.countryCode});

  final String countryCode;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 34,
      height: 34,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: Colors.white.withValues(alpha: 0.06),
        border: Border.all(
          color: Colors.white.withValues(alpha: 0.08),
        ),
      ),
      child: SizedBox.square(
        dimension: 28,
        child: AchievementCountryFlag(
          countryCode: countryCode,
          size: 28,
        ),
      ),
    );
  }
}
