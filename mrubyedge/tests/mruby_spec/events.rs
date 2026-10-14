//! Optional append-only events survive VM panics and process aborts.
use std::{cell::RefCell, fs::OpenOptions, io::Write, rc::Rc};

use mrubyedge::yamrb::{
    helpers::mrb_define_cmethod,
    value::{RObject, RValue},
    vm::VM,
};

fn output() -> Option<std::fs::File> {
    std::env::var_os("MRUBYEDGE_SPEC_EVENTS").map(|path| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("cannot open spec events")
    })
}

pub fn phase(phase: &str, label: &str) {
    if let Some(mut file) = output() {
        writeln!(file, "{}", serde_json::json!(["phase", phase, label]))
            .expect("cannot write spec phase");
    }
}

pub fn install(vm: &mut VM) {
    let file = RefCell::new(output());
    mrb_define_cmethod(
        vm,
        vm.object_class.clone(),
        "_spec_event",
        Box::new(move |_, args| {
            if let Some(file) = file.borrow_mut().as_mut() {
                let values: Vec<serde_json::Value> = args
                    .iter()
                    .map(|arg| match &arg.value {
                        RValue::Integer(n) => serde_json::json!(n),
                        RValue::String(s, _) => {
                            serde_json::json!(String::from_utf8_lossy(&s.borrow()))
                        }
                        RValue::Nil => serde_json::Value::Null,
                        _ => serde_json::json!(format!("{:?}", arg.value)),
                    })
                    .collect();
                writeln!(file, "{}", serde_json::json!(values)).expect("cannot write spec event");
                file.flush().expect("cannot flush spec event");
            }
            Ok(Rc::new(RObject::nil()))
        }),
    );
}
