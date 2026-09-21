import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:memolanes/common/component/app_option_tile.dart';
import 'package:memolanes/common/component/setup_bottom_sheet.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/src/rust/api/achievement.dart' as achievement;
import 'package:mutex/mutex.dart';

export 'package:memolanes/src/rust/api/achievement.dart' show Worldview;

const _worldviewDisplayOrder = [
  achievement.Worldview.iso,
  achievement.Worldview.chn,
  achievement.Worldview.usa,
];

class WorldviewManager {
  WorldviewManager._()
    : this.forTesting(
        readSavedWorldview: _loadSavedWorldview,
        hasConfirmedPreference: () =>
            // Version 1 already asked the user to confirm their preference.
            // Preserve it even when a later setup version needs to be shown.
            MMKVUtil.getInt(MMKVKey.firstLaunchSetupCompletedVersion) >= 1,
        readDeviceLocales: () =>
            WidgetsBinding.instance.platformDispatcher.locales,
        activateGeoData: _activateGeoData,
        persistWorldview: _persistWorldview,
      );

  @visibleForTesting
  WorldviewManager.forTesting({
    required this._readSavedWorldview,
    required this._hasConfirmedPreference,
    required this._readDeviceLocales,
    required Future<void> Function(achievement.Worldview) activateGeoData,
    required void Function(achievement.Worldview) persistWorldview,
  }) : _activate = activateGeoData,
       _persist = persistWorldview;

  static final WorldviewManager instance = WorldviewManager._();

  final Mutex _mutex = Mutex();
  final achievement.Worldview? Function() _readSavedWorldview;
  final bool Function() _hasConfirmedPreference;
  final List<Locale> Function() _readDeviceLocales;
  final Future<void> Function(achievement.Worldview) _activate;
  final void Function(achievement.Worldview) _persist;
  achievement.Worldview? _currentWorldview;
  achievement.Worldview? _confirmedWorldview;

  achievement.Worldview get currentWorldview =>
      _currentWorldview ??
      (throw StateError('WorldviewManager has not been initialized'));

  Future<void> initialize() {
    return _mutex.protect(() async {
      if (_currentWorldview != null) return;
      // Older builds persisted recommendations before setup was accepted.
      // Only a confirmed preference may override a fresh recommendation.
      final saved = _hasConfirmedPreference() ? _readSavedWorldview() : null;
      final worldview =
          saved ?? defaultWorldviewFromLocales(_readDeviceLocales());
      // TODO: right now we make sure the geo data is fully loaded during
      // app initialization, which can be a bit expensive. We should consider
      // delaying this.
      await _activate(worldview);
      _currentWorldview = worldview;
      _confirmedWorldview = saved;
    });
  }

  Future<void> update(achievement.Worldview worldview) {
    return _mutex.protect(() async {
      if (_currentWorldview == null) {
        throw StateError('WorldviewManager has not been initialized');
      }
      if (_currentWorldview != worldview) {
        await _activate(worldview);
        _currentWorldview = worldview;
      }
      // Accepting the recommendation is still an explicit confirmation, even
      // though the already-active geo data does not need to be loaded again.
      if (_confirmedWorldview != worldview) {
        try {
          _persist(worldview);
          _confirmedWorldview = worldview;
        } catch (error, stackTrace) {
          // Keep the UI in sync with the active data even if saving fails.
          // Allow a later confirmation to retry saving.
          _confirmedWorldview = null;
          log.error('Failed to save worldview preference: $error', stackTrace);
        }
      }
    });
  }

  static void _persistWorldview(achievement.Worldview worldview) {
    if (!MMKVUtil.putString(MMKVKey.worldviewPreference, worldview.id)) {
      throw StateError('Failed to save worldview preference');
    }
  }

  static Future<void> _activateGeoData(achievement.Worldview worldview) async {
    await achievement.activateGeoData(
      worldview: worldview,
      loadAsset: () async =>
          (await rootBundle.load(worldview.assetPath)).buffer.asUint8List(),
    );
  }

  static achievement.Worldview? _loadSavedWorldview() {
    final id = MMKVUtil.getStringOpt(MMKVKey.worldviewPreference);
    return id == null ? null : achievement.Worldview.fromId(id: id);
  }
}

/// A first-use recommendation, not a location or a confirmed user preference.
/// Skip locales without a region, but never skip an explicit region merely
/// because its worldview is ISO. Language/script alone cannot select a view.
achievement.Worldview defaultWorldviewFromLocales(Iterable<Locale> locales) {
  for (final locale in locales) {
    final region = locale.countryCode?.trim().toUpperCase();
    if (region == null || region.isEmpty) continue;
    return switch (region) {
      'CN' || 'HK' || 'MO' => achievement.Worldview.chn,
      'US' => achievement.Worldview.usa,
      // TW and all other regions retain the ISO recommendation. This is an
      // explicit preference policy, not derived from geo asset parentage.
      _ => achievement.Worldview.iso,
    };
  }
  return achievement.Worldview.iso;
}

String regionPreferenceTitle(
  BuildContext context,
  achievement.Worldview worldview,
) {
  return switch (worldview) {
    achievement.Worldview.chn => context.tr("privacy.region_china"),
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
  return showSetupCard<achievement.Worldview>(
    context,
    builder: (_) => _RegionPickerSheet(selectedWorldview: selectedWorldview),
  );
}

class _RegionPickerSheet extends StatelessWidget {
  const _RegionPickerSheet({required this.selectedWorldview});

  final achievement.Worldview selectedWorldview;

  @override
  Widget build(BuildContext context) {
    return SetupDialogCard(
      title: context.tr("privacy.region_title"),
      maxHeightFactor: 0.55,
      contentPadding: const EdgeInsets.fromLTRB(20, 14, 20, 10),
      child: Column(
        children: [
          for (var i = 0; i < _worldviewDisplayOrder.length; i++) ...[
            AppOptionTile(
              icon: regionPreferenceIcon(_worldviewDisplayOrder[i]),
              title: regionPreferenceTitle(context, _worldviewDisplayOrder[i]),
              selected: _worldviewDisplayOrder[i] == selectedWorldview,
              trailing: AppOptionTileTrailing.selection,
              onTap: () => Navigator.of(context).pop(_worldviewDisplayOrder[i]),
            ),
            if (i < _worldviewDisplayOrder.length - 1)
              const SizedBox(height: 8),
          ],
        ],
      ),
    );
  }
}
