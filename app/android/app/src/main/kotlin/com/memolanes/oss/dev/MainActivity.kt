package com.memolanes.oss.dev

import android.annotation.TargetApi
import android.app.Activity
import android.os.Build
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    private val screenshotPlugin = ScreenshotPlugin()
    private var screenCaptureObserver: ScreenCaptureObserver? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        flutterEngine.plugins.add(DeviceRegionPlugin())
        flutterEngine.plugins.add(screenshotPlugin)
    }

    override fun onStart() {
        super.onStart()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            screenCaptureObserver = ScreenCaptureObserver(this, screenshotPlugin::onSystemScreenshot)
        }
    }

    override fun onStop() {
        screenCaptureObserver?.close()
        screenCaptureObserver = null
        super.onStop()
    }

    @TargetApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
    private class ScreenCaptureObserver(
        private val activity: Activity,
        onCaptured: () -> Unit,
    ) {
        private val callback = Activity.ScreenCaptureCallback { onCaptured() }

        init {
            activity.registerScreenCaptureCallback(activity.mainExecutor, callback)
        }

        fun close() {
            activity.unregisterScreenCaptureCallback(callback)
        }
    }
}
