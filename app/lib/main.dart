import 'dart:async';

import 'package:memolanes/body/achievement/achievement_explorer_map_controls.dart';

import 'package:memolanes/body/achievement/achievement_region_explorer.dart';
import 'package:memolanes/common/component/shared_map_host.dart';

import 'package:memolanes/theme/app_theme.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:flutter/services.dart';
import 'package:fluttertoast/fluttertoast.dart';
import 'package:memolanes/app_bootstrap.dart';
import 'package:memolanes/body/achievement/achievement_body.dart'
    deferred as achievement;
import 'package:memolanes/body/map/map_body.dart';
import 'package:memolanes/body/map/journey_flow_controller.dart';
import 'package:memolanes/body/map/screenshot/system_screenshot_listener.dart';
import 'package:memolanes/body/settings/settings_body.dart'
    deferred as settings;
import 'package:memolanes/common/achievement_stats_store.dart';
import 'package:memolanes/common/app_theme_controller.dart';
import 'package:memolanes/common/app_translation_loader.dart';
import 'package:memolanes/common/component/app_navigation_shell.dart';
import 'package:memolanes/common/component/bottom_nav_bar.dart';
import 'package:memolanes/common/component/database_version_too_new_gate.dart';
import 'package:memolanes/common/component/map_controls/map_copyright_button.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/duo_device.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/loading_manager.dart';
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
  final savedLocale = MMKVUtil.getStringOpt(MMKVKey.localePreference);
  return EasyLocalization(
    supportedLocales: const [Locale('en', 'US'), Locale('zh', 'CN')],
    path: 'assets/translations',
    assetLoader: const AppTranslationLoader(),
    fallbackLocale: const Locale('en', 'US'),
    startLocale: savedLocale == 'zh-CN'
        ? const Locale('zh', 'CN')
        : savedLocale == 'en-US'
        ? const Locale('en', 'US')
        : null,
    saveLocale: false,
    child: child,
  );
}

class MyApp extends StatefulWidget {
  const MyApp({super.key, this.home});

  final Widget? home;

  @override
  State<MyApp> createState() => _MyAppState();
}

class _MyAppState extends State<MyApp> with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeLocales(List<Locale>? locales) {
    super.didChangeLocales(locales);
    if (MMKVUtil.getStringOpt(MMKVKey.localePreference) != null) return;

    final deviceLocale = locales != null && locales.isNotEmpty
        ? locales.first
        : WidgetsBinding.instance.platformDispatcher.locale;
    unawaited(
      context.setLocale(AppBootstrap.selectInitialLocale(deviceLocale)),
    );
  }

  @override
  Widget build(BuildContext context) {
    final appThemeController = context.watch<AppThemeController>();

    return MaterialApp(
      title: "MemoLanes",
      onGenerateTitle: (context) => context.tr('common.memolanes'),
      supportedLocales: context.supportedLocales,
      localizationsDelegates: [
        ...GlobalMaterialLocalizations.delegates,
        ...context.localizationDelegates,
      ],
      locale: context.locale,
      navigatorKey: navigatorKey,
      builder: (context, child) {
        return AnnotatedRegion<SystemUiOverlayStyle>(
          value: Theme.of(context).appBarTheme.systemOverlayStyle!,
          // Bridge the theme for dependencies such as flutter_markdown_plus
          // that still use the SDK Material library.
          // ignore: deprecated_member_use
          child: MaterialUiCompatibilityBridge(
            child: DuoDeviceProvider(
              child: GlobalLoadingOverlay(
                child: child ?? const SizedBox.shrink(),
              ),
            ),
          ),
        );
      },
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: appThemeController.themeMode,
      home: widget.home ?? const MyHomePage(title: 'MemoLanes [OSS]'),
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
  bool _explorerOpen = false;
  final _sharedMap = SharedMapController();
  final _explorerKey = GlobalKey<AchievementRegionExplorerState>();
  final _explorerViewportKey = GlobalKey();
  bool _recordMapOnly = false;
  bool _hasMapAttribution = false;
  final _journeys = JourneyFlowController();
  DateTime? _lastExitPopAt;
  late final StreamSubscription<void> _screenshotSubscription;

  Future<void>? _achievementLib;
  Future<void>? _settingsLib;

  /// Retains Record/Timeline/Journeys controls and their camera state; the
  /// platform view itself belongs to the shell host.
  final GlobalKey<MapBodyState> _mapBodyKey = GlobalKey<MapBodyState>();

  @override
  void initState() {
    super.initState();
    _journeys.addListener(_onJourneysChanged);
    _screenshotSubscription = listenForSystemScreenshots(() {
      if (_selectedIndex <= 2) {
        unawaited(_mapBodyKey.currentState?.takeScreenshot());
      }
    });
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await AppBootstrap.completeUiStartup(context);
    });
  }

  void _onJourneysChanged() => setState(() {});

  void _onMapAttributionChanged(bool available) {
    if (!mounted || _hasMapAttribution == available) {
      return;
    }
    setState(() => _hasMapAttribution = available);
  }

  void _selectTab(int index) {
    if (index == _selectedIndex ||
        GlobalLoadingManager.instance.isNavigationBlocked) {
      return;
    }
    if (_selectedIndex == 2 && !_journeys.leave()) return;
    setState(() {
      _selectedIndex = index;
      _recordMapOnly = false;
    });
  }

  void _toggleRecordMapOnly() {
    if (_selectedIndex != 0) return;
    setState(() {
      _recordMapOnly = !_recordMapOnly;
      _lastExitPopAt = null;
    });
  }

  @override
  void dispose() {
    unawaited(_screenshotSubscription.cancel());
    _journeys.removeListener(_onJourneysChanged);
    _journeys.dispose();
    _sharedMap.dispose();
    super.dispose();
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
    final loading = GlobalLoadingManager.instance;
    if (loading.isLoading || loading.isNavigationBlocked) return;
    if (_explorerOpen) {
      _explorerKey.currentState?.requestBack();
      return;
    }
    if (_recordMapOnly) {
      _toggleRecordMapOnly();
      return;
    }
    if (_journeys.requestBack() != JourneyBackResult.unhandled) return;

    if (_selectedIndex != 0) {
      _selectTab(0);
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
  Widget _buildPageContent({
    required double topSafeArea,
    required double bottomSafeArea,
  }) {
    _sharedMap.recordActive = _selectedIndex <= 2 && !_explorerOpen;
    _sharedMap.explorerActive = _explorerOpen;
    return Stack(
      fit: StackFit.expand,
      children: [
        MapBody(
          key: _mapBodyKey,
          sharedMap: _sharedMap,
          active: _selectedIndex <= 2 && !_explorerOpen,
          journeys: _journeys,
          onMapAttributionChanged: _onMapAttributionChanged,
          recordMapOnly: _recordMapOnly,
          onRecordMapTap: _toggleRecordMapOnly,
          mode: switch (_selectedIndex) {
            1 => MapMode.timeMachine,
            2 => MapMode.journeys,
            _ => MapMode.normal,
          },
        ),
        if (_selectedIndex > 2)
          _buildDeferredTabBody(
            _selectedIndex,
            topSafeArea: topSafeArea,
            bottomSafeArea: bottomSafeArea,
          ),
      ],
    );
  }

  Widget _buildExplorer() => AchievementRegionExplorer(
    key: _explorerKey,
    onClose: _closeExplorer,
    controlsBuilder: (context, region) => AchievementExplorerMapControls(
      region: region,
      readAttribution: _hasMapAttribution
          ? () async =>
                await _sharedMap.mapKey.currentState?.getMapAttribution()
          : null,
    ),
    mapBuilder: (context, target) => SizedBox.expand(
      key: _explorerViewportKey,
      child: LayoutBuilder(
        builder: (context, constraints) {
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (!mounted || !_explorerOpen) return;
            final viewport = _explorerViewportKey.currentContext
                ?.findRenderObject();
            final host = _sharedMap.viewportKey.currentContext
                ?.findRenderObject();
            if (viewport is! RenderBox ||
                host is! RenderBox ||
                !viewport.hasSize) {
              return;
            }
            final origin =
                viewport.localToGlobal(Offset.zero) -
                host.localToGlobal(Offset.zero);
            final rect = origin & viewport.size;
            _sharedMap.publishExplorer((
              featureGeojson: target.featureGeojson,
              flyToBounds: target.flyToBounds,
              flyToView: target.flyToView,
              boundsPadding: EdgeInsets.fromLTRB(
                rect.left + target.boundsPadding.left,
                rect.top + target.boundsPadding.top,
                host.size.width - rect.right + target.boundsPadding.right,
                host.size.height - rect.bottom + target.boundsPadding.bottom,
              ),
              cameraRevision: target.cameraRevision,
            ));
          });
          return const SizedBox.expand();
        },
      ),
    ),
  );

  Future<void> _openExplorer() async {
    final returnView = await _sharedMap.readHomeView();
    if (!mounted || _selectedIndex != 3 || _explorerOpen) return;
    _sharedMap.returnView = returnView;
    setState(() => _explorerOpen = true);
  }

  void _closeExplorer() {
    _sharedMap.clearExplorer();
    setState(() => _explorerOpen = false);
  }

  Widget _buildDeferredTabBody(
    int index, {
    required double topSafeArea,
    required double bottomSafeArea,
  }) {
    return switch (index) {
      3 => _buildDeferredBody(
        _achievementLib ??= achievement.loadLibrary(),
        () => achievement.AchievementBody(
          onOpenExplorer: _openExplorer,
          topSafeArea: topSafeArea,
          bottomSafeArea: bottomSafeArea,
        ),
      ),
      4 => _buildDeferredBody(
        _settingsLib ??= settings.loadLibrary(),
        () => settings.SettingsBody(
          topSafeArea: topSafeArea,
          bottomSafeArea: bottomSafeArea,
        ),
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
      child: AnnotatedRegion<SystemUiOverlayStyle>(
        value: _selectedIndex <= 2
            ? AppTheme.mapSystemOverlayStyle(Theme.of(context))
            : Theme.of(context).appBarTheme.systemOverlayStyle!,
        child: AppNavigationShell(
          bodyBuilder: _buildPageContent,
          retainedMap: SharedMapHost(controller: _sharedMap),
          fullBleedOverlay: _explorerOpen ? _buildExplorer() : null,
          mapVisible: _selectedIndex <= 2 || _explorerOpen,
          navigationVisible: _journeys.appChromeVisible && !_recordMapOnly,
          animateNavigationVisibility: _selectedIndex == 0,
          contentMode: _selectedIndex > 2
              ? AppNavigationContentMode.inset
              : AppNavigationContentMode.fullBleed,
          navigationBuilder: (context, {required axis}) => BottomNavBar(
            axis: axis,
            selectedIndex: _selectedIndex,
            onIndexChanged: _selectTab,
            hasUpdateNotification: context
                .watch<UpdateNotifier>()
                .hasUpdateNotification,
          ),
          bottomTrailingOverlay:
              (_selectedIndex <= 2 || _explorerOpen) && _hasMapAttribution
              ? MapCopyrightButton(
                  readAttribution: () async =>
                      await _mapBodyKey.currentState?.getMapAttribution(),
                )
              : null,
        ),
      ),
    );
  }
}
