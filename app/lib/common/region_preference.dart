import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/main_db.dart';

export 'package:memolanes/src/rust/main_db.dart' show RegionPreference;

RegionPreference defaultRegionPreferenceFromDeviceLocale() {
  final locales = WidgetsBinding.instance.platformDispatcher.locales;
  final countryCode =
      locales.isNotEmpty ? locales.first.countryCode?.toUpperCase() : null;

  return switch (countryCode) {
    'CN' => RegionPreference.mainlandChina,
    'US' => RegionPreference.unitedStates,
    _ => RegionPreference.international,
  };
}

Future<RegionPreference> loadRegionPreferenceOrDefault() async {
  return await api.getRegionPreference() ??
      defaultRegionPreferenceFromDeviceLocale();
}

Future<void> saveRegionPreference(RegionPreference region) {
  return api.setRegionPreference(region: region);
}

String regionPreferenceTitle(BuildContext context, RegionPreference region) {
  return switch (region) {
    RegionPreference.mainlandChina =>
      context.tr("privacy.region_mainland_china"),
    RegionPreference.international =>
      context.tr("privacy.region_international"),
    RegionPreference.unitedStates => context.tr("privacy.region_united_states"),
  };
}

IconData regionPreferenceIcon(RegionPreference region) {
  return switch (region) {
    RegionPreference.mainlandChina => Icons.location_on_outlined,
    RegionPreference.international => Icons.language,
    RegionPreference.unitedStates => Icons.account_balance_outlined,
  };
}

Future<RegionPreference?> showRegionPreferencePicker(
  BuildContext context, {
  required RegionPreference selectedRegion,
}) {
  return showModalBottomSheet<RegionPreference>(
    context: context,
    backgroundColor: Colors.transparent,
    isScrollControlled: true,
    builder: (context) {
      return _RegionPickerSheet(selectedRegion: selectedRegion);
    },
  );
}

class _RegionPickerSheet extends StatelessWidget {
  const _RegionPickerSheet({required this.selectedRegion});

  final RegionPreference selectedRegion;

  @override
  Widget build(BuildContext context) {
    return SetupBottomSheet(
      title: '',
      showTitle: false,
      maxHeightFactor: 0.55,
      contentPadding: const EdgeInsets.fromLTRB(20, 4, 20, 10),
      child: Column(
        children: [
          for (final region in RegionPreference.values)
            SetupTile(
              icon: regionPreferenceIcon(region),
              title: regionPreferenceTitle(context, region),
              selected: region == selectedRegion,
              onTap: () => Navigator.of(context).pop(region),
              contentPadding:
                  const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
              trailing: Icon(
                region == selectedRegion
                    ? Icons.check_circle
                    : Icons.circle_outlined,
                color: region == selectedRegion
                    ? StyleConstants.defaultColor
                    : const Color(0x99FFFFFF),
              ),
            ),
        ],
      ),
    );
  }
}
