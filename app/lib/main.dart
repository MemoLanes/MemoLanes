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
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/constants/index.dart';
import 'package:provider/provider.dart';

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
      builder: (context, child) {
        return GlobalLoadingOverlay(
          child: child ?? const SizedBox.shrink(),
        );
      },
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

  Future<void>? _journeyLib;
  Future<void>? _achievementLib;
  Future<void>? _settingsLib;

  /// Keeps MapBody's State stable so that switching between tab 0 and 1 does
  /// not trigger parent rebuild and thus avoids MapBody/WebView being
  /// recreated and the web page reloading.
  final GlobalKey<MapBodyState> _mapBodyKey = GlobalKey<MapBodyState>();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await showPrivacyAgreementIfNeeded(context);
      if (!context.mounted) return;

      var mainMapReady = AppBootstrap.mainMapReady;

      if (!mainMapReady.isCompleted) {
        await showLoadingDialog(
          asyncTask: mainMapReady.future,
        );
      }
      if (!context.mounted) return;
      await tryShowPermissionSheetIfFirstTime();
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
    if (GlobalLoadingManager.instance.isLoading) return;

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

  /// Tabs 0 and 1: map (shared MapBody, overlay switched by mode); tabs 2, 3, 4: separate pages.
  Widget _buildPageContent() {
    if (_selectedIndex <= 1) {
      return MapBody(
        key: _mapBodyKey,
        mode: _selectedIndex == 0 ? MapMode.normal : MapMode.timeMachine,
      );
    }
    return _buildDeferredTabBody(_selectedIndex);
  }

  Widget _buildDeferredTabBody(int index) {
    return switch (index) {
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
      _ => throw RangeError('Invalid tab index: $index'),
    };
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
              useSafeArea: _selectedIndex > 1,
              child: _buildPageContent(),
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
