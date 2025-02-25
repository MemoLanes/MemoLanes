import 'dart:async';
import 'dart:isolate';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/utils/logger.dart';

final _logger = Logger('ErrorHandler');

class ErrorHandler {
  /// Set up global error handlers for the app
  static void initialize() {
    // Handle Flutter framework errors
    FlutterError.onError = (FlutterErrorDetails details) {
      _logger.e('Flutter error: ${details.exception}', 
        details.exception, details.stack);
      
      // Rethrow in debug mode so the red screen appears
      if (kDebugMode) {
        FlutterError.dumpErrorToConsole(details);
      }
    };

    // Handle errors from the current zone
    PlatformDispatcher.instance.onError = (Object error, StackTrace stack) {
      _logger.e('Uncaught platform exception', error, stack);
      return true; // true means we've handled the error
    };

    // Handle errors from Isolates
    Isolate.current.addErrorListener(RawReceivePort((pair) {
      final List<dynamic> errorAndStacktrace = pair;
      final error = errorAndStacktrace[0];
      final stack = StackTrace.fromString(errorAndStacktrace[1]);
      _logger.e('Isolate error', error, stack);
    }).sendPort);
  }
} 