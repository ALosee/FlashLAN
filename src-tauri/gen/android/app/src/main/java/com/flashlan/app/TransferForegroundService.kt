package com.flashlan.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder

/**
 * Foreground service that keeps the process alive while FlashLAN is running so
 * the TCP transfer server on port 17321 keeps accepting incoming files when
 * the app is backgrounded or the screen is off.
 */
class TransferForegroundService : Service() {

  override fun onCreate() {
    super.onCreate()
    startAsForeground()
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    // START_STICKY: restart after being killed by memory pressure.
    return START_STICKY
  }

  override fun onBind(intent: Intent?): IBinder? = null

  private fun startAsForeground() {
    val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      val channel =
          NotificationChannel(CHANNEL_ID, "文件传输", NotificationManager.IMPORTANCE_LOW).apply {
            description = "保持 FlashLAN 可随时接收文件"
            setShowBadge(false)
          }
      manager.createNotificationChannel(channel)
    }
    val notification =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
          Notification.Builder(this, CHANNEL_ID)
              .setContentTitle("FlashLAN 正在运行")
              .setContentText("局域网内其他设备可向本机发送文件")
              .setSmallIcon(android.R.drawable.stat_sys_download)
              .setOngoing(true)
              .build()
        } else {
          @Suppress("DEPRECATION")
          Notification.Builder(this)
              .setContentTitle("FlashLAN 正在运行")
              .setContentText("局域网内其他设备可向本机发送文件")
              .setSmallIcon(android.R.drawable.stat_sys_download)
              .setOngoing(true)
              .build()
        }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
    } else {
      startForeground(NOTIFICATION_ID, notification)
    }
  }

  companion object {
    private const val CHANNEL_ID = "flashlan-transfer"
    private const val NOTIFICATION_ID = 17321

    /** Idempotent: safe to call from every activity launch. */
    fun start(context: Context) {
      val intent = Intent(context, TransferForegroundService::class.java)
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        context.startForegroundService(intent)
      } else {
        context.startService(intent)
      }
    }
  }
}
