import 'package:flutter/foundation.dart';
import 'package:logging/logging.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

void initLogger() {
  if (kDebugMode) {
    api.subscribeToLogStream().listen((log) {
      print(log);
    });
  }
  Logger.root.level = Level.INFO;
  Logger.root.onRecord.listen((rec) {
    api.LogLevel logLevel = switch (rec.level) {
      Level.INFO => api.LogLevel.info,
      Level.WARNING => api.LogLevel.warn,
      Level.SEVERE => api.LogLevel.error,
      _ => api.LogLevel.info,
    };
    api.writeLog(message: rec.message, level: logLevel);
  });
}

class _Log {
  final Logger _logger = Logger('App');

  void info(String message) {
    _logger.info(message);
  }

  void warning(String message) {
    _logger.warning(message);
  }

  void error(String message) {
    _logger.severe(message);
  }
}

final log = _Log();
