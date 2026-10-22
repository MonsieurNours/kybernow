//! L'objet Kotlin qui reçoit les rappels du cœur (`SessionListener`), appelé
//! depuis les fils du cœur (fil de session, fil réseau de Kyber, fils du
//! décodeur et des entrées). Chaque appel attache le fil à la JVM s'il ne
//! l'est pas (en démon : il n'a pas à s'en détacher) et travaille dans un
//! cadre de références locales, libérées au retour : ces fils ne repassent
//! jamais par Java, rien ne les libérerait sinon.

use jni::objects::{GlobalRef, JObject, JValue};
use jni::{JNIEnv, JavaVM};

pub struct Listener {
    vm: JavaVM,
    obj: GlobalRef,
}

impl Listener {
    pub fn new(env: &mut JNIEnv, obj: &JObject) -> jni::errors::Result<Self> {
        Ok(Self { vm: env.get_java_vm()?, obj: env.new_global_ref(obj)? })
    }

    /// `f` reçoit l'environnement du fil et l'objet Kotlin ; `None` si la
    /// JVM refuse ou si Kotlin a levé une exception (effacée et écrite au journal).
    pub fn with<R>(&self, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
        let mut env = self.vm.attach_current_thread_as_daemon().ok()?;
        let obj = self.obj.as_obj();
        let r = env.with_local_frame(16, |env| f(env, obj));
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
            return None;
        }
        r.ok()
    }

    /// Méthode sans retour, arguments simples.
    pub fn void(&self, name: &str, sig: &str, args: &[JValue]) {
        self.with(|env, obj| env.call_method(obj, name, sig, args).map(|_| ()));
    }
}

/// Chaîne Java facultative pour un argument.
pub fn jstring_or_null<'a>(env: &mut JNIEnv<'a>, s: Option<&str>) -> jni::errors::Result<JObject<'a>> {
    match s {
        Some(s) => Ok(env.new_string(s)?.into()),
        None => Ok(JObject::null()),
    }
}
