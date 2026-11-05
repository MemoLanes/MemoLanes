package com.memolanes.oss.dev

import android.Manifest
import android.app.Activity
import android.content.ContentValues
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.embedding.engine.plugins.activity.ActivityAware
import io.flutter.embedding.engine.plugins.activity.ActivityPluginBinding
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.PluginRegistry
import java.io.File
import java.util.UUID
import java.util.concurrent.Executors

class ScreenshotPlugin : FlutterPlugin, ActivityAware,
    MethodChannel.MethodCallHandler, PluginRegistry.RequestPermissionsResultListener {
    private lateinit var context: Context
    private lateinit var channel: MethodChannel
    private var binding: ActivityPluginBinding? = null
    private var pending: Pair<ByteArray, MethodChannel.Result>? = null
    private val executor = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        context = binding.applicationContext
        channel = MethodChannel(binding.binaryMessenger, "com.memolanes/screenshots")
        channel.setMethodCallHandler(this)
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "saveToPhotos") {
            result.notImplemented()
            return
        }
        val png = call.argument<ByteArray>("png")
        if (png == null || png.isEmpty()) {
            result.error("invalid_image", "Missing PNG data", null)
            return
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q &&
            context.checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) !=
            PackageManager.PERMISSION_GRANTED) {
            val activity: Activity? = binding?.activity
            if (activity == null || pending != null) {
                result.error("unavailable", "Cannot request photo permission", null)
                return
            }
            pending = Pair(png, result)
            activity.requestPermissions(arrayOf(Manifest.permission.WRITE_EXTERNAL_STORAGE), 6091)
        } else {
            save(png, result)
        }
    }

    override fun onRequestPermissionsResult(
        requestCode: Int, permissions: Array<out String>, grantResults: IntArray
    ): Boolean {
        if (requestCode != 6091) return false
        val request = pending ?: return false
        pending = null
        if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
            save(request.first, request.second)
        } else {
            request.second.error("permission_denied", "Photo permission denied", null)
        }
        return true
    }

    @Suppress("DEPRECATION")
    private fun save(png: ByteArray, result: MethodChannel.Result) {
        executor.execute {
            val resolver = context.contentResolver
            var uri: android.net.Uri? = null
            var legacyFile: File? = null
            try {
                val name = "MemoLanes_${UUID.randomUUID()}.png"
                val values = ContentValues().apply {
                    put(MediaStore.Images.Media.DISPLAY_NAME, name)
                    put(MediaStore.Images.Media.MIME_TYPE, "image/png")
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        put(MediaStore.Images.Media.RELATIVE_PATH, "${Environment.DIRECTORY_PICTURES}/MemoLanes")
                        put(MediaStore.Images.Media.IS_PENDING, 1)
                    } else {
                        val directory = File(Environment.getExternalStoragePublicDirectory(
                            Environment.DIRECTORY_PICTURES), "MemoLanes")
                        check(directory.isDirectory || directory.mkdirs()) { "Cannot create photo directory" }
                        legacyFile = File(directory, name)
                        put(MediaStore.Images.Media.DATA, legacyFile!!.absolutePath)
                    }
                }
                val destination = resolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, values)
                    ?: error("Cannot create photo")
                uri = destination
                val stream = resolver.openOutputStream(destination) ?: error("Cannot open photo")
                stream.use { it.write(png) }
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    val published = resolver.update(destination, ContentValues().apply {
                        put(MediaStore.Images.Media.IS_PENDING, 0)
                    }, null, null)
                    check(published == 1) { "Cannot publish photo" }
                }
                main.post { result.success(null) }
            } catch (error: Exception) {
                uri?.let { runCatching { resolver.delete(it, null, null) } }
                legacyFile?.let { runCatching { it.delete() } }
                main.post { result.error("save_failed", error.message, null) }
            }
        }
    }

    override fun onAttachedToActivity(binding: ActivityPluginBinding) {
        this.binding = binding
        binding.addRequestPermissionsResultListener(this)
    }

    override fun onDetachedFromActivityForConfigChanges() {
        binding?.removeRequestPermissionsResultListener(this)
        binding = null
    }

    override fun onReattachedToActivityForConfigChanges(binding: ActivityPluginBinding) {
        onAttachedToActivity(binding)
    }

    override fun onDetachedFromActivity() {
        onDetachedFromActivityForConfigChanges()
        pending?.second?.error("unavailable", "Activity closed", null)
        pending = null
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel.setMethodCallHandler(null)
        executor.shutdown()
    }
}
