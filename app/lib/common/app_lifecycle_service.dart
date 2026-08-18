import 'dart:async';
import 'dart:io';

import 'package:flutter_fgbg/flutter_fgbg.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class AppLifecycleService {
  static final AppLifecycleService instance = AppLifecycleService._internal();
  AppLifecycleService._internal();

  StreamSubscription? _sub;
  Timer? _freeResourceCountdown;
  bool _countdownCanceled = false;
  final Set<InAppWebViewController> _webViewControllers = {};
  final Set<InAppWebViewController> _pausedWebViewControllers = {};
  Future<void> _webViewLifecycleTask = Future<void>.value();
  bool _webViewsShouldBePaused = false;
  bool _androidTimersPaused = false;

  bool get isRunning => _sub != null;

  void start() {
    if (_sub != null) return;

    _webViewsShouldBePaused = FGBGEvents.last == FGBGType.background;
    _sub = FGBGEvents.instance.stream.listen((event) {
      final triggerTime = DateTime.now();

      if (event == FGBGType.background) {
        log.info(
            '[AppLifecycleService][$triggerTime] Background event received.');
        _webViewsShouldBePaused = true;
        _scheduleWebViewLifecycleSync();
        _countdownCanceled = false;
        _startFreeResourceCountdown();
      } else if (event == FGBGType.foreground) {
        log.info(
            '[AppLifecycleService][$triggerTime] Foreground event received.');
        _webViewsShouldBePaused = false;
        _scheduleWebViewLifecycleSync();
        _reset();
        _reloadResource();
      }
    });
    _scheduleWebViewLifecycleSync();
  }

  void registerWebView(InAppWebViewController controller) {
    _webViewControllers.add(controller);
    _scheduleWebViewLifecycleSync();
  }

  void unregisterWebView(InAppWebViewController controller) {
    _webViewControllers.remove(controller);
    _pausedWebViewControllers.remove(controller);
  }

  void _scheduleWebViewLifecycleSync() {
    _webViewLifecycleTask = _webViewLifecycleTask.then((_) async {
      try {
        await _syncWebViewLifecycle();
      } catch (error, stackTrace) {
        log.error(
          '[AppLifecycleService] Unexpected WebView lifecycle error: $error',
          stackTrace,
        );
      }
    });
  }

  Future<void> _syncWebViewLifecycle() async {
    if (!Platform.isAndroid && !Platform.isIOS) return;

    final controllers = _webViewControllers.toList(growable: false);
    if (_webViewsShouldBePaused) {
      if (Platform.isAndroid) {
        // WebView.onPause() is per-instance and does not pause JavaScript.
        for (final controller in controllers) {
          if (!_webViewControllers.contains(controller) ||
              _pausedWebViewControllers.contains(controller)) {
            continue;
          }
          if (await _runWebViewOperation(
                'pause Android WebView',
                controller.pause,
              ) &&
              _webViewControllers.contains(controller)) {
            _pausedWebViewControllers.add(controller);
          }
        }

        // Android WebView.pauseTimers() is process-global, so one live
        // controller is enough to stop JavaScript timers in every WebView.
        if (!_androidTimersPaused) {
          final controller = _firstRegisteredController(controllers);
          if (controller != null &&
              await _runWebViewOperation(
                'pause Android WebView timers',
                controller.pauseTimers,
              )) {
            _androidTimersPaused = true;
          }
        }
      } else {
        // WKWebView has no pause()/resume() equivalent. The plugin implements
        // pauseTimers()/resumeTimers() per WebView using JavaScript.
        for (final controller in controllers) {
          if (!_webViewControllers.contains(controller) ||
              _pausedWebViewControllers.contains(controller)) {
            continue;
          }
          if (await _runWebViewOperation(
                'pause iOS WebView timers',
                controller.pauseTimers,
              ) &&
              _webViewControllers.contains(controller)) {
            _pausedWebViewControllers.add(controller);
          }
        }
      }
      return;
    }

    if (Platform.isAndroid) {
      // Timers were paused globally, so resume them before resuming each view.
      if (_androidTimersPaused) {
        final controller = _firstRegisteredController(controllers);
        if (controller != null &&
            await _runWebViewOperation(
              'resume Android WebView timers',
              controller.resumeTimers,
            )) {
          _androidTimersPaused = false;
        }
      }

      for (final controller in controllers) {
        if (!_webViewControllers.contains(controller) ||
            !_pausedWebViewControllers.contains(controller)) {
          continue;
        }
        if (await _runWebViewOperation(
          'resume Android WebView',
          controller.resume,
        )) {
          _pausedWebViewControllers.remove(controller);
        }
      }
    } else {
      for (final controller in controllers) {
        if (!_webViewControllers.contains(controller) ||
            !_pausedWebViewControllers.contains(controller)) {
          continue;
        }
        if (await _runWebViewOperation(
          'resume iOS WebView timers',
          controller.resumeTimers,
        )) {
          _pausedWebViewControllers.remove(controller);
        }
      }
    }
  }

  InAppWebViewController? _firstRegisteredController(
    List<InAppWebViewController> controllers,
  ) {
    for (final controller in controllers) {
      if (_webViewControllers.contains(controller)) return controller;
    }
    return null;
  }

  Future<bool> _runWebViewOperation(
    String description,
    Future<void> Function() operation,
  ) async {
    try {
      await operation();
      log.info('[AppLifecycleService] Completed: $description.');
      return true;
    } catch (error, stackTrace) {
      log.error(
        '[AppLifecycleService] Failed to $description: $error',
        stackTrace,
      );
      return false;
    }
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
