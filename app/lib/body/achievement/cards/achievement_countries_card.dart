import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/achievement_country_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/region_preference.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:provider/provider.dart';

const _countryGold = Color(0xFFD4AF37);
const _countriesGridColumnCount = 5;
const _countryItemHeight = 75.0;
const _countryRowSpacing = 6.0;
const _countryItemVerticalPadding = 2.0;
const _countryFlagSize = 42.0;
const _countryNameTopSpacing = 4.0;
const _countryNameSlotHeight = 24.0;
const _countryNameWidthFactor = 0.78;

class AchievementCountriesCard extends StatelessWidget {
  const AchievementCountriesCard({super.key});

  @override
  Widget build(BuildContext context) {
    final store = context.watch<AchievementStatsStore>();

    if (!store.hasCountryStats) {
      if (store.isCountryStatsLoading) {
        return const _CountriesSkeletonCard();
      }
      if (store.countryStatsError != null) {
        return const _CountriesErrorCard();
      }
    }

    final countries = store.countries;
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _CountriesHeader(
                count: countries.length,
                hasCountries: countries.isNotEmpty,
              ),
              const SizedBox(height: 18),
              if (countries.isEmpty)
                const _CountriesEmptyState()
              else
                _CountriesGrid(countries: countries),
            ],
          ),
        ),
      ],
    );
  }
}

class _CountriesErrorCard extends StatelessWidget {
  const _CountriesErrorCard();

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              AchievementCardTitleRow(
                title: context.tr('achievement.countries.title'),
                info: context.tr('achievement.countries.note'),
              ),
              const SizedBox(height: 12),
              Text(
                context.tr('achievement.countries.error'),
                style: TextStyle(
                  color: Colors.white.withValues(alpha: 0.58),
                  fontSize: 14,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _CountriesHeader extends StatelessWidget {
  const _CountriesHeader({
    required this.count,
    required this.hasCountries,
  });

  final int count;
  final bool hasCountries;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Expanded(
              child: AchievementCardTitleRow(
                title: context.tr('achievement.countries.title'),
                info: context.tr('achievement.countries.note'),
              ),
            ),
            if (hasCountries) ...[
              const SizedBox(width: 8),
              TextButton.icon(
                onPressed: () {
                  navigatorPush(
                    context,
                    page: const AchievementCountryListPage(),
                  );
                },
                icon: const Icon(Icons.arrow_forward_rounded, size: 16),
                label: Text(
                  context.tr('achievement.country_list.view_all'),
                ),
                style: TextButton.styleFrom(
                  foregroundColor: _countryGold,
                  textStyle: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.w800,
                  ),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 6,
                  ),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  visualDensity: VisualDensity.compact,
                ),
              ),
            ],
          ],
        ),
        const SizedBox(height: 12),
        Text(
          context.tr(
            'achievement.countries.description',
            args: [count.toString()],
          ),
          style: TextStyle(
            color: Colors.white.withValues(alpha: 0.58),
            fontSize: 14,
            fontWeight: FontWeight.w500,
          ),
        ),
      ],
    );
  }
}

class _CountriesGrid extends StatelessWidget {
  const _CountriesGrid({required this.countries});

  final List<AchievementCountryStats> countries;

  @override
  Widget build(BuildContext context) {
    final rows = <Widget>[];

    for (var start = 0;
        start < countries.length;
        start += _countriesGridColumnCount) {
      final rowCountries =
          countries.skip(start).take(_countriesGridColumnCount).toList();
      rows.add(
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final country in rowCountries)
              Expanded(
                child: SizedBox(
                  height: _countryItemHeight,
                  child: _CountryFlagItem(country: country),
                ),
              ),
            for (var i = rowCountries.length;
                i < _countriesGridColumnCount;
                i++)
              const Expanded(child: SizedBox(height: _countryItemHeight)),
          ],
        ),
      );
      if (start + _countriesGridColumnCount < countries.length) {
        rows.add(const SizedBox(height: _countryRowSpacing));
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: rows,
    );
  }
}

class _CountryFlagItem extends StatelessWidget {
  const _CountryFlagItem({required this.country});

  final AchievementCountryStats country;

  @override
  Widget build(BuildContext context) {
    final countryName = country.entity.displayName(
      WorldviewManager.instance.currentWorldview.id,
    );

    // TODO: Re-enable country detail navigation after province/city data is wired.
    return Tooltip(
      message: countryName,
      child: SizedBox.expand(
        child: Padding(
          padding:
              const EdgeInsets.symmetric(vertical: _countryItemVerticalPadding),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Container(
                width: _countryFlagSize,
                height: _countryFlagSize,
                alignment: Alignment.center,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  gradient: const LinearGradient(
                    colors: [Color(0xFF2A2A2A), Color(0xFF1A1A1A)],
                    begin: Alignment.topLeft,
                    end: Alignment.bottomRight,
                  ),
                  boxShadow: [
                    BoxShadow(
                      color: _countryGold.withValues(alpha: 0.25),
                      blurRadius: 6,
                    ),
                  ],
                ),
                child: SizedBox.square(
                  dimension: 36,
                  child: AchievementCountryFlag(
                    countryCode: country.isoA3Eh ?? '',
                    size: 36,
                  ),
                ),
              ),
              const SizedBox(height: _countryNameTopSpacing),
              SizedBox(
                height: _countryNameSlotHeight,
                child: FractionallySizedBox(
                  widthFactor: _countryNameWidthFactor,
                  child: _CountryNameText(countryName),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _CountryNameText extends StatelessWidget {
  const _CountryNameText(this.name);

  final String name;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final fontSize = _fontSizeForWidth(name, constraints.maxWidth);
        return Text(
          name,
          textAlign: TextAlign.center,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: TextStyle(
            color: Colors.white,
            fontSize: fontSize,
            fontWeight: FontWeight.w600,
            height: 1.08,
          ),
        );
      },
    );
  }

  static double _fontSizeForWidth(String text, double maxWidth) {
    const maxFontSize = 11.0;
    const minFontSize = 8.0;
    if (maxWidth <= 0 || text.isEmpty) return maxFontSize;

    final longestWord = text
        .split(RegExp(r'\s+'))
        .where((word) => word.isNotEmpty)
        .fold<String>('', (longest, word) {
      return word.length > longest.length ? word : longest;
    });
    final probe = longestWord.isEmpty ? text : longestWord;

    final painter = TextPainter(
      text: TextSpan(
        text: probe,
        style: TextStyle(
          fontSize: maxFontSize,
          fontWeight: FontWeight.w600,
          height: 1.08,
        ),
      ),
      maxLines: 1,
      textDirection: ui.TextDirection.ltr,
    );
    painter.layout(maxWidth: double.infinity);

    if (painter.width <= maxWidth) return maxFontSize;
    return (maxFontSize * maxWidth / painter.width)
        .clamp(minFontSize, maxFontSize)
        .toDouble();
  }
}

class _CountriesEmptyState extends StatelessWidget {
  const _CountriesEmptyState();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 12),
      child: Text(
        context.tr('achievement.countries.empty'),
        textAlign: TextAlign.center,
        style: TextStyle(
          color: Colors.white.withValues(alpha: 0.46),
          fontSize: 13,
          fontWeight: FontWeight.w500,
          height: 1.35,
        ),
      ),
    );
  }
}

class _CountriesSkeletonCard extends StatelessWidget {
  const _CountriesSkeletonCard();

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const _SkeletonBlock(width: 104, height: 22),
              const SizedBox(height: 12),
              const _SkeletonBlock(width: 190, height: 14),
              const SizedBox(height: 18),
              for (var row = 0; row < 2; row++) ...[
                Row(
                  children: [
                    for (var i = 0; i < _countriesGridColumnCount; i++)
                      const Expanded(
                        child: SizedBox(
                          height: _countryItemHeight,
                          child: _CountrySkeletonItem(),
                        ),
                      ),
                  ],
                ),
                if (row == 0) const SizedBox(height: _countryRowSpacing),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

class _CountrySkeletonItem extends StatelessWidget {
  const _CountrySkeletonItem();

  @override
  Widget build(BuildContext context) {
    return const Padding(
      padding: EdgeInsets.symmetric(vertical: _countryItemVerticalPadding),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          _SkeletonBlock(
            width: _countryFlagSize,
            height: _countryFlagSize,
            radius: 999,
            alignment: Alignment.center,
          ),
          SizedBox(height: _countryNameTopSpacing),
          SizedBox(
            height: _countryNameSlotHeight,
            child: FractionallySizedBox(
              widthFactor: _countryNameWidthFactor,
              child: Center(
                child: _SkeletonBlock(
                  width: double.infinity,
                  height: 10,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _SkeletonBlock extends StatelessWidget {
  const _SkeletonBlock({
    required this.width,
    required this.height,
    this.radius = 6,
    this.alignment = Alignment.centerLeft,
  });

  final double width;
  final double height;
  final double radius;
  final Alignment alignment;

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: alignment,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: Colors.white.withValues(alpha: 0.075),
          borderRadius: BorderRadius.circular(radius),
        ),
        child: SizedBox(width: width, height: height),
      ),
    );
  }
}
