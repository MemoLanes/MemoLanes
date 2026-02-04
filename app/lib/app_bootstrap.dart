import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart';
import 'package:memolanes/common/app_lifecycle_service.dart';
import 'package:memolanes/common/update_notifier.dart';
import 'package:memolanes/common/gps_manager.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/mmkv_util.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/main.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';

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

class AppBootstrap {
  static bool _started = false;
  static final Completer<void> _mainMapReady = Completer<void>();

  /// 启动计时起点，用于输出 first_screen_ms / map_ready_ms 等可量化日志。
  static DateTime? _launchStartedAt;

  static void recordLaunchStartTime() {
    _launchStartedAt = DateTime.now();
  }

  /// 自 recordLaunchStartTime 起经过的毫秒数，用于打点日志。
  static int? get launchElapsedMs => _launchStartedAt != null
      ? DateTime.now().difference(_launchStartedAt!).inMilliseconds
      : null;

  static Future<void> initAppRuntime() async {
    // This is required since we are doing things before calling `runApp`.
    WidgetsFlutterBinding.ensureInitialized();

    // Run independent inits in parallel to speed up time to first frame.
    final tempDirFuture = getTemporaryDirectory();
    final docDirFuture = getApplicationDocumentsDirectory();
    final supportDirFuture = getApplicationSupportDirectory();
    final cacheDirFuture = getApplicationCacheDirectory();

    await Future.wait([
      EasyLocalization.ensureInitialized(),
      MMKVUtil.init(),
      RustLib.init().whenComplete(() => initLog()),
      tempDirFuture,
      docDirFuture,
      supportDirFuture,
      cacheDirFuture,
    ]);

    await api.init(
        tempDir: (await tempDirFuture).path,
        docDir: (await docDirFuture).path,
        supportDir: (await supportDirFuture).path,
        systemCacheDir: (await cacheDirFuture).path);
  }

  // i18n is ready
  static void startAppServices({
    required GpsManager gpsManager,
    required UpdateNotifier updateNotifier,
  }) {
    if (_started) return;
    _started = true;

    api.initMainMap().then(
      (_) {
        _mainMapReady.complete();
        final ms = launchElapsedMs;
        if (ms != null) {
          // 同时 print 便于控制台/logcat 直接看到；log.info 会写入 Rust 日志文件
          print('[Startup] map_ready_ms=$ms');
          log.info('[Startup] map_ready_ms=$ms');
        }
      },
      onError: (e, s) {
        _mainMapReady.completeError(e, s);
        log.error("initMainMap error $e");
      },
    );
    AppLifecycleService.instance.start();
    gpsManager.readyToStart();

    delayedInit(updateNotifier);
  }

  /// the return value should be considered readonly
  static Completer<void> get mainMapReady {
    return _mainMapReady;
  }
}
