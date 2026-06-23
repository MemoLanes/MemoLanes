import 'package:country_flags/country_flags.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/body/achievement/achievement_region_list_page.dart';
import 'package:memolanes/body/achievement/shared/achievement_common.dart';
import 'package:memolanes/common/component/cards/option_card.dart';

const _countryGold = Color(0xFFD4AF37);

class AchievementCountriesCard extends StatelessWidget {
  const AchievementCountriesCard({super.key});

  static const _countries = [
    _ExploredCountry(
      code: 'CN',
      nameKey: 'achievement.countries.names.cn',
      provinces: [
        AchievementRegionItem(
          name: '浙江',
          children: [
            AchievementRegionItem(name: '杭州'),
            AchievementRegionItem(name: '宁波'),
            AchievementRegionItem(name: '绍兴'),
          ],
        ),
        AchievementRegionItem(
          name: '江苏',
          children: [
            AchievementRegionItem(name: '南京'),
            AchievementRegionItem(name: '苏州'),
            AchievementRegionItem(name: '无锡'),
          ],
        ),
        AchievementRegionItem(
          name: '广东',
          children: [
            AchievementRegionItem(name: '广州'),
            AchievementRegionItem(name: '深圳'),
            AchievementRegionItem(name: '珠海'),
          ],
        ),
        AchievementRegionItem(
          name: '四川',
          children: [
            AchievementRegionItem(name: '成都'),
            AchievementRegionItem(name: '绵阳'),
            AchievementRegionItem(name: '乐山'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'JP',
      nameKey: 'achievement.countries.names.jp',
      provinces: [
        AchievementRegionItem(
          name: '东京都',
          children: [
            AchievementRegionItem(name: '新宿'),
            AchievementRegionItem(name: '涩谷'),
            AchievementRegionItem(name: '台东'),
          ],
        ),
        AchievementRegionItem(
          name: '大阪府',
          children: [
            AchievementRegionItem(name: '大阪市'),
            AchievementRegionItem(name: '堺市'),
            AchievementRegionItem(name: '吹田市'),
          ],
        ),
        AchievementRegionItem(
          name: '京都府',
          children: [
            AchievementRegionItem(name: '京都市'),
            AchievementRegionItem(name: '宇治市'),
            AchievementRegionItem(name: '舞鹤市'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'US',
      nameKey: 'achievement.countries.names.us',
      provinces: [
        AchievementRegionItem(
          name: 'California',
          children: [
            AchievementRegionItem(name: 'San Francisco'),
            AchievementRegionItem(name: 'Los Angeles'),
            AchievementRegionItem(name: 'San Diego'),
          ],
        ),
        AchievementRegionItem(
          name: 'New York',
          children: [
            AchievementRegionItem(name: 'New York City'),
            AchievementRegionItem(name: 'Buffalo'),
            AchievementRegionItem(name: 'Albany'),
          ],
        ),
        AchievementRegionItem(
          name: 'Washington',
          children: [
            AchievementRegionItem(name: 'Seattle'),
            AchievementRegionItem(name: 'Bellevue'),
            AchievementRegionItem(name: 'Tacoma'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'GB',
      nameKey: 'achievement.countries.names.gb',
      provinces: [
        AchievementRegionItem(
          name: 'England',
          children: [
            AchievementRegionItem(name: 'London'),
            AchievementRegionItem(name: 'Manchester'),
            AchievementRegionItem(name: 'Oxford'),
          ],
        ),
        AchievementRegionItem(
          name: 'Scotland',
          children: [
            AchievementRegionItem(name: 'Edinburgh'),
            AchievementRegionItem(name: 'Glasgow'),
            AchievementRegionItem(name: 'Aberdeen'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'FR',
      nameKey: 'achievement.countries.names.fr',
      provinces: [
        AchievementRegionItem(
          name: 'Île-de-France',
          children: [
            AchievementRegionItem(name: 'Paris'),
            AchievementRegionItem(name: 'Versailles'),
            AchievementRegionItem(name: 'Saint-Denis'),
          ],
        ),
        AchievementRegionItem(
          name: 'Provence-Alpes-Côte d’Azur',
          children: [
            AchievementRegionItem(name: 'Marseille'),
            AchievementRegionItem(name: 'Nice'),
            AchievementRegionItem(name: 'Avignon'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'DE',
      nameKey: 'achievement.countries.names.de',
      provinces: [
        AchievementRegionItem(
          name: 'Bavaria',
          children: [
            AchievementRegionItem(name: 'Munich'),
            AchievementRegionItem(name: 'Nuremberg'),
            AchievementRegionItem(name: 'Augsburg'),
          ],
        ),
        AchievementRegionItem(
          name: 'Berlin',
          children: [
            AchievementRegionItem(name: 'Mitte'),
            AchievementRegionItem(name: 'Kreuzberg'),
            AchievementRegionItem(name: 'Charlottenburg'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'IT',
      nameKey: 'achievement.countries.names.it',
      provinces: [
        AchievementRegionItem(
          name: 'Lazio',
          children: [
            AchievementRegionItem(name: 'Rome'),
            AchievementRegionItem(name: 'Tivoli'),
            AchievementRegionItem(name: 'Viterbo'),
          ],
        ),
        AchievementRegionItem(
          name: 'Tuscany',
          children: [
            AchievementRegionItem(name: 'Florence'),
            AchievementRegionItem(name: 'Pisa'),
            AchievementRegionItem(name: 'Siena'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'ES',
      nameKey: 'achievement.countries.names.es',
      provinces: [
        AchievementRegionItem(
          name: 'Catalonia',
          children: [
            AchievementRegionItem(name: 'Barcelona'),
            AchievementRegionItem(name: 'Girona'),
            AchievementRegionItem(name: 'Tarragona'),
          ],
        ),
        AchievementRegionItem(
          name: 'Madrid',
          children: [
            AchievementRegionItem(name: 'Madrid'),
            AchievementRegionItem(name: 'Alcalá de Henares'),
            AchievementRegionItem(name: 'Getafe'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'PT',
      nameKey: 'achievement.countries.names.pt',
      provinces: [
        AchievementRegionItem(
          name: 'Lisbon',
          children: [
            AchievementRegionItem(name: 'Lisbon'),
            AchievementRegionItem(name: 'Sintra'),
            AchievementRegionItem(name: 'Cascais'),
          ],
        ),
        AchievementRegionItem(
          name: 'Porto',
          children: [
            AchievementRegionItem(name: 'Porto'),
            AchievementRegionItem(name: 'Vila Nova de Gaia'),
            AchievementRegionItem(name: 'Matosinhos'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'NL',
      nameKey: 'achievement.countries.names.nl',
      provinces: [
        AchievementRegionItem(
          name: 'North Holland',
          children: [
            AchievementRegionItem(name: 'Amsterdam'),
            AchievementRegionItem(name: 'Haarlem'),
            AchievementRegionItem(name: 'Alkmaar'),
          ],
        ),
        AchievementRegionItem(
          name: 'South Holland',
          children: [
            AchievementRegionItem(name: 'Rotterdam'),
            AchievementRegionItem(name: 'The Hague'),
            AchievementRegionItem(name: 'Leiden'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'CH',
      nameKey: 'achievement.countries.names.ch',
      provinces: [
        AchievementRegionItem(
          name: 'Zürich',
          children: [
            AchievementRegionItem(name: 'Zürich'),
            AchievementRegionItem(name: 'Winterthur'),
            AchievementRegionItem(name: 'Uster'),
          ],
        ),
        AchievementRegionItem(
          name: 'Geneva',
          children: [
            AchievementRegionItem(name: 'Geneva'),
            AchievementRegionItem(name: 'Carouge'),
            AchievementRegionItem(name: 'Meyrin'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'AT',
      nameKey: 'achievement.countries.names.at',
      provinces: [
        AchievementRegionItem(
          name: 'Vienna',
          children: [
            AchievementRegionItem(name: 'Innere Stadt'),
            AchievementRegionItem(name: 'Leopoldstadt'),
            AchievementRegionItem(name: 'Landstraße'),
          ],
        ),
        AchievementRegionItem(
          name: 'Salzburg',
          children: [
            AchievementRegionItem(name: 'Salzburg'),
            AchievementRegionItem(name: 'Hallein'),
            AchievementRegionItem(name: 'Saalfelden'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'TH',
      nameKey: 'achievement.countries.names.th',
      provinces: [
        AchievementRegionItem(
          name: 'Bangkok',
          children: [
            AchievementRegionItem(name: 'Phra Nakhon'),
            AchievementRegionItem(name: 'Pathum Wan'),
            AchievementRegionItem(name: 'Sathorn'),
          ],
        ),
        AchievementRegionItem(
          name: 'Chiang Mai',
          children: [
            AchievementRegionItem(name: 'Chiang Mai'),
            AchievementRegionItem(name: 'Mae Rim'),
            AchievementRegionItem(name: 'Hang Dong'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'SG',
      nameKey: 'achievement.countries.names.sg',
      provinces: [
        AchievementRegionItem(
          name: 'Central Region',
          children: [
            AchievementRegionItem(name: 'Marina Bay'),
            AchievementRegionItem(name: 'Orchard'),
            AchievementRegionItem(name: 'Chinatown'),
          ],
        ),
        AchievementRegionItem(
          name: 'East Region',
          children: [
            AchievementRegionItem(name: 'Changi'),
            AchievementRegionItem(name: 'Tampines'),
            AchievementRegionItem(name: 'Pasir Ris'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'AU',
      nameKey: 'achievement.countries.names.au',
      provinces: [
        AchievementRegionItem(
          name: 'New South Wales',
          children: [
            AchievementRegionItem(name: 'Sydney'),
            AchievementRegionItem(name: 'Newcastle'),
            AchievementRegionItem(name: 'Wollongong'),
          ],
        ),
        AchievementRegionItem(
          name: 'Victoria',
          children: [
            AchievementRegionItem(name: 'Melbourne'),
            AchievementRegionItem(name: 'Geelong'),
            AchievementRegionItem(name: 'Ballarat'),
          ],
        ),
      ],
    ),
    _ExploredCountry(
      code: 'CA',
      nameKey: 'achievement.countries.names.ca',
      provinces: [
        AchievementRegionItem(
          name: 'Ontario',
          children: [
            AchievementRegionItem(name: 'Toronto'),
            AchievementRegionItem(name: 'Ottawa'),
            AchievementRegionItem(name: 'Waterloo'),
          ],
        ),
        AchievementRegionItem(
          name: 'British Columbia',
          children: [
            AchievementRegionItem(name: 'Vancouver'),
            AchievementRegionItem(name: 'Victoria'),
            AchievementRegionItem(name: 'Burnaby'),
          ],
        ),
      ],
    ),
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
    required this.provinces,
  });

  final String code;
  final String nameKey;
  final List<AchievementRegionItem> provinces;
}

class _CountriesHeader extends StatelessWidget {
  const _CountriesHeader({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        AchievementCardTitleRow(
          title: context.tr('achievement.countries.title'),
          info: context.tr('achievement.countries.note'),
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
    final countryName = context.tr(country.nameKey);

    return InkWell(
      borderRadius: BorderRadius.circular(12),
      onTap: () {
        Navigator.push(
          context,
          MaterialPageRoute<void>(
            builder: (context) => AchievementRegionListPage(
              title: countryName,
              items: country.provinces,
              kind: AchievementRegionListKind.province,
              countryCode: country.code,
            ),
          ),
        );
      },
      child: Column(
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
            countryName,
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
      ),
    );
  }
}
