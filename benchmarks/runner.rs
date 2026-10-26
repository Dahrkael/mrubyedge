//! Compiled against each selected mrubyedge checkout by run.py.
use mrubyedge::{
    rite,
    yamrb::{helpers::mrb_funcall, vm::VM},
};
use std::{
    hint::black_box,
    time::{Duration, Instant},
};

fn batch(f: &mut impl FnMut(), iterations: u64) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed()
}

fn measure(mut f: impl FnMut(), samples: usize, target: Duration, warmup: Duration) {
    // Check correctness even when warmup is disabled.
    f();
    let start = Instant::now();
    while start.elapsed() < warmup {
        f();
    }
    let mut iterations = 1u64;
    loop {
        let elapsed = batch(&mut f, iterations);
        if elapsed >= target / 4 || iterations >= 1_000_000 {
            iterations = ((iterations as f64 * target.as_secs_f64()
                / elapsed.as_secs_f64().max(1e-9))
            .ceil() as u64)
                .clamp(1, 1_000_000);
            break;
        }
        iterations *= 2;
    }
    for sample in 0..samples {
        let elapsed = batch(&mut f, iterations);
        println!("{sample}\t{iterations}\t{}", elapsed.as_nanos());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        7,
        "expected: file mode samples sample_ms warmup_ms expected"
    );
    let source = std::fs::read_to_string(&args[1]).expect("read workload");
    let samples: usize = args[3].parse().unwrap();
    let target = Duration::from_millis(args[4].parse().unwrap());
    let warmup = Duration::from_millis(args[5].parse().unwrap());
    let expected: i64 = args[6].parse().unwrap();
    let bin = unsafe {
        let mut context = mruby_compiler2_sys::MRubyCompiler2Context::new();
        context.compile(&source).expect("compile workload")
    };
    let invoke = |vm: &mut VM| {
        let result = mrb_funcall(vm, None, "benchmark", &[]).expect("execute workload");
        let actual: i64 = result
            .as_ref()
            .try_into()
            .expect("benchmark must return an integer");
        assert_eq!(actual, expected, "incorrect workload result");
        black_box(result);
    };
    match args[2].as_str() {
        "cold" => measure(
            || {
                let mut rite = rite::load(black_box(&bin)).expect("load bytecode");
                let mut vm = VM::open(&mut rite);
                vm.run().expect("initialize workload");
                invoke(&mut vm);
                // VM and bytecode destruction are included in this mode.
            },
            samples,
            target,
            warmup,
        ),
        "warm" => {
            let mut rite = rite::load(&bin).expect("load bytecode");
            let mut vm = VM::open(&mut rite);
            vm.run().expect("initialize workload");
            measure(|| invoke(&mut vm), samples, target, warmup);
        }
        _ => panic!("unknown measurement mode"),
    }
}
