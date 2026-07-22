import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:provider/provider.dart';

class AchievementCountryListPage extends StatelessWidget {
  const AchievementCountryListPage({super.key});

  @override
  Widget build(BuildContext context) {
    final countries = context.watch<AchievementStatsStore>().countries;
    final items = countries
        .map(
          (country) => AchievementRegionAreaListItem(
            name: country.entity.displayName(
              WorldviewManager.instance.currentWorldview.id,
            ),
            visitedKm2: country.visitedKm2,
            totalKm2: country.totalKm2,
            flagCountryCode: country.isoA3Eh,
            sortKey: country.isoA3Eh ?? '',
          ),
        )
        .toList();

    return AchievementRegionAreaListPage(
      title: context.tr('achievement.country_list.title'),
      emptyText: context.tr('achievement.countries.empty'),
      items: items,
    );
  }
}
