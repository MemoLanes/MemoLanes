import 'dart:async';

import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/notification_handler.dart';
import 'package:memolanes/time_machine.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:memolanes/settings.dart';
import 'package:memolanes/gps_page.dart';
import 'package:memolanes/gps_recording_state.dart';
import 'package:memolanes/journey.dart';
import 'package:memolanes/map.dart';
import 'package:memolanes/raw_data.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/frb_generated.dart';
import 'package:provider/provider.dart';
import 'package:badges/badges.dart' as badges;
import 'package:timezone/data/latest.dart' as tz;

void delayedInit(UpdateNotifier updateNotifier) {
  Future.delayed(const Duration(milliseconds: 2000), () async {
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
    doWork() async {
      // TODO: for future use
    }

    await doWork();
    Timer.periodic(const Duration(minutes: 10), (_) async {
      await api.tenMinutesHeartbeat();
      await doWork();
    });
  });
}

void main() async {
  // This is required since we are doing things before calling `runApp`.
  WidgetsFlutterBinding.ensureInitialized();
  // TODO: Consider using `flutter_native_splash`
  await RustLib.init();
  await api.init(
      tempDir: (await getTemporaryDirectory()).path,
      docDir: (await getApplicationDocumentsDirectory()).path,
      supportDir: (await getApplicationSupportDirectory()).path,
      cacheDir: (await getApplicationCacheDirectory()).path);
  var updateNotifier = UpdateNotifier();
  delayedInit(updateNotifier);
  await NotificationHandler.instance.initialize();
  var gpsRecordingState = GpsRecordingState();
  tz.initializeTimeZones();
  runApp(
    MultiProvider(
      providers: [
        ChangeNotifierProvider(create: (context) => gpsRecordingState),
        ChangeNotifierProvider(create: (context) => updateNotifier),
      ],
      child: const MyApp(),
    ),
  );
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  // This widget is the root of your application.
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MemoLanes',
      theme: ThemeData(
        // This is the theme of your application.
        //
        // Try running your application with "flutter run". You'll see the
        // application has a blue toolbar. Then, without quitting the app, try
        // changing the primarySwatch below to Colors.green and then invoke
        // "hot reload" (press "r" in the console where you ran "flutter run",
        // or simply save your changes to "hot reload" in a Flutter IDE).
        // Notice that the counter didn't reset back to zero; the application
        // is not restarted.
        primarySwatch: Colors.blue,
      ),
      home: const MyHomePage(title: 'MemoLanes [OSS]'),
    );
  }
}

class MyHomePage extends StatefulWidget {
  const MyHomePage({Key? key, required this.title}) : super(key: key);

  // This widget is the home page of your application. It is stateful, meaning
  // that it has a State object (defined below) that contains fields that affect
  // how it looks.

  // This class is the configuration for the state. It holds the values (in this
  // case the title) provided by the parent (in this case the App widget) and
  // used by the build method of the State. Fields in a Widget subclass are
  // always marked "final".

  final String title;

  @override
  State<MyHomePage> createState() => _MyHomePageState();
}

class _MyHomePageState extends State<MyHomePage> {
  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 5,
      child: Scaffold(
          appBar: AppBar(
            bottom: TabBar(
              tabs: [
                const Tab(icon: Icon(Icons.home)),
                const Tab(icon: Icon(Icons.update)),
                const Tab(icon: Icon(Icons.map)),
                Tab(
                  child: badges.Badge(
                    badgeStyle: badges.BadgeStyle(
                      shape: badges.BadgeShape.square,
                      borderRadius: BorderRadius.circular(5),
                      padding: const EdgeInsets.all(2),
                      badgeGradient: const badges.BadgeGradient.linear(
                        colors: [
                          Colors.purple,
                          Colors.blue,
                        ],
                        begin: Alignment.topLeft,
                        end: Alignment.bottomRight,
                      ),
                    ),
                    position: badges.BadgePosition.topEnd(top: -12, end: -20),
                    badgeContent: const Text(
                      'NEW',
                      style: TextStyle(
                          color: Colors.white,
                          fontSize: 10,
                          fontWeight: FontWeight.bold),
                    ),
                    showBadge:
                        context.watch<UpdateNotifier>().hasUpdateNotification(),
                    child: const Icon(Icons.settings),
                  ),
                ),
                const Tab(icon: Icon(Icons.description)),
              ],
            ),
            title: Text(widget.title),
          ),
          body: TabBarView(
            physics: const NeverScrollableScrollPhysics(),
            children: [
              Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.start,
                  children: <Widget>[
                    GPSPage(),
                    const Expanded(
                      child: MapUiBody(),
                    ),
                  ],
                ),
              ),
              const Center(child: TimeMachineUIBody()),
              const Center(child: JourneyUiBody()),
              const Center(child: SettingsBody()),
              const Center(child: RawDataBody()),
            ],
          )),
    );
  }
}
