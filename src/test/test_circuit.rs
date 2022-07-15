use crate::circuits::{
    etable::{EventTableChip, EventTableConfig},
    imtable::InitMemoryTableConfig,
    itable::{InstructionTableChip, InstructionTableConfig},
    jtable::{JumpTableChip, JumpTableConfig},
    mtable::{MemoryTableChip, MemoryTableConfig},
    rtable::{RangeTableChip, RangeTableConfig},
    utils::Context,
};
use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{Layouter, SimpleFloorPlanner},
    plonk::{Circuit, ConstraintSystem, Error},
};
use specs::{CompileTable, ExecutionTable};
use std::marker::PhantomData;

const VAR_COLUMNS: usize = 51;

#[derive(Clone)]
pub struct TestCircuitConfig<F: FieldExt> {
    rtable: RangeTableConfig<F>,
    itable: InstructionTableConfig<F>,
    _imtable: InitMemoryTableConfig<F>,
    mtable: MemoryTableConfig<F>,
    jtable: JumpTableConfig<F>,
    etable: EventTableConfig<F>,
}

#[derive(Default)]
pub struct TestCircuit<F: FieldExt> {
    compile_tables: CompileTable,
    execution_tables: ExecutionTable,
    _data: PhantomData<F>,
}

impl<F: FieldExt> TestCircuit<F> {
    pub fn new(compile_tables: CompileTable, execution_tables: ExecutionTable) -> Self {
        TestCircuit {
            compile_tables,
            execution_tables,
            _data: PhantomData,
        }
    }
}

impl<F: FieldExt> Circuit<F> for TestCircuit<F> {
    type Config = TestCircuitConfig<F>;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let constants = meta.fixed_column();
        meta.enable_constant(constants);
        meta.enable_equality(constants);

        let mut cols = [(); VAR_COLUMNS].map(|_| meta.advice_column()).into_iter();

        let rtable = RangeTableConfig::configure([0; 3].map(|_| meta.lookup_table_column()));
        let itable = InstructionTableConfig::configure(meta.lookup_table_column());
        let imtable = InitMemoryTableConfig::configure(meta.lookup_table_column());
        let mtable = MemoryTableConfig::configure(meta, &mut cols, &rtable, &imtable);
        let jtable = JumpTableConfig::configure(meta, &mut cols, &rtable);
        let etable =
            EventTableConfig::configure(meta, &mut cols, &rtable, &itable, &mtable, &jtable);

        Self::Config {
            rtable,
            itable,
            _imtable: imtable,
            mtable,
            jtable,
            etable,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let echip = EventTableChip::new(config.etable);
        let rchip = RangeTableChip::new(config.rtable);
        let ichip = InstructionTableChip::new(config.itable);
        let mchip = MemoryTableChip::new(config.mtable);
        let jchip = JumpTableChip::new(config.jtable);

        println!("etable is {:?}", self.execution_tables.etable);
        println!();
        println!("itable is {:?}", self.compile_tables.itable);
        println!();
        println!("mtable is {:?}", self.execution_tables.mtable);
        println!();

        rchip.init(&mut layouter, 1usize << 16)?;
        ichip.assign(&mut layouter, &self.compile_tables.itable)?;

        layouter.assign_region(
            || "table",
            |region| {
                let mut ctx = Context::new(region);
                let (rest_mops_cell, rest_jops_cell) =
                    echip.assign(&mut ctx, &self.execution_tables.etable)?;

                ctx.reset();
                mchip.assign(
                    &mut ctx,
                    &self.execution_tables.mtable.entries(),
                    Some(rest_mops_cell),
                )?;

                ctx.reset();
                jchip.assign(
                    &mut ctx,
                    &self.execution_tables.jtable,
                    Some(rest_jops_cell),
                )?;
                Ok(())
            },
        )?;

        Ok(())
    }
}
