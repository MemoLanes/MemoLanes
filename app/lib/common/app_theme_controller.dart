import 'package:flutter/material.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/constants/style_constants.dart';

enum AppThemePreference {
  system('system'),
  light('light'),
  dark('dark');

  const AppThemePreference(this.id);

  final String id;

  static AppThemePreference fromId(String? id) {
    return values.firstWhere(
      (preference) => preference.id == id,
      orElse: () => AppThemePreference.dark,
    );
  }
}

class AppThemeController extends ChangeNotifier with WidgetsBindingObserver {
  AppThemeController()
      : _preference = AppThemePreference.fromId(
          MMKVUtil.getStringOpt(MMKVKey.interfaceThemeMode),
        ) {
    WidgetsBinding.instance.addObserver(this);
    _applyPalette();
  }

  AppThemePreference _preference;

  AppThemePreference get preference => _preference;

  Brightness get brightness => switch (_preference) {
        AppThemePreference.system =>
          WidgetsBinding.instance.platformDispatcher.platformBrightness,
        AppThemePreference.light => Brightness.light,
        AppThemePreference.dark => Brightness.dark,
      };

  void setPreference(AppThemePreference preference) {
    if (_preference == preference) return;
    _preference = preference;
    MMKVUtil.putString(MMKVKey.interfaceThemeMode, preference.id);
    _applyPalette();
    notifyListeners();
  }

  @override
  void didChangePlatformBrightness() {
    if (_preference != AppThemePreference.system) return;
    _applyPalette();
    notifyListeners();
  }

  void _applyPalette() {
    StyleConstants.setDarkMode(brightness == Brightness.dark);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }
}
