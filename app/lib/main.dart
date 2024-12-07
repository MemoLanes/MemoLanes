import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/time_machine.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/settings.dart';
import 'package:memolanes/gps_page.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/journey.dart';
import 'package:memolanes/map.dart';
import 'package:memolanes/raw_data.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:provider/provider.dart';
import 'package:flutter/services.dart';
import 'package:easy_localization/easy_localization.dart';
import 'dart:ui';

void delayedInit(UpdateNotifier updateNotifier) {
  Future.delayed(const Duration(milliseconds: 2000), () async {
    DeviceInfoPlugin deviceInfo = DeviceInfoPlugin();
    String? manufacturer;
    String? model;
    String? systemVersion;
    bool isPhysicalDevice = false;
    if (defaultTargetPlatform == TargetPlatform.android) {
      var androidInfo = await deviceInfo.androidInfo;
      manufacturer = androidInfo.manufacturer;
      model = androidInfo.model;
      systemVersion = androidInfo.version.release;
      isPhysicalDevice = androidInfo.isPhysicalDevice;
    } else if (defaultTargetPlatform == TargetPlatform.iOS) {
      var iosInfo = await deviceInfo.iosInfo;
      manufacturer = "Apple";
      model = iosInfo.utsname.machine;
      systemVersion = iosInfo.systemVersion;
      isPhysicalDevice = iosInfo.isPhysicalDevice;
    }

    PackageInfo packageInfo = await PackageInfo.fromPlatform();

    await api.delayedInit(
        deviceInfo: api.DeviceInfo(
            isPhysicalDevice: isPhysicalDevice,
            manufacturer: manufacturer,
            model: model,
            systemVersion: systemVersion),
        appInfo: api.AppInfo(
            packageName: packageInfo.packageName,
            version: packageInfo.version,
            buildNumber: packageInfo.buildNumber));
    doWork() async {
      // TODO: for future use
    }

    await doWork();
    Timer.periodic(const Duration(minutes: 10), (_) async {
      await api.tenMinutesHeartbeat();
      await doWork();
    });
  });
}

void main() async {
  // This is required since we are doing things before calling `runApp`.
  WidgetsFlutterBinding.ensureInitialized();
  await EasyLocalization.ensureInitialized();
  // TODO: Consider using `flutter_native_splash`
  await RustLib.init();
  await api.init(
      tempDir: (await getTemporaryDirectory()).path,
      docDir: (await getApplicationDocumentsDirectory()).path,
      supportDir: (await getApplicationSupportDirectory()).path,
      cacheDir: (await getApplicationCacheDirectory()).path);
  var updateNotifier = UpdateNotifier();
  delayedInit(updateNotifier);
  var gpsRecordingState = GpsRecordingState();
  runApp(EasyLocalization(
      supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
      path: 'assets/translations',
      fallbackLocale: const Locale('en', 'US'),
      saveLocale: false,
      child: MultiProvider(
        providers: [
          ChangeNotifierProvider(create: (context) => gpsRecordingState),
          ChangeNotifierProvider(create: (context) => updateNotifier),
        ],
        child: const MyApp(),
      )));
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  void _naiveLocaleSelection(BuildContext context) {
    // TODO: This naive version is good enough for now, as we only have two locales.
    // The one provided by the lib is kinda weird. e.g. It will map `zh-Hans-HK` to
    // `en-US` (I guess `Hans` + `HK` is a weird case).
    // Maybe related to: https://github.com/aissat/easy_localization/issues/372
    var deviceLocale = context.deviceLocale;
    var locale = const Locale('en', 'US');
    if (deviceLocale.languageCode == 'zh') {
      locale = const Locale('zh', 'CN');
    }
    context.setLocale(locale);
  }

  @override
  Widget build(BuildContext context) {
    _naiveLocaleSelection(context);
    return MaterialApp(
      title: 'MemoLanes',
      localizationsDelegates: context.localizationDelegates,
      supportedLocales: context.supportedLocales,
      locale: context.locale,
      theme: ThemeData(
        useMaterial3: true,
        fontFamily: 'MiSans',
        colorScheme: ColorScheme.fromSeed(
          seedColor: Colors.black,
          primary: Colors.black,
          secondary: Colors.black87,
          tertiary: Colors.black54,
          surface: Colors.white,
          onPrimary: Colors.white,
          onSecondary: Colors.white,
          onSurface: Colors.black87,
        ),
        iconTheme: const IconThemeData(
          color: Colors.black87,
        ),
        bottomNavigationBarTheme: const BottomNavigationBarThemeData(
          elevation: 8,
          backgroundColor: Colors.white,
          selectedItemColor: Colors.black,
          unselectedItemColor: Colors.black54,
        ),
      ),
      home: const MyHomePage(title: 'MemoLanes [OSS]'),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({Key? key, required this.title}) : super(key: key);
  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  int _selectedIndex = 0;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        bottom: false,
        child: Stack(
          children: [
            IndexedStack(
              index: _selectedIndex,
              children: const [
                MapUiBody(),
                TimeMachineUIBody(),
                JourneyUiBody(),
                SettingsBody(),
                RawDataBody(),
              ],
            ),
            const Positioned(
              left: 0,
              right: 0,
              bottom: 130,
              child: GPSPage(),
            ),
            Positioned(
              left: 32,
              right: 32,
              bottom: 32,
              child: ClipRRect(
                borderRadius: BorderRadius.circular(20),
                child: Container(
                  decoration: BoxDecoration(
                    color: Colors.white.withOpacity(0.8),
                    borderRadius: BorderRadius.circular(20),
                    boxShadow: [
                      BoxShadow(
                        color: Colors.black.withOpacity(0.1),
                        blurRadius: 8,
                        offset: const Offset(0, 2),
                      ),
                    ],
                  ),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(vertical: 10),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceAround,
                      children: [
                        _buildNavItem(0, Icons.map_outlined, Icons.map),
                        _buildNavItem(1, Icons.update_outlined, Icons.update),
                        _buildNavItem(2, Icons.route_outlined, Icons.route),
                        _buildNavItem(
                            3, Icons.settings_outlined, Icons.settings),
                        _buildNavItem(
                            4, Icons.data_array_outlined, Icons.data_array),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildNavItem(int index, IconData icon, IconData activeIcon) {
    final isSelected = _selectedIndex == index;

    return GestureDetector(
      onTap: () => setState(() => _selectedIndex = index),
      child: Container(
        padding: const EdgeInsets.all(12),
        decoration: isSelected
            ? BoxDecoration(
                color: Colors.white.withOpacity(0.5),
                borderRadius: BorderRadius.circular(10),
              )
            : null,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              isSelected ? activeIcon : icon,
              color: isSelected ? Theme.of(context).primaryColor : Colors.grey,
              size: 36,
            ),
          ],
        ),
      ),
    );
  }
}
