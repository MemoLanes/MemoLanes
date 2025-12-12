import 'dart:async';

import 'package:flutter_fgbg/flutter_fgbg.dart';
import 'package:memolanes/common/log.dart';

class AppLifecycleService {
  static final AppLifecycleService instance = AppLifecycleService._internal();
  AppLifecycleService._internal();

  StreamSubscription? _sub;
  Timer? _checker;

  bool canceled = false;

  bool get isRunning => _sub != null;

  void start() {
    if (_sub != null) return;

    _sub = FGBGEvents.instance.stream.listen((event) {
      final triggerTime = DateTime.now();

      if (event == FGBGType.background) {
        log.info('[${triggerTime.toIso8601String()}] Background event received.');
        canceled = false;
        _startChecker();
      } else if (event == FGBGType.foreground) {
        log.info('[${triggerTime.toIso8601String()}] Foreground event received.');
        _reset();_reload();
      }
    });
  }

  void _reset() {
    _checker?.cancel();
    _checker = null;
    canceled = true;
  }

  void _startChecker() {
    if (_checker != null) {
      _checker!.cancel();
      _checker = null;
    }

    _checker = Timer(const Duration(seconds: 30), () {
      Future.delayed(const Duration(milliseconds: 100), () {
        if (!canceled) {
          _clean();
        } else {
          final triggerTime = DateTime.now();
          log.info(
              '[${triggerTime.toIso8601String()}] Scheduled task skipped (canceled).');
        }
      });
    });
  }
  void _clean() {
    final triggerTime = DateTime.now();
    log.info('[${triggerTime.toIso8601String()}] Running scheduled task.');
    // TODO:  API
  }

  void _reload() {
    // TODO:  API
  }

  void stop() {
    _sub?.cancel();
    _sub = null;
    _reset();
  }
}
