# Le pont JNI appelle ces classes et méthodes par leur nom.
-keep class fr.leslignesclaires.kybernow.core.** { *; }

# JmDNS (secours de la découverte : adresse IPv4 d'un service annoncé).
-keep class javax.jmdns.** { *; }
-dontwarn javax.jmdns.**
-dontwarn org.slf4j.**

# Rappels du pont JNI implémentés hors de core (suivi de session, progression des mises à jour).
-keep class * implements fr.leslignesclaires.kybernow.core.** { *; }
