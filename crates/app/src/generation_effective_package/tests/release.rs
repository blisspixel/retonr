use std::cell::RefCell;

use super::super::{run_release_and_validation_steps, run_release_steps};

#[test]
fn release_steps_always_run_cleanup_then_both_revalidations() {
    for mask in 0_u8..8 {
        let order = RefCell::new(Vec::new());
        let (cleanup, model, runtime) = run_release_steps(
            || {
                order.borrow_mut().push("cleanup");
                if mask & 1 == 0 {
                    Ok(())
                } else {
                    Err("cleanup")
                }
            },
            || {
                order.borrow_mut().push("model");
                if mask & 2 == 0 { Ok(()) } else { Err("model") }
            },
            || {
                order.borrow_mut().push("runtime");
                if mask & 4 == 0 {
                    Ok(())
                } else {
                    Err("runtime")
                }
            },
        );
        assert_eq!(*order.borrow(), ["cleanup", "model", "runtime"]);
        assert_eq!(cleanup.is_some(), mask & 1 != 0);
        assert_eq!(model.is_some(), mask & 2 != 0);
        assert_eq!(runtime.is_some(), mask & 4 != 0);
    }
}

#[test]
fn final_validation_is_typed_and_runs_only_after_every_release_gate() {
    for mask in 0_u8..16 {
        let order = RefCell::new(Vec::new());
        let (cleanup, model, runtime, validation) = run_release_and_validation_steps(
            || {
                order.borrow_mut().push("cleanup");
                if mask & 1 == 0 {
                    Ok(())
                } else {
                    Err("cleanup")
                }
            },
            || {
                order.borrow_mut().push("model");
                if mask & 2 == 0 { Ok(()) } else { Err("model") }
            },
            || {
                order.borrow_mut().push("runtime");
                if mask & 4 == 0 {
                    Ok(())
                } else {
                    Err("runtime")
                }
            },
            || {
                order.borrow_mut().push("validation");
                if mask & 8 == 0 {
                    Ok(())
                } else {
                    Err("validation")
                }
            },
        );
        let release_passed = mask.is_multiple_of(8);
        let expected_order: &[&str] = if release_passed {
            &["cleanup", "model", "runtime", "validation"]
        } else {
            &["cleanup", "model", "runtime"]
        };
        assert_eq!(*order.borrow(), expected_order);
        assert_eq!(cleanup.is_some(), mask & 1 != 0);
        assert_eq!(model.is_some(), mask & 2 != 0);
        assert_eq!(runtime.is_some(), mask & 4 != 0);
        assert_eq!(validation.is_some(), release_passed && mask & 8 != 0);
    }
}
