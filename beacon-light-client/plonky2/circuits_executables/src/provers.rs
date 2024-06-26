use std::{str::FromStr, time::Instant};

use ark_bls12_381::{g1, g2, G1Affine, G2Affine};
use circuits::{
    biguint::WitnessBigUint, build_balance_inner_level_circuit::BalanceInnerCircuitTargets,
    build_commitment_mapper_inner_level_circuit::CommitmentMapperInnerCircuitTargets,
    build_final_circuit::FinalCircuitTargets, build_stark_proof_verifier::RecursiveStarkTargets,
    utils::SetBytesArray, validator_balance_circuit::ValidatorBalanceVerificationTargets,
    validator_hash_tree_root::ValidatorShaTargets,
    validator_hash_tree_root_poseidon::ValidatorPoseidonTargets,
};

use num_bigint::BigUint;
use plonky2::{
    field::goldilocks_field::GoldilocksField,
    iop::{
        target::BoolTarget,
        witness::{PartialWitness, WitnessWrite},
    },
    plonk::{
        circuit_data::{CircuitData, VerifierCircuitTarget},
        config::{GenericConfig, PoseidonGoldilocksConfig},
        proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget},
    },
};
use starky_bls12_381::{
    aggregate_proof::{
        calc_pairing_precomp, final_exponentiate_main, fp12_mul_main, miller_loop_main,
    },
    native::{Fp, Fp12, Fp2},
};

use crate::{
    crud::common::FinalCircuitInput,
    validator::ValidatorShaInput,
    validator_balances_input::{ValidatorBalancesInput, ValidatorPoseidonInput},
};

use anyhow::Result;

pub fn handle_generic_inner_level_proof(
    proof1_bytes: Vec<u8>,
    proof2_bytes: Vec<u8>,
    inner_circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
    proof1_target: &ProofWithPublicInputsTarget<2>,
    proof2_target: &ProofWithPublicInputsTarget<2>,
    verifier_circuit_target: &VerifierCircuitTarget,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> Result<ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2>> {
    let inner_proof1 =
        ProofWithPublicInputs::<GoldilocksField, PoseidonGoldilocksConfig, 2>::from_bytes(
            proof1_bytes,
            &inner_circuit_data.common,
        )?;

    let inner_proof2 =
        ProofWithPublicInputs::<GoldilocksField, PoseidonGoldilocksConfig, 2>::from_bytes(
            proof2_bytes,
            &inner_circuit_data.common,
        )?;

    let mut pw = PartialWitness::new();

    pw.set_proof_with_pis_target(proof1_target, &inner_proof1);
    pw.set_proof_with_pis_target(proof2_target, &inner_proof2);

    pw.set_cap_target(
        &verifier_circuit_target.constants_sigmas_cap,
        &inner_circuit_data.verifier_only.constants_sigmas_cap,
    );

    pw.set_hash_target(
        verifier_circuit_target.circuit_digest,
        inner_circuit_data.verifier_only.circuit_digest,
    );

    Ok(circuit_data.prove(pw)?)
}

pub fn handle_commitment_mapper_inner_level_proof(
    proof1_bytes: Vec<u8>,
    proof2_bytes: Vec<u8>,
    inner_circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
    inner_circuit_targets: &CommitmentMapperInnerCircuitTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> Result<ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2>> {
    handle_generic_inner_level_proof(
        proof1_bytes,
        proof2_bytes,
        inner_circuit_data,
        &inner_circuit_targets.proof1,
        &inner_circuit_targets.proof2,
        &inner_circuit_targets.verifier_circuit_target,
        circuit_data,
    )
}

pub fn handle_balance_inner_level_proof(
    proof1_bytes: Vec<u8>,
    proof2_bytes: Vec<u8>,
    inner_circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
    inner_circuit_targets: &BalanceInnerCircuitTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> Result<ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2>> {
    handle_generic_inner_level_proof(
        proof1_bytes,
        proof2_bytes,
        inner_circuit_data,
        &inner_circuit_targets.proof1,
        &inner_circuit_targets.proof2,
        &inner_circuit_targets.verifier_circuit_target,
        circuit_data,
    )
}

fn set_boolean_pw_values(
    pw: &mut PartialWitness<GoldilocksField>,
    target: &[BoolTarget],
    source: &Vec<bool>,
) {
    for i in 0..target.len() {
        pw.set_bool_target(target[i], source[i]);
    }
}

pub trait SetPWValues<T> {
    fn set_pw_values(&self, pw: &mut PartialWitness<GoldilocksField>, source: &T);
}

impl SetPWValues<ValidatorPoseidonInput> for ValidatorPoseidonTargets {
    fn set_pw_values(
        &self,
        pw: &mut PartialWitness<GoldilocksField>,
        source: &ValidatorPoseidonInput,
    ) {
        pw.set_bytes_array(&self.pubkey, &hex::decode(&source.pubkey).unwrap());

        pw.set_bytes_array(
            &self.withdrawal_credentials,
            &hex::decode(&source.withdrawal_credentials).unwrap(),
        );

        pw.set_biguint_target(&self.effective_balance, &source.effective_balance);

        pw.set_bool_target(self.slashed, source.slashed == 1);

        pw.set_biguint_target(
            &self.activation_eligibility_epoch,
            &source.activation_eligibility_epoch,
        );

        pw.set_biguint_target(&self.activation_epoch, &source.activation_epoch);

        pw.set_biguint_target(&self.exit_epoch, &source.exit_epoch);

        pw.set_biguint_target(&self.withdrawable_epoch, &source.withdrawable_epoch);
    }
}

impl<const N: usize> SetPWValues<ValidatorBalancesInput>
    for ValidatorBalanceVerificationTargets<N>
{
    fn set_pw_values(
        &self,
        pw: &mut PartialWitness<GoldilocksField>,
        source: &ValidatorBalancesInput,
    ) {
        for i in 0..self.balances.len() {
            set_boolean_pw_values(pw, &self.balances[i], &source.balances[i]);
        }

        for i in 0..self.validators.len() {
            self.validators[i].set_pw_values(pw, &source.validators[i]);
        }

        for i in 0..N {
            set_boolean_pw_values(
                pw,
                &self.withdrawal_credentials[i],
                &source.withdrawal_credentials[i],
            );
        }

        set_boolean_pw_values(pw, &self.validator_is_zero, &source.validator_is_zero);

        pw.set_biguint_target(&self.current_epoch, &source.current_epoch);
    }
}

impl SetPWValues<ValidatorShaInput> for ValidatorShaTargets {
    fn set_pw_values(&self, pw: &mut PartialWitness<GoldilocksField>, source: &ValidatorShaInput) {
        pw.set_bytes_array(&self.pubkey, &hex::decode(&source.pubkey).unwrap());

        pw.set_bytes_array(
            &self.withdrawal_credentials,
            &hex::decode(&source.withdrawal_credentials).unwrap(),
        );

        pw.set_bytes_array(
            &self.effective_balance,
            &hex::decode(&source.effective_balance).unwrap(),
        );

        pw.set_bytes_array(&self.slashed, &hex::decode(&source.slashed).unwrap());

        pw.set_bytes_array(
            &self.activation_eligibility_epoch,
            &hex::decode(&source.activation_eligibility_epoch).unwrap(),
        );

        pw.set_bytes_array(
            &self.activation_epoch,
            &hex::decode(&source.activation_epoch).unwrap(),
        );

        pw.set_bytes_array(&self.exit_epoch, &hex::decode(&source.exit_epoch).unwrap());

        pw.set_bytes_array(
            &self.withdrawable_epoch,
            &hex::decode(&source.withdrawable_epoch).unwrap(),
        );
    }
}

impl<const N: usize> SetPWValues<FinalCircuitInput> for FinalCircuitTargets<N> {
    fn set_pw_values(&self, pw: &mut PartialWitness<GoldilocksField>, source: &FinalCircuitInput) {
        set_boolean_pw_values(pw, &self.state_root, &source.state_root);

        for i in 0..source.state_root_branch.len() {
            set_boolean_pw_values(pw, &self.state_root_branch[i], &source.state_root_branch[i]);
        }

        set_boolean_pw_values(pw, &self.block_root, &source.block_root);

        pw.set_biguint_target(&self.slot, &source.slot);

        for i in 0..source.slot_branch.len() {
            set_boolean_pw_values(pw, &self.slot_branch[i], &source.slot_branch[i]);
        }

        for i in 0..N {
            set_boolean_pw_values(
                pw,
                &self.withdrawal_credentials[i],
                &source.withdrawal_credentials[i],
            );
        }

        for i in 0..source.balance_branch.len() {
            set_boolean_pw_values(pw, &self.balance_branch[i], &source.balance_branch[i]);
        }

        for i in 0..source.validators_branch.len() {
            set_boolean_pw_values(pw, &self.validators_branch[i], &source.validators_branch[i]);
        }

        set_boolean_pw_values(pw, &self.validator_size_bits, &source.validators_size_bits);
    }
}

// impl SetPWValues<BlsInput> for BlsCircuitTargets {
//     fn set_pw_values(&self, pw: &mut PartialWitness<GoldilocksField>, source: &BlsInput) {
//         pw.set_bytes_array(&self.pubkey, &hex::decode(&source.pubkey).unwrap());

//         pw.set_bytes_array(&self.signature, &hex::decode(&source.signature).unwrap());

//         pw.set_bytes_array(&self.message, &hex::decode(&source.message).unwrap());
//     }
// }

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

pub fn generate_pairing_precomp_proof(
    g2: &G2Affine,
    pairing_precomp_targets: &RecursiveStarkTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2> {
    println!("Starting Pairing precomp starky proof");

    let s = Instant::now();
    let (_, proof_pp, _) = calc_pairing_precomp::<F, C, D>(
        Fp2([
            Fp::get_fp_from_biguint(g2.x.c0.to_string().parse::<BigUint>().unwrap()),
            Fp::get_fp_from_biguint(g2.x.c1.to_string().parse::<BigUint>().unwrap()),
        ]),
        Fp2([
            Fp::get_fp_from_biguint(g2.y.c0.to_string().parse::<BigUint>().unwrap()),
            Fp::get_fp_from_biguint(g2.y.c1.to_string().parse::<BigUint>().unwrap()),
        ]),
        Fp2([
            Fp::get_fp_from_biguint(BigUint::from_str("1").unwrap()),
            Fp::get_fp_from_biguint(BigUint::from_str("0").unwrap()),
        ]),
    );

    println!("Pairing precomp starky proof done {:?}", s.elapsed());

    let mut pw = PartialWitness::new();
    starky::recursive_verifier::set_stark_proof_with_pis_target(
        &mut pw,
        &pairing_precomp_targets.proof,
        &proof_pp,
        pairing_precomp_targets.zero,
    );

    println!("Starting to generate plonky2 proof");
    let s = Instant::now();
    let proof = circuit_data.prove(pw).unwrap();
    println!("time taken for plonky2 recursive proof {:?}", s.elapsed());

    proof
}

pub fn generate_miller_loop_proof(
    g1: &G1Affine,
    g2: &G2Affine,
    miller_loop_targets: &RecursiveStarkTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2> {
    println!("Starting Miller Loop Proving");

    let s = Instant::now();

    let (_, proof_ml, _) = miller_loop_main::<F, C, D>(
        Fp::get_fp_from_biguint(g1.x.to_string().parse::<BigUint>().unwrap()),
        Fp::get_fp_from_biguint(g1.y.to_string().parse::<BigUint>().unwrap()),
        Fp2([
            Fp::get_fp_from_biguint(g2.x.c0.to_string().parse::<BigUint>().unwrap()),
            Fp::get_fp_from_biguint(g2.x.c1.to_string().parse::<BigUint>().unwrap()),
        ]),
        Fp2([
            Fp::get_fp_from_biguint(g2.y.c0.to_string().parse::<BigUint>().unwrap()),
            Fp::get_fp_from_biguint(g2.y.c1.to_string().parse::<BigUint>().unwrap()),
        ]),
        Fp2([
            Fp::get_fp_from_biguint(BigUint::from_str("1").unwrap()),
            Fp::get_fp_from_biguint(BigUint::from_str("0").unwrap()),
        ]),
    );

    println!("Miller Loop Proving Done {:?}", s.elapsed());

    let mut pw = PartialWitness::new();
    starky::recursive_verifier::set_stark_proof_with_pis_target(
        &mut pw,
        &miller_loop_targets.proof,
        &proof_ml,
        miller_loop_targets.zero,
    );

    println!("Starting to generate proof");

    let s = Instant::now();
    let proof = circuit_data.prove(pw).unwrap();
    println!("time taken for plonky2 recursive proof {:?}", s.elapsed());

    proof
}

pub fn generate_fp12_mul_proof(
    miller_loop1: &Fp12,
    miller_loop2: &Fp12,
    fp12_mul_targets: &RecursiveStarkTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2> {
    let s = Instant::now();
    println!("Starting FP12 Mul Proving");
    let (_, proof_fp12_mul, _) = fp12_mul_main::<F, C, D>(*miller_loop1, *miller_loop2);
    println!("FP12 Mul Proving Done {:?}", s.elapsed());

    let mut pw = PartialWitness::new();
    starky::recursive_verifier::set_stark_proof_with_pis_target(
        &mut pw,
        &fp12_mul_targets.proof,
        &proof_fp12_mul,
        fp12_mul_targets.zero,
    );

    println!("Starting to generate proof");

    let s = Instant::now();
    let proof = circuit_data.prove(pw).unwrap();
    println!("time taken for plonky2 recursive proof {:?}", s.elapsed());

    proof
}

pub fn generate_final_exponentiate(
    fp12_mul: &Fp12,
    final_exponentiate_targets: &RecursiveStarkTargets,
    circuit_data: &CircuitData<GoldilocksField, PoseidonGoldilocksConfig, 2>,
) -> ProofWithPublicInputs<GoldilocksField, PoseidonGoldilocksConfig, 2> {
    println!("Final exponetiate stark proof started");

    let s = Instant::now();

    let (_, proof_final_exp, _) = final_exponentiate_main::<F, C, D>(*fp12_mul);

    println!("Final exponetiate stark proof done in {:?}", s.elapsed());

    let mut pw = PartialWitness::new();
    starky::recursive_verifier::set_stark_proof_with_pis_target(
        &mut pw,
        &final_exponentiate_targets.proof,
        &proof_final_exp,
        final_exponentiate_targets.zero,
    );

    println!("Starting to generate proof");

    let s = Instant::now();
    let proof = circuit_data.prove(pw).unwrap();
    println!("time taken for plonky2 recursive proof {:?}", s.elapsed());

    proof
}
