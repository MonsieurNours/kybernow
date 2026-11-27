//! L'objet Kotlin qui reçoit les rappels du cœur (`SessionListener`), appelé
//! depuis les fils du cœur (fil de session, fil réseau de Kyber, fils du
//! décodeur et des entrées). Chaque appel attache le fil à la JVM s'il ne
//! l'est pas (en démon ; jni-rs le détache quand même à la fin du fil, par
//! un garde de stockage local, sans quoi ART arrêterait l'app à la fin de
//! chaque fil de décodage) et travaille dans un
//! cadre de références locales, libérées au retour : ces fils ne repassent
//! jamais par Java, rien ne les libérerait sinon.

use std::collections::HashMap;
use std::sync::Mutex;

use jni::objects::{GlobalRef, JMethodID, JObject, JValue};
use jni::signature::{Primitive, ReturnType};
use jni::sys::jvalue;
use jni::{JNIEnv, JavaVM};

pub struct Listener {
    vm: JavaVM,
    obj: GlobalRef,
    /// Les méthodes déjà cherchées (AND-26) : `call_method` refaisait
    /// `GetObjectClass` et `GetMethodID` à chaque rappel, pointeur compris (à
    /// la cadence de la souris). Un identifiant de méthode vaut tant que la
    /// classe est chargée, ce que garde la référence globale à l'objet.
    methods: Mutex<HashMap<(&'static str, &'static str), JMethodID>>,
}

impl Listener {
    pub fn new(env: &mut JNIEnv, obj: &JObject) -> jni::errors::Result<Self> {
        Ok(Self { vm: env.get_java_vm()?, obj: env.new_global_ref(obj)?, methods: Mutex::new(HashMap::new()) })
    }

    /// L'identifiant de `name` (`sig`) sur l'objet Kotlin, cherché une fois.
    pub fn method(&self, env: &mut JNIEnv, name: &'static str, sig: &'static str) -> jni::errors::Result<JMethodID> {
        if let Some(m) = self.methods.lock().unwrap_or_else(|e| e.into_inner()).get(&(name, sig)) {
            return Ok(*m);
        }
        let class = env.get_object_class(self.obj.as_obj())?;
        let m = env.get_method_id(&class, name, sig)?;
        self.methods.lock().unwrap_or_else(|e| e.into_inner()).insert((name, sig), m);
        Ok(m)
    }

    /// Appel d'une méthode sans retour par son identifiant gardé.
    pub fn call_void(&self, env: &mut JNIEnv, obj: &JObject, name: &'static str, sig: &'static str, args: &[JValue]) -> jni::errors::Result<()> {
        let m = self.method(env, name, sig)?;
        let raw: Vec<jvalue> = args.iter().map(|a| a.as_jni()).collect();
        // SAFETY : `m` vient de `sig`, et les arguments sont ceux de cette signature (vérifié par chaque appelant, comme
        // pour `call_method`).
        unsafe { env.call_method_unchecked(obj, m, ReturnType::Primitive(Primitive::Void), &raw) }.map(|_| ())
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
    pub fn void(&self, name: &'static str, sig: &'static str, args: &[JValue]) {
        self.with(|env, obj| self.call_void(env, obj, name, sig, args));
    }
}

/// Chaîne Java facultative pour un argument.
pub fn jstring_or_null<'a>(env: &mut JNIEnv<'a>, s: Option<&str>) -> jni::errors::Result<JObject<'a>> {
    match s {
        Some(s) => Ok(env.new_string(s)?.into()),
        None => Ok(JObject::null()),
    }
}
