use std::cmp::max;
use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use nalgebra::DMatrix;
use num::integer::lcm;
use rug::Rational;

use crate::model::{BalancedReaction, Compound, Element, Reactant};

pub fn balance(
    reagents: Vec<Compound>,
    products: Vec<Compound>,
) -> Result<BalancedReaction, String> {
    let mut reagent_atoms: HashSet<&Element> = HashSet::new();
    let mut product_atoms: HashSet<&Element> = HashSet::new();
    for r in &reagents {
        for e in r.atoms.keys() {
            reagent_atoms.insert(e);
        }
    }
    for p in &products {
        for e in p.atoms.keys() {
            product_atoms.insert(e);
        }
    }
    if !&reagent_atoms.eq(&product_atoms) {
        let missing_products: HashSet<_> =
            reagent_atoms.difference(&product_atoms).collect();
        let missing_reagents: HashSet<_> =
            product_atoms.difference(&reagent_atoms).collect();
        return Err(format!(
            "Equation cannot be balanced. Reagent elements that are not in products = {:?}. Product elements that are not in products = {:?}",
            missing_products, missing_reagents)
        );
    }
    let all_atoms: Vec<&Element> = reagent_atoms
        .iter()
        .chain(product_atoms.iter())
        .cloned()
        .collect();
    let elements: Vec<&Element> = all_atoms.iter().cloned().collect();
    let mut all_compounds: Vec<Compound> = Vec::new();
    let mut matrix: Vec<f64> = Vec::new();
    debug!("Building matrix");
    let mut push_atom = |input_compounds: &Vec<&Compound>,
                         element: &Element| {
        for compound in input_compounds {
            all_compounds.push(compound.to_owned().to_owned());
            let coefficient =
                compound.atoms.get(element).cloned().unwrap_or(0 as u32);
            trace!(
                "Pushing {:?}*{:?} from {:?}",
                coefficient,
                element,
                compound
            );
            matrix.push(coefficient as f64);
        }
    };
    for element in all_atoms.iter() {
        debug!("Getting coefficients for {:?}", element);
        push_atom(&reagents.iter().collect(), element);
        push_atom(&products.iter().collect(), element);
    }
    debug!("Constructing matrix");
    let mx = DMatrix::from_row_slice(
        elements.len(),
        reagents.len() + products.len(),
        matrix.as_slice(),
    );
    let c = reagents.len() + products.len() - 1;
    let (a, b) = mx.columns_range_pair(0..c, c..);
    let x = a.svd(true, true);
    debug!("Solving equation system");
    let solution = x.solve(&b, 0.0)?;
    debug!("Solution: {:?}", solution);
    let coefficients: Vec<f32> =
        solution.column(0).iter().map(|c| *c as f32).collect();
    debug!("Got solution coefficients: {:?}", &coefficients);
    trace!("Converting to rationals");
    let rational_coeffs: Vec<Rational> = coefficients
        .iter()
        .cloned()
        .map(|c| {
            trace!("Constructing rational from {:?}", &c);
            Rational::from_f32(c).ok_or_else(|| {
                format!("Could not construct rational from: {:?}", &c)
            })
        })
        .collect::<Result<Vec<Rational>, String>>()?;
    trace!("Got rational coefficients: {:?}", &rational_coeffs);
    trace!("Limiting denominators");
    let rational_limited: Vec<Rational> = rational_coeffs
        .iter()
        .cloned()
        .map(|r| limit_denominator(&r.abs(), 100))
        .collect::<Result<Vec<Rational>, String>>()?;
    trace!("Limited rational coefficients: {:?}", rational_limited);
    let denominators: Vec<u64> = rational_limited
        .iter()
        .cloned()
        .map(|c| {
            c.denom().to_u64().ok_or_else(|| {
                format!(
                    "Could not convert denominator {:?} to u64!",
                    &c.denom()
                )
            })
        })
        .collect::<Result<Vec<u64>, String>>()?;
    trace!("Got denominators: {:?}", &denominators);
    let scale: u64 = denominators
        .iter()
        .cloned()
        .combinations(2)
        .fold(1, |acc, cur| max(acc, lcm(cur[0] as u64, cur[1] as u64)));
    debug!("Scaling coefficients by: {}", scale);
    let mut scaled_coeffs: Vec<u64> = rational_limited
        .iter()
        .map(|f| f * Rational::from((scale, 1)))
        .map(|f| {
            f.numer().to_u64().ok_or_else(|| {
                format!("Could not convert scaled {:?} to u64", &f.numer())
            })
        })
        .collect::<Result<Vec<u64>, String>>()?;
    scaled_coeffs.push(scale);
    trace!("Got scaled coefficients: {:?}", scaled_coeffs);
    let result: Vec<Reactant> = all_compounds
        .iter()
        .map(|compound| compound.to_owned())
        .zip(&mut scaled_coeffs.iter().map(|c| c.to_owned()))
        .map(|(c, coeff)| Reactant::of_compound(c, coeff as u32))
        .collect();
    let (reagents_result, products_result) = result.split_at(reagents.len());
    if check_balance(reagents_result, products_result)? {
        let reaction = BalancedReaction::new(
            reagents_result.to_vec(),
            products_result.to_vec(),
        );
        Ok(reaction)
    } else {
        Err(format!("Equation could not be balanced!"))
    }
}

fn check_balance(
    reactants: &[Reactant],
    products: &[Reactant],
) -> Result<bool, String> {
    let react_elems: HashMap<Element, u64> = reactants
        .iter()
        .map(|s| (&s.compound.atoms, s.molar_coefficient))
        .fold(HashMap::new(), |mut acc, (item, coeff)| {
            for (e, c) in item {
                let counter = acc.entry(e.to_owned()).or_insert(0);
                *counter += *c as u64 * coeff as u64;
            }
            acc
        });
    let prod_elems: HashMap<Element, u64> = products
        .iter()
        .map(|s| (&s.compound.atoms, s.molar_coefficient))
        .fold(HashMap::new(), |mut acc, (item, coeff)| {
            for (e, c) in item {
                let counter = acc.entry(e.to_owned()).or_insert(0);
                *counter += *c as u64 * coeff as u64;
            }
            acc
        });
    debug!(
        "Checking balanced?: Reagent elements: {:?} === Product elements: {:?}",
        react_elems, prod_elems
    );
    Ok(react_elems.eq(&prod_elems))
}

fn limit_denominator(
    given: &Rational,
    max_denominator: u64,
) -> Result<Rational, String> {
    debug!(
        "Limiting denominator for {:?} to at most {:?}",
        given, max_denominator
    );
    if max_denominator < 1 || given.denom() <= &max_denominator {
        Ok(given.to_owned())
    } else {
        let (mut p0, mut q0, mut p1, mut q1) = (0u64, 1u64, 1u64, 0u64);
        let (mut n, mut d) = (
            given
                .numer()
                .to_u64()
                .ok_or_else(|| format!("No numerator for: {:?}", given))?,
            given
                .denom()
                .to_u64()
                .ok_or_else(|| format!("No denominator for: {:?}", given))?,
        );
        let mut a: u64;
        let mut q2: u64;
        loop {
            a = n / d;
            q2 = q0 + (a * q1);
            if q2 > max_denominator {
                break;
            }
            let p0_new: u64 = p1;
            let q0_new: u64 = q1;
            let p1_new: u64 = p0 + (a * p1);
            let q1_new: u64 = q2;
            p0 = p0_new;
            q0 = q0_new;
            p1 = p1_new;
            q1 = q1_new;
            let n_new: u64 = d;
            let d_new: u64 = n - (a * d);
            n = n_new;
            d = d_new;
        }
        let k = (max_denominator - q0) / q1;
        let bound1: Rational = Rational::from((p0 + k * p1, q0 + k * q1));
        let bound2: Rational = Rational::from((p1, q1));
        let d1f: Rational = bound1.clone() - given;
        let d2f: Rational = bound2.clone() - given;
        if d1f.abs() <= d2f.abs() {
            Ok(bound1)
        } else {
            Ok(bound2)
        }
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use crate::model::*;
    use crate::solve::balance;

    macro_rules! parse_balanced_reagent {
        (($subst:tt, $coef: tt)) => {
            Reactant::from_formula(stringify!($subst), $coef).unwrap()
        };
        ($subst:tt) => {
            Reactant::from_formula(stringify!($subst), 1).unwrap()
        };
    }

    /**
       e.x. expect_balanced!(H2O = H2 + O2 => (H2O, 2) = (H2, 2) + O2)
    */
    macro_rules! expect_balanced {
        ($reactSubst:tt $( + $reactSubstTail:tt)* = $prodSubst:tt $( + $prodSubstTail:tt)* => $expectedReactant:tt $( + $expectedReactantTail:tt)* = $expectedProduct:tt $( + $expectedProductTail:tt)*) => {
            // Al + Cl2 = AlCl3
            let reagents = vec![stringify!($reactSubst) $(,stringify!($reactSubstTail))*];
            let products = vec![stringify!($prodSubst) $(,stringify!($prodSubstTail))*];
            let exp_reag = vec![parse_balanced_reagent!($expectedReactant) $(, parse_balanced_reagent!($expectedReactantTail))*];
            let exp_prod = vec![parse_balanced_reagent!($expectedProduct) $(, parse_balanced_reagent!($expectedProductTail))*];
            let solution = balance(
                _formulas_to_compounds(reagents),
                _formulas_to_compounds(products),
            )
            .unwrap();
            assert_eq!(solution, BalancedReaction::new(exp_reag, exp_prod))
        };
    }

    fn _formulas_to_compounds(formulas: Vec<&str>) -> Vec<Compound> {
        formulas
            .iter()
            .cloned()
            .map(|f| Compound::from_formula(f).unwrap())
            .collect()
    }

    #[test]
    fn test_AlCl3() {
        expect_balanced!(
            Al + Cl2 = AlCl3 =>
                (Al, 2) + (Cl2, 3) = (AlCl3, 2)
        );
    }

    #[test]
    // C6H5COOH + O2 = CO2 + H2O
    fn test_CO2() {
        expect_balanced!(
            C6H5COOH + O2 = CO2 + H2O =>
                (C6H5COOH, 2) + (O2, 15) = (CO2, 14) + (H2O, 6)
        );
    }

    #[test]
    // KMnO4 + HCl = KCl + MnCl2 + H2O + Cl2
    fn test_KMnO4() {
        expect_balanced!(
            KMnO4 + HCl = KCl + MnCl2 + H2O + Cl2 =>
                (KMnO4, 2) + (HCl, 16) = (KCl, 2) + (MnCl2, 2) + (H2O, 8) + (Cl2, 5)
        );
    }

    #[test]
    fn test_H2O() {
        expect_balanced!(
            H2 + O2 = H2O =>
                (H2, 2) + (O2, 1) = (H2O, 2)
        );
    }

    #[test]
    fn test_missing_products() {
        //Fe3 + Cl5 = Cl2Fe5H2O
        let rg = vec!["Fe3", "Cl5"];
        let pd = vec!["Cl2Fe5H2O"];
        let result =
            balance(_formulas_to_compounds(rg), _formulas_to_compounds(pd));
        assert!(
            result.is_err(),
            format!("Balance solution was not Err: {:?}", result),
        )
    }

    #[test]
    fn test_impossible_reaction() {
        // H2O + NO2 = HNO3
        let rg = vec!["H2O", "NO2"];
        let pd = vec!["HNO3"];
        let result =
            balance(_formulas_to_compounds(rg), _formulas_to_compounds(pd));
        assert!(
            result.is_err(),
            format!("Balance solution was not Err: {:?}", result),
        )
    }
}
