use melior::{
    Context, ExecutionEngine,
    dialect::{DialectRegistry, arith, func},
    ir::{
        attribute::{StringAttribute, TypeAttribute},
        operation::OperationLike,
        r#type::FunctionType,
        *,
    },
    pass::{self, PassIrPrintingOptions, PassManager},
    utility::register_all_dialects,
};
use operation::OperationPrintingFlags;
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let registry = DialectRegistry::new();
    register_all_dialects(&registry);

    let context = Context::new();
    context.append_dialect_registry(&registry);
    context.enable_multi_threading(false); // necessary for printing C++ moment
    context.load_all_available_dialects();

    let location = Location::unknown(&context);
    let mut module = Module::new(location);

    let double_type = Type::float64(&context);

    module.body().append_operation(func::func(
        &context,
        StringAttribute::new(&context, "add"),
        TypeAttribute::new(
            FunctionType::new(&context, &[double_type, double_type], &[double_type]).into(),
        ),
        {
            let block = Block::new(&[(double_type, location), (double_type, location)]);

            let sum = block
                .append_operation(arith::addf(
                    block.argument(0).unwrap().into(),
                    block.argument(1).unwrap().into(),
                    location,
                ))
                .result(0)
                .unwrap();

            block.append_operation(func::r#return(&[sum.into()], location));

            let region = Region::new();
            region.append_block(block);
            region
        },
        &[(
            Identifier::new(&context, "llvm.emit_c_interface"),
            Attribute::unit(&context).into(),
        )],
        location,
    ));

    let pass_manager = PassManager::new(&context);
    pass_manager.enable_verifier(true);
    pass_manager.add_pass(pass::transform::create_canonicalizer());
    pass_manager.add_pass(pass::conversion::create_control_flow_to_llvm());
    pass_manager.add_pass(pass::conversion::create_to_llvm());
    pass_manager.enable_ir_printing(&PassIrPrintingOptions {
        before_all: true,
        after_all: true,
        module_scope: true,
        on_change: true,
        on_failure: true,
        flags: OperationPrintingFlags::new(),
        tree_printing_path: PathBuf::from("all_pass"),
    });
    pass_manager.run(&mut module).unwrap();

    assert!(module.as_operation().verify());
    let engine = ExecutionEngine::new(&module, 3, &[], true);

    let mut argument1: f64 = 2.0;
    let mut argument2: f64 = 4.0;
    let mut result: f64 = -1.0;

    unsafe {
        engine.invoke_packed(
            "add",
            &mut [
                (&mut argument1) as *mut f64 as _,
                (&mut argument2) as *mut f64 as _,
                (&mut result) as *mut f64 as _,
            ],
        )?;
    };

    assert_eq!(result, 6.0);
    println!("{} + {} = {}", argument1, argument2, result);
    Ok(())
}
