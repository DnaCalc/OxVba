use std::mem::{align_of, size_of};

use oxvba_com::{ComCallbackPayload, ComInvokeArg, ComValue};
use oxvba_runtime::{
    BindingHandle, CurrencyValue, F64Value, ObjectHandle, RuntimeValue, Variant,
    bstr::BStr,
    safe_array::{SafeArray, SafeArrayBound},
};

fn emit<T>(name: &str) {
    println!("{name},{},{}", size_of::<T>(), align_of::<T>());
}

fn main() {
    println!("type_name,size_bytes,align_bytes");
    emit::<String>("RustString");
    emit::<BStr>("BStr");
    emit::<RuntimeValue>("RuntimeValue");
    emit::<Variant>("Variant");
    emit::<SafeArray>("SafeArray");
    emit::<SafeArrayBound>("SafeArrayBound");
    emit::<F64Value>("F64Value");
    emit::<CurrencyValue>("CurrencyValue");
    emit::<ObjectHandle>("ObjectIdentityCarrier");
    emit::<BindingHandle>("BindingHandle");
    emit::<ComValue>("ComValue");
    emit::<ComInvokeArg>("ComInvokeArg");
    emit::<ComCallbackPayload>("ComCallbackPayload");
}
