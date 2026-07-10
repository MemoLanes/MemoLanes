import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/achievement_region_area_list_page.dart';
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:provider/provider.dart';

class AchievementCountryListPage extends StatelessWidget {
  const AchievementCountryListPage({super.key});

  @override
  Widget build(BuildContext context) {
    final countries = context.watch<AchievementStatsStore>().countries;
    final items = countries
        .map(
          (country) => AchievementRegionAreaListItem(
            name: context.tr(country.nameKey),
            visitedKm2: country.visitedKm2,
            totalKm2: country.totalKm2,
            flagCountryCode: country.isoCode,
            sortKey: country.isoCode,
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
