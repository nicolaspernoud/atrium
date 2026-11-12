package fr.ninico.atrium

import android.app.DownloadManager
import android.content.Intent
import android.media.MediaScannerConnection
import android.net.Uri
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File

class MainActivity : FlutterActivity() {
    private val CHANNEL = "fr.ninico.atrium/file_explorer"

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL).setMethodCallHandler { call, result ->
            when (call.method) {
                "scanFile" -> {
                    val path = call.argument<String>("path")
                    if (path != null) {
                        scanMediaFile(path)
                        result.success(true)
                    } else {
                        result.error("INVALID_PATH", "Path is null", null)
                    }
                }
                "openFileExplorer" -> {
                    val path = call.argument<String>("path")
                    val success = openExplorer(path)
                    result.success(success)
                }
                else -> result.notImplemented()
            }
        }
    }

    private fun scanMediaFile(filePath: String) {
        try {
            MediaScannerConnection.scanFile(
                applicationContext,
                arrayOf(filePath),
                null,
                null
            )
        } catch (e: Exception) {
            e.printStackTrace()
        }
    }

    private fun openExplorer(filePath: String?): Boolean {
        if (filePath != null) {
            scanMediaFile(filePath)
            // Try opening directory directly if a file manager supports resource/folder intent
            try {
                val file = File(filePath)
                val parentDir = file.parentFile
                if (parentDir != null && parentDir.exists()) {
                    val uri = Uri.parse(parentDir.absolutePath)
                    val folderIntent = Intent(Intent.ACTION_VIEW).apply {
                        setDataAndType(uri, "resource/folder")
                        flags = Intent.FLAG_ACTIVITY_NEW_TASK
                    }
                    if (folderIntent.resolveActivity(packageManager) != null) {
                        startActivity(folderIntent)
                        return true
                    }
                }
            } catch (e: Exception) {
                // proceed to next fallback
            }
        }

        // 1. Try DownloadManager.ACTION_VIEW_DOWNLOADS
        // This opens the system Downloads view (handled by Files by Google, Samsung My Files, etc.)
        try {
            val downloadIntent = Intent(DownloadManager.ACTION_VIEW_DOWNLOADS).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK
            }
            startActivity(downloadIntent)
            return true
        } catch (e: Exception) {
            e.printStackTrace()
        }

        // 2. Try to launch a known file manager app
        val fileManagerPackages = arrayOf(
            "com.google.android.apps.nbu.files",   // Files by Google
            "com.sec.android.app.myfiles",         // Samsung My Files
            "com.google.android.documentsui",      // Pixel / AOSP Files
            "com.android.documentsui",             // System Documents UI
            "com.mi.android.globalFileexplorer",   // Xiaomi File Manager
            "com.huawei.hidisk",                   // Huawei Files
            "com.coloros.filemanager"              // Oppo / Realme
        )

        for (pkg in fileManagerPackages) {
            try {
                val launchIntent = packageManager.getLaunchIntentForPackage(pkg)
                if (launchIntent != null) {
                    launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    startActivity(launchIntent)
                    return true
                }
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }

        return false
    }
}
