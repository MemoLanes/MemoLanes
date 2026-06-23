import 'package:country_flags/country_flags.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';

const _countryGold = Color(0xFFD4AF37);

class AchievementCountriesCard extends StatelessWidget {
  const AchievementCountriesCard({super.key});

  static const _countries = [
    _ExploredCountry(code: 'CN', nameKey: 'achievement.countries.names.cn'),
    _ExploredCountry(code: 'JP', nameKey: 'achievement.countries.names.jp'),
    _ExploredCountry(code: 'US', nameKey: 'achievement.countries.names.us'),
    _ExploredCountry(code: 'GB', nameKey: 'achievement.countries.names.gb'),
    _ExploredCountry(code: 'FR', nameKey: 'achievement.countries.names.fr'),
    _ExploredCountry(code: 'DE', nameKey: 'achievement.countries.names.de'),
    _ExploredCountry(code: 'IT', nameKey: 'achievement.countries.names.it'),
    _ExploredCountry(code: 'ES', nameKey: 'achievement.countries.names.es'),
    _ExploredCountry(code: 'PT', nameKey: 'achievement.countries.names.pt'),
    _ExploredCountry(code: 'NL', nameKey: 'achievement.countries.names.nl'),
    _ExploredCountry(code: 'CH', nameKey: 'achievement.countries.names.ch'),
    _ExploredCountry(code: 'AT', nameKey: 'achievement.countries.names.at'),
    _ExploredCountry(code: 'TH', nameKey: 'achievement.countries.names.th'),
    _ExploredCountry(code: 'SG', nameKey: 'achievement.countries.names.sg'),
    _ExploredCountry(code: 'AU', nameKey: 'achievement.countries.names.au'),
    _ExploredCountry(code: 'CA', nameKey: 'achievement.countries.names.ca'),
  ];

  @override
  Widget build(BuildContext context) {
    return OptionCard(
      children: [
        Padding(
          padding: achievementCardPadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _CountriesHeader(count: _countries.length),
              const SizedBox(height: 18),
              _CountriesGrid(countries: _countries),
            ],
          ),
        ),
      ],
    );
  }
}

class _ExploredCountry {
  const _ExploredCountry({
    required this.code,
    required this.nameKey,
  });

  final String code;
  final String nameKey;
}

class _CountriesHeader extends StatelessWidget {
  const _CountriesHeader({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          context.tr('achievement.countries.title'),
          style: const TextStyle(
            color: Colors.white,
            fontSize: 22,
            fontWeight: FontWeight.w800,
            height: 1,
          ),
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

  final List<_ExploredCountry> countries;

  @override
  Widget build(BuildContext context) {
    const columnCount = 5;
    const itemHeight = 67.0;
    final rows = <Widget>[];

    for (var start = 0; start < countries.length; start += columnCount) {
      final rowCountries = countries.skip(start).take(columnCount).toList();
      rows.add(
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final country in rowCountries)
              Expanded(
                child: SizedBox(
                  height: itemHeight,
                  child: _CountryFlagItem(country: country),
                ),
              ),
            for (var i = rowCountries.length; i < columnCount; i++)
              const Expanded(child: SizedBox(height: itemHeight)),
          ],
        ),
      );
      if (start + columnCount < countries.length) {
        rows.add(const SizedBox(height: 12));
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

  final _ExploredCountry country;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 42,
          height: 42,
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
            child: CountryFlag.fromCountryCode(
              country.code,
              theme: const ImageTheme(
                width: 36,
                height: 36,
                shape: Circle(),
              ),
            ),
          ),
        ),
        const SizedBox(height: 6),
        Text(
          context.tr(country.nameKey),
          textAlign: TextAlign.center,
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(
            color: Colors.white,
            fontSize: 11,
            fontWeight: FontWeight.w600,
            height: 1.15,
          ),
        ),
      ],
    );
  }
}
