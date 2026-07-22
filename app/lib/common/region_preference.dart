import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement;
import 'package:mutex/mutex.dart';

export 'package:memolanes/src/rust/api/achievement.dart' show Worldview;

const _worldviewDisplayOrder = [
  achievement.Worldview.iso,
  achievement.Worldview.chn,
  achievement.Worldview.usa,
];

class WorldviewManager {
  WorldviewManager._();

  static final WorldviewManager instance = WorldviewManager._();

  final Mutex _mutex = Mutex();
  achievement.Worldview? _currentWorldview;

  achievement.Worldview get currentWorldview =>
      _currentWorldview ??
      (throw StateError('WorldviewManager has not been initialized'));

  Future<void> initialize() {
    return _mutex.protect(() async {
      if (_currentWorldview != null) return;
      final saved = _loadSavedWorldview();
      final worldview = saved ?? _defaultWorldviewFromDeviceLocale();
      // TODO: right now we make sure the geo data is fully loaded during
      // app initialization, which can be a bit expensive. We should consider
      // delaying this.
      await _applyAndStore(worldview, persist: saved == null);
    });
  }

  Future<void> update(achievement.Worldview worldview) {
    return _mutex.protect(() async {
      if (_currentWorldview == null) {
        throw StateError('WorldviewManager has not been initialized');
      }
      if (_currentWorldview == worldview) return;

      await _applyAndStore(worldview, persist: true);
    });
  }

  Future<void> _applyAndStore(
    achievement.Worldview worldview, {
    required bool persist,
  }) async {
    final data = await rootBundle.load(worldview.assetPath);
    await achievement.initOrChangeGeoData(
      worldview: worldview,
      geoData: data.buffer.asUint8List(),
    );
    if (persist) {
      MMKVUtil.putString(MMKVKey.worldviewPreference, worldview.id);
    }
    _currentWorldview = worldview;
  }

  achievement.Worldview? _loadSavedWorldview() {
    final id = MMKVUtil.getStringOpt(MMKVKey.worldviewPreference);
    return id == null ? null : achievement.Worldview.fromId(id: id);
  }

  achievement.Worldview _defaultWorldviewFromDeviceLocale() {
    final locales = WidgetsBinding.instance.platformDispatcher.locales;
    final countryCode =
        locales.isNotEmpty ? locales.first.countryCode?.toUpperCase() : null;

    return switch (countryCode) {
      'CN' => achievement.Worldview.chn,
      'US' => achievement.Worldview.usa,
      _ => achievement.Worldview.iso,
    };
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
