import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement;

export 'package:memolanes/src/rust/api/achievement.dart' show Worldview;

const _worldviewDisplayOrder = [
  achievement.Worldview.iso,
  achievement.Worldview.chn,
  achievement.Worldview.usa,
];

achievement.Worldview defaultWorldviewFromDeviceLocale() {
  final locales = WidgetsBinding.instance.platformDispatcher.locales;
  final countryCode =
      locales.isNotEmpty ? locales.first.countryCode?.toUpperCase() : null;

  return switch (countryCode) {
    'CN' => achievement.Worldview.chn,
    'US' => achievement.Worldview.usa,
    _ => achievement.Worldview.iso,
  };
}

Future<achievement.Worldview> loadWorldviewOrDefault() async {
  return await achievement.getWorldviewPreference() ??
      defaultWorldviewFromDeviceLocale();
}

Future<achievement.Worldview?> loadSavedWorldview() {
  return achievement.getWorldviewPreference();
}

Future<void> applyWorldview(achievement.Worldview worldview) async {
  final data = await rootBundle.load(worldview.assetPath);
  await achievement.initOrChangeGeoData(
    worldview: worldview,
    geoData: data.buffer.asUint8List(),
  );
}

Future<void> saveWorldview(achievement.Worldview worldview) {
  return achievement.setWorldviewPreference(worldview: worldview);
}

Future<void> applyAndSaveWorldview(achievement.Worldview worldview) async {
  await applyWorldview(worldview);
  await saveWorldview(worldview);
}

Future<void> ensureWorldviewReady() async {
  final saved = await loadSavedWorldview();
  final worldview = saved ?? defaultWorldviewFromDeviceLocale();

  await applyWorldview(worldview);

  if (saved == null) {
    await saveWorldview(worldview);
  }
}

String regionPreferenceTitle(
    BuildContext context, achievement.Worldview worldview) {
  return switch (worldview) {
    achievement.Worldview.chn => context.tr("privacy.region_mainland_china"),
    achievement.Worldview.iso => context.tr("privacy.region_international"),
    achievement.Worldview.usa => context.tr("privacy.region_united_states"),
  };
}

IconData regionPreferenceIcon(achievement.Worldview worldview) {
  return switch (worldview) {
    achievement.Worldview.chn => Icons.location_on_outlined,
    achievement.Worldview.iso => Icons.language,
    achievement.Worldview.usa => Icons.account_balance_outlined,
  };
}

Future<achievement.Worldview?> showWorldviewPicker(
  BuildContext context, {
  required achievement.Worldview selectedWorldview,
}) {
  return showModalBottomSheet<achievement.Worldview>(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    builder: (context) {
      return _RegionPickerSheet(selectedWorldview: selectedWorldview);
    },
  );
}

class _RegionPickerSheet extends StatelessWidget {
  const _RegionPickerSheet({required this.selectedWorldview});

  final achievement.Worldview selectedWorldview;

  @override
  Widget build(BuildContext context) {
    return SetupBottomSheet(
      title: '',
      showTitle: false,
      maxHeightFactor: 0.55,
      contentPadding: const EdgeInsets.fromLTRB(20, 4, 20, 10),
      child: Column(
        children: [
          for (final worldview in _worldviewDisplayOrder)
            SetupTile(
              icon: regionPreferenceIcon(worldview),
              title: regionPreferenceTitle(context, worldview),
              selected: worldview == selectedWorldview,
              onTap: () => Navigator.of(context).pop(worldview),
              contentPadding:
                  const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
              trailing: Icon(
                worldview == selectedWorldview
                    ? Icons.check_circle
                    : Icons.circle_outlined,
                color: worldview == selectedWorldview
                    ? StyleConstants.defaultColor
                    : const Color(0x99FFFFFF),
              ),
            ),
        ],
      ),
    );
  }
}
