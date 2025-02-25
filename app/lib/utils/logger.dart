import 'package:flutter/foundation.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class Logger {
  final String tag;

  /// Create a logger with a specific tag to identify the source of logs
  Logger(this.tag);

  /// Log an info message
  void i(String message) {
    final formattedMessage = '[${tag}] $message';
    debugPrint('📘 INFO: $formattedMessage');
    api.writeLog(message: formattedMessage, level: api.LogLevel.info);
  }

  /// Log a warning message
  void w(String message) {
    final formattedMessage = '[${tag}] $message';
    debugPrint('⚠️ WARN: $formattedMessage');
    api.writeLog(message: formattedMessage, level: api.LogLevel.warn);
  }

  /// Log an error message
  void e(String message, [dynamic error, StackTrace? stackTrace]) {
    final formattedMessage =
        '[${tag}] $message${error != null ? ' | Error: $error' : ''}';
    debugPrint('🔴 ERROR: $formattedMessage');
    api.writeLog(message: formattedMessage, level: api.LogLevel.error);

    // Log stack trace separately to avoid too long messages
    if (stackTrace != null) {
      debugPrint('Stack trace: $stackTrace');
      api.writeLog(
          message: '[${tag}] Stack trace: $stackTrace',
          level: api.LogLevel.error);
    }
  }

  /// Log a debug message (not logged to Rust in release builds)
  void d(String message) {
    final formattedMessage = '[${tag}] $message';
    debugPrint('🔍 DEBUG: $formattedMessage');

    // Only log debug messages to Rust in debug mode
    if (kDebugMode) {
      api.writeLog(message: formattedMessage, level: api.LogLevel.info);
    }
  }
}
