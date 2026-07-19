import 'dart:async';

import 'package:flutter_fgbg/flutter_fgbg.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class AppLifecycleService {
  static final AppLifecycleService instance = AppLifecycleService._internal();
  AppLifecycleService._internal();

  StreamSubscription? _sub;
  Timer? _freeResourceCountdown;
  bool _countdownCanceled = false;

  bool get isRunning => _sub != null;

  void start() {
    if (_sub != null) return;

    _sub = FGBGEvents.instance.stream.listen((event) {
      final triggerTime = DateTime.now();

      if (event == FGBGType.background) {
        log.info(
            '[AppLifecycleService][$triggerTime] Background event received.');
        _countdownCanceled = false;
        _startFreeResourceCountdown();
      } else if (event == FGBGType.foreground) {
        log.info(
            '[AppLifecycleService][$triggerTime] Foreground event received.');
        _reset();
        _reloadResource();
      }
    });
  }

  void _reset() {
    _freeResourceCountdown?.cancel();
    _freeResourceCountdown = null;
    _countdownCanceled = true;
  }

  void _startFreeResourceCountdown() {
    if (_freeResourceCountdown != null) {
      _freeResourceCountdown!.cancel();
      _freeResourceCountdown = null;
    }

    // try to free resource after 2 minutes in background
    _freeResourceCountdown = Timer(const Duration(seconds: 2 * 60), () {
      // delay a bit to avoid flicker with foreground event
      Future.delayed(const Duration(milliseconds: 500), () {
        if (!_countdownCanceled) {
          _freeResource();
        } else {
          final triggerTime = DateTime.now();
          log.info(
              '[AppLifecycleService][$triggerTime] free resource task skipped.');
        }
      });
    });
  }

  void _freeResource() {
    final triggerTime = DateTime.now();
    log.info('[AppLifecycleService][$triggerTime] Try to free resource.');
    api.freeResourceForLongTimeBackground();
  }

  void _reloadResource() {
    // TODO: would be nice if we can display a loading indicator if this is taking very long
    api.reloadResourceForForeground();
  }

  void stop() {
    _sub?.cancel();
    _sub = null;
    _reset();
  }
}
