# JNA / UniFFI loads native methods reflectively; keep the generated bindings.
-keep class dev.wvb.** { *; }
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { *; }
