use anyhow::Result;
use jni::{objects::{JObject, JValue}, sys::jobject, JNIEnv};

/// Reflect all declared fields of a Java object into (name, value) pairs.
/// `raw` must be a valid JNI global reference (kept alive by the caller).
pub fn reflect_fields(env: &mut JNIEnv, raw: jobject) -> Result<Vec<(String, String)>> {
    let obj = unsafe { JObject::from_raw(raw) };

    let class = env.call_method(&obj, "getClass", "()Ljava/lang/Class;", &[])?.l()?;
    let fields_arr = env
        .call_method(&class, "getDeclaredFields", "()[Ljava/lang/reflect/Field;", &[])?
        .l()?;

    let arr_cls = env.find_class("java/lang/reflect/Array")?;
    let len = env
        .call_static_method(
            &arr_cls,
            "getLength",
            "(Ljava/lang/Object;)I",
            &[JValue::Object(&fields_arr)],
        )?
        .i()?;

    let mut result = Vec::with_capacity(len as usize);

    for i in 0..len {
        let field = match env.call_static_method(
            &arr_cls,
            "get",
            "(Ljava/lang/Object;I)Ljava/lang/Object;",
            &[JValue::Object(&fields_arr), JValue::Int(i)],
        ) {
            Ok(v) => match v.l() {
                Ok(o) if !o.is_null() => o,
                _ => continue,
            },
            Err(_) => {
                let _ = env.exception_clear();
                continue;
            }
        };

        // setAccessible bypasses field access restrictions (Field.get on private fields)
        let _ = env.call_method(&field, "setAccessible", "(Z)V", &[JValue::Bool(1)]);
        if env.exception_check()? {
            env.exception_clear()?;
        }

        let name_obj = match env.call_method(&field, "getName", "()Ljava/lang/String;", &[]) {
            Ok(v) => match v.l() {
                Ok(o) if !o.is_null() => o,
                _ => continue,
            },
            Err(_) => {
                let _ = env.exception_clear();
                continue;
            }
        };
        let name: String = env.get_string((&name_obj).into())?.into();

        let obj_again = unsafe { JObject::from_raw(raw) };
        let value = match env.call_method(
            &field,
            "get",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[JValue::Object(&obj_again)],
        ) {
            Ok(v) => match v.l() {
                Ok(vo) if !vo.is_null() => jobj_to_str(env, &vo),
                _ => "null".to_owned(),
            },
            Err(_) => {
                let _ = env.exception_clear();
                "<inaccessible>".to_owned()
            }
        };

        result.push((name, value));
    }

    Ok(result)
}

fn jobj_to_str<'local>(env: &mut JNIEnv<'local>, obj: &JObject<'local>) -> String {
    match env.call_method(obj, "toString", "()Ljava/lang/String;", &[]) {
        Ok(v) => match v.l() {
            Ok(s) if !s.is_null() => match env.get_string((&s).into()) {
                Ok(js) => {
                    let s: String = js.into();
                    if s.len() > 256 {
                        format!("{}…", &s[..253])
                    } else {
                        s
                    }
                }
                Err(_) => String::new(),
            },
            _ => "null".to_owned(),
        },
        Err(_) => {
            let _ = env.exception_clear();
            "<error>".to_owned()
        }
    }
}
