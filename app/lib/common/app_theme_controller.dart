import 'package:flutter/material.dart';
import 'package:memolanes/common/mmkv_util.dart';

enum AppThemePreference {
  system('system'),
  light('light'),
  dark('dark');

  const AppThemePreference(this.id);

  final String id;

  static AppThemePreference fromId(String? id) {
    return values.firstWhere(
      (preference) => preference.id == id,
      orElse: () => AppThemePreference.system,
    );
  }
}

class AppThemeController extends ChangeNotifier {
  AppThemeController({
    String? Function()? readPreference,
    void Function(String)? writePreference,
  }) : _preference = AppThemePreference.fromId(
         (readPreference ?? _readPreference)(),
       ),
       _writePreference = writePreference ?? _savePreference;

  AppThemePreference _preference;
  final void Function(String) _writePreference;

  static String? _readPreference() =>
      MMKVUtil.getStringOpt(MMKVKey.interfaceThemeMode);

  static void _savePreference(String value) =>
      MMKVUtil.putString(MMKVKey.interfaceThemeMode, value);

  AppThemePreference get preference => _preference;

  ThemeMode get themeMode => switch (_preference) {
    AppThemePreference.system => ThemeMode.system,
    AppThemePreference.light => ThemeMode.light,
    AppThemePreference.dark => ThemeMode.dark,
  };

  void setPreference(AppThemePreference preference) {
    if (_preference == preference) return;
    _writePreference(preference.id);
    _preference = preference;
    notifyListeners();
  }
}
