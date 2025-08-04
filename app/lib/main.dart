import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:memolanes/achievement.dart';
import 'package:memolanes/component/bottom_nav_bar.dart';
import 'package:memolanes/component/safe_area_wrapper.dart';
import 'package:memolanes/gps_manager.dart';
import 'package:memolanes/journey.dart';
import 'package:memolanes/logger.dart';
import 'package:memolanes/map.dart';
import 'package:memolanes/mmkv_util.dart';
import 'package:memolanes/settings.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:memolanes/time_machine.dart';
import 'package:memolanes/utils.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';

GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();
bool mainMapInitialized = false;

void delayedInit(UpdateNotifier updateNotifier) {
  Future.delayed(const Duration(milliseconds: 4000), () async {
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

    // Db optimization check
    const currentOptimizationCheckVersion = 1;
    final dbOptimizeCheck = MMKVUtil.getInt(MMKVKey.dbOptimizationCheck);
    if (dbOptimizeCheck < currentOptimizationCheckVersion) {
      if (await api.mainDbRequireOptimization()) {
        var context = navigatorKey.currentState?.context;
        if (context != null && context.mounted) {
          await showCommonDialog(
              context, context.tr("db_optimization.notification"));
        }
      } else {
        MMKVUtil.putInt(
            MMKVKey.dbOptimizationCheck, currentOptimizationCheckVersion);
      }
    }

    doRepeatWork() async {}

    await doRepeatWork();
    Timer.periodic(const Duration(minutes: 10), (_) async {
      await api.tenMinutesHeartbeat();
      await doRepeatWork();
    });
  });
}

void main() async {
  runZonedGuarded(() async {
    // This is required since we are doing things before calling `runApp`.
    WidgetsFlutterBinding.ensureInitialized();
    await EasyLocalization.ensureInitialized();
    await MMKVUtil.init();

    // TODO: Consider using `flutter_native_splash`
    await RustLib.init();
    initLogger();
    await api.init(
        tempDir: (await getTemporaryDirectory()).path,
        docDir: (await getApplicationDocumentsDirectory()).path,
        supportDir: (await getApplicationSupportDirectory()).path,
        cacheDir: (await getApplicationCacheDirectory()).path);
    var updateNotifier = UpdateNotifier();
    delayedInit(updateNotifier);
    var gpsManager = GpsManager();
    runApp(EasyLocalization(
        supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
        path: 'assets/translations',
        fallbackLocale: const Locale('en', 'US'),
        saveLocale: false,
        child: MultiProvider(
          providers: [
            ChangeNotifierProvider(create: (context) => gpsManager),
            ChangeNotifierProvider(create: (context) => updateNotifier),
          ],
          child: const MyApp(),
        )));
  }, (error, stackTrace) {
    log.error('Uncaught exception in Flutter: $error', stackTrace);
  });
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

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
    initializeDateFormatting(locale.toString());
    context.setLocale(locale);
  }

  @override
  Widget build(BuildContext context) {
    _naiveLocaleSelection(context);
    return MaterialApp(
      title: "MemoLanes",
      onGenerateTitle: (context) => context.tr('common.memolanes'),
      supportedLocales: context.supportedLocales,
      localizationsDelegates: context.localizationDelegates,
      locale: context.locale,
      navigatorKey: navigatorKey,
      theme: ThemeData(
        useMaterial3: true,
        fontFamily: 'MiSans',
        scaffoldBackgroundColor: const Color(0xFF141414),
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFFB6E13D),
          brightness: Brightness.dark,
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
  const MyHomePage({super.key, required this.title});

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  int _selectedIndex = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mainMapInitialized) {
        mainMapInitialized = true;
        showLoadingDialog(context: context, asyncTask: api.initMainMap());
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Stack(
        children: [
          SafeAreaWrapper(
              useSafeArea: _selectedIndex !=
                  0, // we don't need safe area for `MapUiBody`
              child: switch (_selectedIndex) {
                0 => const MapUiBody(),
                1 => const TimeMachineUIBody(),
                2 => const JourneyUiBody(),
                3 => const AchievementBody(),
                4 => const SettingsBody(),
                _ => throw FormatException('Invalid index: $_selectedIndex')
              }),
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: SafeArea(
              child: Padding(
                padding: const EdgeInsets.only(
                  left: 24,
                  right: 24,
                  bottom: 32,
                ),
                child: BottomNavBar(
                  selectedIndex: _selectedIndex,
                  onIndexChanged: (index) =>
                      setState(() => _selectedIndex = index),
                  hasUpdateNotification:
                      context.watch<UpdateNotifier>().hasUpdateNotification,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
