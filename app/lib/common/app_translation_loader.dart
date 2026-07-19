import 'dart:convert';
import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/services.dart' show rootBundle;

class AppTranslationLoader extends AssetLoader {
  const AppTranslationLoader();

  static const _geoDir = 'assets/geo';

  @override
  Future<Map<String, dynamic>> load(String path, Locale locale) async {
    final tag = '${locale.languageCode}-${locale.countryCode}';
    final ui = await _loadJson('$path/$tag.json');
    final region = await _loadJson('$_geoDir/region_names.$tag.json');
    return {...ui, ...region};
  }

  Future<Map<String, dynamic>> _loadJson(String assetPath) async =>
      jsonDecode(await rootBundle.loadString(assetPath)) as Map<String, dynamic>;
}
