import 'dart:async';
import 'dart:io';
import 'dart:math' as math;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/app_bootstrap.dart';
import 'package:memolanes/body/achievement/achievement_body.dart'
    deferred as achievement;
import 'package:memolanes/body/map/map_body.dart';
import 'package:memolanes/body/first_launch_setup.dart';
import 'package:memolanes/body/settings/settings_body.dart'
    deferred as settings;
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/database_version_too_new_gate.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/map_style.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/common/loading_manager.dart';
import 'package:memolanes/constants/index.dart';
import 'package:provider/provider.dart';

void main() async {
  runZonedGuarded(
    () async {
      final startupStatus = await AppBootstrap.initAppRuntime();
      if (startupStatus == AppStartupStatus.databaseVersionTooNew) {
        runApp(
          _appRoot(
            ChangeNotifierProvider(
              create: (_) => AppThemeController(),
              child: const MyApp(home: DatabaseVersionTooNewGate()),
            ),
          ),
        );
        return;
      }

      final gpsManager = GpsManager();
      final updateNotifier = UpdateNotifier();
      final achievementStatsStore = AchievementStatsStore();

      runApp(
        _appRoot(
          MultiProvider(
            providers: [
              // Do NOT use `create: (_) => gpsManager` here
              ChangeNotifierProvider.value(value: gpsManager),
              ChangeNotifierProvider.value(value: updateNotifier),
              ChangeNotifierProvider.value(value: achievementStatsStore),
              ChangeNotifierProvider(create: (_) => AppThemeController()),
            ],
            child: const MyApp(),
          ),
        ),
      );

      AppBootstrap.startAppServices(
        gpsManager: gpsManager,
        updateNotifier: updateNotifier,
      );
    },
    (error, stackTrace) {
      log.error('Uncaught exception in Flutter: $error', stackTrace);
    },
  );
}

Widget _appRoot(Widget child) {
  return EasyLocalization(
    supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
    path: 'assets/translations',
    assetLoader: const AppTranslationLoader(),
    fallbackLocale: const Locale('en', 'US'),
    saveLocale: false,
    child: child,
  );
}

class MyApp extends StatelessWidget {
  const MyApp({super.key, this.home});

  final Widget? home;

  @override
  Widget build(BuildContext context) {
    final appThemeController = context.watch<AppThemeController>();
    final brightness = appThemeController.brightness;
    final systemOverlayStyle = SystemUiOverlayStyle(
      statusBarColor: Colors.transparent,
      statusBarIconBrightness: brightness == Brightness.dark
          ? Brightness.light
          : Brightness.dark,
      statusBarBrightness: brightness,
      systemNavigationBarColor: StyleConstants.canvasColor,
      systemNavigationBarIconBrightness: brightness == Brightness.dark
          ? Brightness.light
          : Brightness.dark,
      systemNavigationBarDividerColor: StyleConstants.lineColor,
    );

    return MaterialApp(
      title: "MemoLanes",
      onGenerateTitle: (context) => context.tr('common.memolanes'),
      supportedLocales: context.supportedLocales,
      localizationsDelegates: context.localizationDelegates,
      locale: context.locale,
      navigatorKey: navigatorKey,
      builder: (context, child) {
        return AnnotatedRegion<SystemUiOverlayStyle>(
          value: systemOverlayStyle,
          child: GlobalLoadingOverlay(child: child ?? const SizedBox.shrink()),
        );
      },
      theme: ThemeData(
        useMaterial3: true,
        fontFamilyFallback: Platform.isIOS
            ? ['.AppleSystemUIFont', 'PingFang SC']
            : null,
        brightness: brightness,
        scaffoldBackgroundColor: StyleConstants.canvasColor,
        canvasColor: StyleConstants.canvasColor,
        colorScheme:
            ColorScheme.fromSeed(
              seedColor: StyleConstants.primaryGreen,
              brightness: brightness,
              primary: StyleConstants.primaryActionColor,
              onPrimary: StyleConstants.onPrimaryActionColor,
              primaryContainer: StyleConstants.selectedSurfaceColor,
              onPrimaryContainer: StyleConstants.deepGreen,
              secondary: StyleConstants.warningColor,
              onSecondary: StyleConstants.isDarkMode
                  ? StyleConstants.inverseInkColor
                  : StyleConstants.inkColor,
              secondaryContainer: StyleConstants.warningSurfaceColor,
              onSecondaryContainer: StyleConstants.warningInkColor,
              error: StyleConstants.dangerColor,
              onError: StyleConstants.onDangerColor,
              errorContainer: StyleConstants.dangerSurfaceColor,
              onErrorContainer: StyleConstants.dangerInkColor,
              outline: StyleConstants.mutedInkColor,
              outlineVariant: StyleConstants.lineColor,
              surface: StyleConstants.surfaceColor,
              onSurface: StyleConstants.inkColor,
              onSurfaceVariant: StyleConstants.mutedInkColor,
              shadow: StyleConstants.shadowColor,
              scrim: StyleConstants.shadowColor,
              surfaceTint: Colors.transparent,
            ).copyWith(
              surfaceContainerLowest: StyleConstants.canvasColor,
              surfaceContainerLow: StyleConstants.surfaceColor,
              surfaceContainer: StyleConstants.surfaceColor,
              surfaceContainerHigh: StyleConstants.elevatedSurfaceColor,
              surfaceContainerHighest: StyleConstants.elevatedSurfaceColor,
            ),
        textTheme:
            (brightness == Brightness.dark
                    ? ThemeData.dark()
                    : ThemeData.light())
                .textTheme
                .merge(AppTypography.textTheme)
                .apply(
                  bodyColor: StyleConstants.inkColor,
                  displayColor: StyleConstants.inkColor,
                ),
        iconTheme: IconThemeData(color: StyleConstants.inkColor),
        dividerColor: StyleConstants.lineColor,
        cardTheme: CardThemeData(
          color: StyleConstants.surfaceColor,
          surfaceTintColor: Colors.transparent,
          elevation: 0,
          margin: EdgeInsets.zero,
        ),
        appBarTheme: AppBarTheme(
          elevation: 0,
          scrolledUnderElevation: 0,
          backgroundColor: StyleConstants.canvasColor,
          foregroundColor: StyleConstants.inkColor,
          surfaceTintColor: Colors.transparent,
          systemOverlayStyle: systemOverlayStyle,
        ),
        filledButtonTheme: FilledButtonThemeData(
          style: FilledButton.styleFrom(
            elevation: 0,
            backgroundColor: StyleConstants.primaryActionColor,
            foregroundColor: StyleConstants.onPrimaryActionColor,
            minimumSize: const Size(0, 44),
            textStyle: AppTypography.button,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
        ),
        elevatedButtonTheme: ElevatedButtonThemeData(
          style: ElevatedButton.styleFrom(
            elevation: 0,
            backgroundColor: StyleConstants.surfaceColor,
            foregroundColor: StyleConstants.isDarkMode
                ? StyleConstants.inkColor
                : StyleConstants.onPrimaryActionColor,
            minimumSize: const Size(0, 44),
            textStyle: AppTypography.button,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
              side: BorderSide(color: StyleConstants.lineColor),
            ),
          ),
        ),
        outlinedButtonTheme: OutlinedButtonThemeData(
          style: OutlinedButton.styleFrom(
            foregroundColor: StyleConstants.deepGreen,
            minimumSize: const Size(0, 44),
            side: BorderSide(color: StyleConstants.lineColor),
            textStyle: AppTypography.button,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
        ),
        textButtonTheme: TextButtonThemeData(
          style: TextButton.styleFrom(
            foregroundColor: StyleConstants.deepGreen,
            textStyle: AppTypography.button,
          ),
        ),
        dialogTheme: DialogThemeData(
          backgroundColor: StyleConstants.isDarkMode
              ? StyleConstants.elevatedSurfaceColor
              : StyleConstants.canvasColor,
          surfaceTintColor: Colors.transparent,
          titleTextStyle: AppTypography.surfaceTitle.copyWith(
            color: StyleConstants.deepGreen,
          ),
          contentTextStyle: AppTypography.body.copyWith(
            color: StyleConstants.inkColor,
          ),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(24),
            side: BorderSide(color: StyleConstants.lineColor),
          ),
        ),
        datePickerTheme: DatePickerThemeData(
          backgroundColor: StyleConstants.isDarkMode
              ? StyleConstants.elevatedSurfaceColor
              : StyleConstants.canvasColor,
          surfaceTintColor: Colors.transparent,
          headerBackgroundColor: StyleConstants.softGreen,
          headerForegroundColor: StyleConstants.deepGreen,
          weekdayStyle: AppTypography.label.copyWith(
            color: StyleConstants.mutedInkColor,
          ),
          todayBorder: BorderSide(color: StyleConstants.deepGreen),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(24),
            side: BorderSide(color: StyleConstants.lineColor),
          ),
          cancelButtonStyle: OutlinedButton.styleFrom(
            foregroundColor: StyleConstants.deepGreen,
            side: BorderSide(color: StyleConstants.lineColor),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
          confirmButtonStyle: FilledButton.styleFrom(
            backgroundColor: StyleConstants.primaryActionColor,
            foregroundColor: StyleConstants.onPrimaryActionColor,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
        ),
        timePickerTheme: TimePickerThemeData(
          backgroundColor: StyleConstants.canvasColor,
          dialBackgroundColor: StyleConstants.surfaceColor,
          dialHandColor: StyleConstants.deepGreen,
          hourMinuteColor: StyleConstants.softGreen,
          hourMinuteTextColor: StyleConstants.inkColor,
          dialTextColor: StyleConstants.inkColor,
          dayPeriodColor: StyleConstants.surfaceColor,
          dayPeriodTextColor: StyleConstants.inkColor,
          entryModeIconColor: StyleConstants.deepGreen,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(24),
            side: BorderSide(color: StyleConstants.lineColor),
          ),
          cancelButtonStyle: OutlinedButton.styleFrom(
            foregroundColor: StyleConstants.deepGreen,
            side: BorderSide(color: StyleConstants.lineColor),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
          confirmButtonStyle: FilledButton.styleFrom(
            backgroundColor: StyleConstants.primaryActionColor,
            foregroundColor: StyleConstants.onPrimaryActionColor,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(14),
            ),
          ),
        ),
        checkboxTheme: CheckboxThemeData(
          fillColor: WidgetStatePropertyAll(StyleConstants.surfaceColor),
          checkColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.disabled)
                ? StyleConstants.mutedInkColor
                : StyleConstants.deepGreen,
          ),
          side: WidgetStateBorderSide.resolveWith(
            (states) => BorderSide(
              color: states.contains(WidgetState.disabled)
                  ? StyleConstants.lineColor
                  : StyleConstants.deepGreen,
              width: 1.4,
            ),
          ),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(5)),
        ),
        progressIndicatorTheme: ProgressIndicatorThemeData(
          color: StyleConstants.primaryActionColor,
          linearTrackColor: StyleConstants.lineColor,
          circularTrackColor: StyleConstants.lineColor,
        ),
        switchTheme: SwitchThemeData(
          thumbColor: const WidgetStatePropertyAll(Colors.transparent),
          thumbIcon: WidgetStateProperty.resolveWith(
            (states) => Icon(
              Icons.circle,
              size: states.contains(WidgetState.selected)
                  ? StyleConstants.switchActiveThumbSize
                  : StyleConstants.switchInactiveThumbSize,
              color: states.contains(WidgetState.selected)
                  ? StyleConstants.switchActiveThumbColor
                  : StyleConstants.switchInactiveThumbColor,
            ),
          ),
          trackColor: WidgetStateProperty.resolveWith(
            (states) => states.contains(WidgetState.selected)
                ? StyleConstants.switchActiveTrackColor
                : StyleConstants.switchInactiveTrackColor,
          ),
          trackOutlineColor: WidgetStatePropertyAll(
            StyleConstants.switchTrackOutlineColor,
          ),
          trackOutlineWidth: const WidgetStatePropertyAll(
            StyleConstants.switchTrackOutlineWidth,
          ),
        ),
        bottomNavigationBarTheme: BottomNavigationBarThemeData(
          elevation: 8,
          backgroundColor: StyleConstants.surfaceColor,
          selectedItemColor: StyleConstants.deepGreen,
          unselectedItemColor: StyleConstants.mutedInkColor,
        ),
      ),
      home: home ?? const MyHomePage(title: 'MemoLanes [OSS]'),
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

  Future<void>? _achievementLib;
  Future<void>? _settingsLib;

  /// Keeps MapBody's State stable so that switching among tabs 0, 1 and 2 does
  /// not trigger parent rebuild and thus avoids MapBody/WebView being
  /// recreated and the web page reloading.
  final GlobalKey<MapBodyState> _mapBodyKey = GlobalKey<MapBodyState>();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await showFirstLaunchSetupIfNeeded(context);
      if (!context.mounted) return;

      final mainMapReady = AppBootstrap.mainMapReady;
      if (!mainMapReady.isCompleted) {
        await showLoadingDialog(asyncTask: mainMapReady.future);
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
            'Deferred load failed ${snapshot.error}',
            snapshot.stackTrace,
          );
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

  /// Tabs 0, 1 and 2 share one MapBody and only switch its overlay. This keeps
  /// the current map position while moving between Record, Timeline and
  /// Journeys. Tabs 3 and 4 are separate pages.
  Widget _buildPageContent() {
    Widget child;
    if (_selectedIndex <= 2) {
      child = MapBody(
        key: _mapBodyKey,
        mode: switch (_selectedIndex) {
          0 => MapMode.normal,
          1 => MapMode.timeMachine,
          _ => MapMode.journeys,
        },
      );
    } else {
      child = KeyedSubtree(
        key: ValueKey(_selectedIndex),
        child: _buildDeferredTabBody(_selectedIndex),
      );
    }

    // Do not retain an outgoing MapBody for a page-transition animation.
    // Re-entering a map tab before that animation ends would temporarily put
    // the same GlobalKey in two subtrees and can crash in debug builds.
    return child;
  }

  Widget _buildDeferredTabBody(int index) {
    return switch (index) {
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
    // The persistent map controls use the shared palette directly. Rebuild the
    // home shell as well as MaterialApp when that palette changes.
    context.watch<AppThemeController>();
    final mediaQuery = MediaQuery.of(context);
    final navBarBottomInset = StyleConstants.navBarBottomInset(context);
    final horizontalSafeArea = math.max(
      mediaQuery.viewPadding.left,
      mediaQuery.viewPadding.right,
    );
    final mapCopyrightTextMarkdown = MapStyle.findById(
      MMKVUtil.getString(MMKVKey.mapStyle),
    ).copyright;
    const mapCopyrightNavBarGap = 6.0;
    const mapCopyrightTrailingGap = 8.0;

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
              useSafeArea: _selectedIndex > 2,
              child: _buildPageContent(),
            ),
            Positioned(
              left: 0,
              right: 0,
              bottom: 0,
              child: Padding(
                padding: EdgeInsets.only(
                  left: horizontalSafeArea,
                  right: horizontalSafeArea,
                  bottom: navBarBottomInset,
                ),
                child: FittedBox(
                  fit: BoxFit.scaleDown,
                  child: SizedBox(
                    width:
                        mediaQuery.size.width -
                        BottomNavBar.designHorizontalMargin * 2,
                    height: BottomNavBar.height,
                    child: BottomNavBar(
                      selectedIndex: _selectedIndex,
                      onIndexChanged: (index) =>
                          setState(() => _selectedIndex = index),
                      hasUpdateNotification: context
                          .watch<UpdateNotifier>()
                          .hasUpdateNotification,
                    ),
                  ),
                ),
              ),
            ),
            if (_selectedIndex <= 2)
              Positioned(
                right: mediaQuery.viewPadding.right + mapCopyrightTrailingGap,
                bottom:
                    navBarBottomInset +
                    BottomNavBar.height +
                    mapCopyrightNavBarGap,
                child: MapCopyrightButton(
                  textMarkdown: mapCopyrightTextMarkdown,
                ),
              ),
          ],
        ),
      ),
    );
  }
}
