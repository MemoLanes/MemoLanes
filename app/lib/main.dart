import 'dart:async';
import 'dart:io';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/app_bootstrap.dart';
import 'package:memolanes/body/achievement/achievement_body.dart'
    deferred as achievement;
import 'package:memolanes/body/journey/journey_body.dart' deferred as journey;
import 'package:memolanes/body/map/map_body.dart';
import 'package:memolanes/body/privacy_agreement.dart';
import 'package:memolanes/body/settings/settings_body.dart'
    deferred as settings;
import 'package:memolanes/body/time_machine/time_machine_body.dart'
    deferred as time_machine;
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/share_handler_util.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/constants/index.dart';
import 'package:provider/provider.dart';

GlobalKey<NavigatorState> navigatorKey = GlobalKey<NavigatorState>();

void main() async {
  runZonedGuarded(() async {
    await AppBootstrap.initAppRuntime();

    final gpsManager = GpsManager();
    final updateNotifier = UpdateNotifier();

    runApp(
      EasyLocalization(
        supportedLocales: const [
          Locale('en', 'US'),
          Locale('zh', 'CN'),
        ],
        path: 'assets/translations',
        fallbackLocale: const Locale('en', 'US'),
        saveLocale: false,
        child: MultiProvider(
          providers: [
            // Do NOT use `create: (_) => gpsManager` here
            ChangeNotifierProvider.value(value: gpsManager),
            ChangeNotifierProvider.value(value: updateNotifier),
          ],
          child: const MyApp(),
        ),
      ),
    );

    WidgetsBinding.instance.addPostFrameCallback((_) {
      AppBootstrap.onFirstFrame();
    });

    AppBootstrap.startAppServices(
      gpsManager: gpsManager,
      updateNotifier: updateNotifier,
    );
  }, (error, stackTrace) {
    log.error('Uncaught exception in Flutter: $error', stackTrace);
  });
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: "MemoLanes",
      onGenerateTitle: (context) => context.tr('common.memolanes'),
      supportedLocales: context.supportedLocales,
      localizationsDelegates: context.localizationDelegates,
      locale: context.locale,
      navigatorKey: navigatorKey,
      theme: ThemeData(
        useMaterial3: true,
        fontFamilyFallback:
            Platform.isIOS ? ['.AppleSystemUIFont', 'PingFang SC'] : null,
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
  DateTime? _lastExitPopAt;

  Future<void>? _timeMachineLib;
  Future<void>? _journeyLib;
  Future<void>? _achievementLib;
  Future<void>? _settingsLib;

  @override
  void initState() {
    super.initState();
    ShareHandlerUtil.init(context);
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      showPrivacyAgreementIfNeeded(context);

      var mainMapReady = AppBootstrap.mainMapReady;

      if (!mainMapReady.isCompleted) {
        await showLoadingDialog(
          context: context,
          asyncTask: mainMapReady.future,
        );
      }
    });
  }

  Widget _buildDeferredBody(Future<void> loadFuture, Widget Function() body) {
    return FutureBuilder<void>(
      future: loadFuture,
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.done &&
            !snapshot.hasError) {
          return body();
        }

        if (snapshot.hasError) {
          log.error(
              'Deferred load failed ${snapshot.error}', snapshot.stackTrace);
        }

        return const Center(child: CircularProgressIndicator());
      },
    );
  }

  Future<void> _handleOnPop() async {
    if (_selectedIndex != 0) {
      setState(() => _selectedIndex = 0);
      return;
    }

    final now = DateTime.now();
    final lastPop = _lastExitPopAt;
    if (lastPop == null ||
        now.difference(lastPop) > const Duration(seconds: 2)) {
      _lastExitPopAt = now;
      Fluttertoast.showToast(msg: tr("home.double_back_exit"));
      return;
    }
    SystemNavigator.pop();
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: false,
      onPopInvokedWithResult: (bool didPop, dynamic result) async {
        if (didPop) return;
        await _handleOnPop();
      },
      child: Scaffold(
        body: Stack(
          children: [
            SafeAreaWrapper(
              useSafeArea: _selectedIndex !=
                  0, // we don't need safe area for `MapUiBody`
              child: switch (_selectedIndex) {
                0 => const MapBody(),
                1 => _buildDeferredBody(
                    _timeMachineLib ??= time_machine.loadLibrary(),
                    () => time_machine.TimeMachineBody(),
                  ),
                2 => _buildDeferredBody(
                    _journeyLib ??= journey.loadLibrary(),
                    () => journey.JourneyBody(),
                  ),
                3 => _buildDeferredBody(
                    _achievementLib ??= achievement.loadLibrary(),
                    () => achievement.AchievementBody(),
                  ),
                4 => _buildDeferredBody(
                    _settingsLib ??= settings.loadLibrary(),
                    () => settings.SettingsBody(),
                  ),
                _ => throw FormatException('Invalid index: $_selectedIndex'),
              },
            ),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: SafeArea(
                child: Padding(
                  padding: const EdgeInsets.only(
                    left: StyleConstants.navBarHorizontalPadding,
                    right: StyleConstants.navBarHorizontalPadding,
                    bottom: StyleConstants.navBarBottomPadding,
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
      ),
    );
  }
}
