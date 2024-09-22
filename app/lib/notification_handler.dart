import 'package:flutter_local_notifications/flutter_local_notifications.dart';

class NotificationHandler {
  static final NotificationHandler instance = NotificationHandler._();

  FlutterLocalNotificationsPlugin flutterLocalNotificationsPlugin =
      FlutterLocalNotificationsPlugin();

  // NOTE: we actually don't send this on Android.
  // See `_UnexpectedCloseNotifier` for more detial.
  NotificationDetails alertPlatformChannelSpecifics =
      const NotificationDetails();
  int alertUnexpectedClosedId = 100;

  NotificationHandler._();

  initialize() async {
    var initializationSettingsAndroid =
        const AndroidInitializationSettings('@mipmap/ic_launcher');
    var initializationSettingsIOS = const DarwinInitializationSettings();
    var initializationSettings = InitializationSettings(
        android: initializationSettingsAndroid, iOS: initializationSettingsIOS);
    await flutterLocalNotificationsPlugin.initialize(initializationSettings);
  }
}
