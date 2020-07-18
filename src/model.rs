use crate::parse::parse_formula;
use std::collections::HashMap;

use crate::molecule::molecular_weight;

pub struct Substance {
    formula: String,
    mass: f32,
    atoms: HashMap<String, u32>,
    molecular_weight: f32,
}

impl Substance {
    pub fn new(formula: &str, mass: f32) -> Result<Substance, String> {
        let atoms = parse_formula(formula);
        let molecular_weight = atoms.clone().and_then(|atoms| molecular_weight(atoms))?;
        Ok(Substance {
            formula: formula.to_string(),
            mass,
            atoms: atoms?,
            molecular_weight: molecular_weight,
        })
    }

    pub fn moles(self: &Self) -> f32 {
        self.mass / self.molecular_weight
    }
}

pub struct Reaction {
    pub reagent: Substance,
    pub product: Substance,
}

impl Reaction {
    pub fn new(reagent: Substance, product: Substance) -> Reaction {
        Reaction { reagent, product }
    }

    pub fn percent_yield(self: &Self) -> f32 {
        let rmoles = self.reagent.moles();
        let pmoles = self.product.moles();
        rmoles / pmoles
    }
}