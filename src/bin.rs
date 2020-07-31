extern crate clap;
#[macro_use]
extern crate log;

use clap::Clap;
use itertools::Itertools;

use std::fs::read_to_string;
use stoichkit::ext::parse_chemdraw_reaction;
use stoichkit::model::{Reaction, Substance};
use stoichkit::solve::balance;

#[derive(Clap)]
#[clap(version = "0.2.0")]
struct Cli {
    #[clap(subcommand)]
    command: Subcommand,
}

#[derive(Clap)]
struct ReactionList {
    #[clap(
        about = "Fully balanced chemical reaction list: [<coeff>*]<formula> <grams>, coeff defaults to 1"
    )]
    substances: Vec<String>,
}

#[derive(Clap)]
enum Subcommand {
    Yield(ReactionList),
    Balance(Unbal),
}

#[derive(Clap)]
struct Unbal {
    #[clap(about = "Chemical equation [...reactants] = [...products]")]
    substances: Vec<String>,
    #[clap(short, conflicts_with = "substances")]
    chemdraw_file: Option<String>,
}

impl Unbal {
    pub fn balance(&self) -> Result<String, String> {
        let (reagents, products) = match &self.chemdraw_file {
            None => {
                let reagent_input: Result<Vec<Substance>, _> = self
                    .substances
                    .iter()
                    .take_while(|a| a.as_str() != "=")
                    .map(|f| Substance::from_formula(f.as_str()))
                    .collect();
                let rx_len = reagent_input.clone()?.len() + 1;
                let product_input: Result<Vec<Substance>, _> = self
                    .substances
                    .iter()
                    .dropping(rx_len)
                    .map(|f| Substance::from_formula(f.as_str()))
                    .collect();
                match (
                    reagent_input.clone()?.len(),
                    product_input.clone()?.len(),
                ) {
                    (x, y) if x >= 1 as usize && y >= 1 as usize => Ok(()),
                    _ => Err("Must provide at least 1 reactant and 1 product"),
                }?;
                (reagent_input?, product_input?)
            }
            Some(f) => {
                let s = read_to_string(f)
                    .map_err(|_e| format!("Could not read file {:?}", f))?;
                let result = parse_chemdraw_reaction(s.as_str())?;
                info!(
                    "Parsed reaction {:?} = {:?}",
                    result.reactants, result.products
                );
                (result.reactants, result.products)
            }
        };
        let (reagents, products) = balance(reagents, products)?;
        let balr: Vec<String> = reagents
            .iter()
            .map(|(e, c)| format!("{} {}", c, e))
            .collect();
        let balp: Vec<String> = products
            .iter()
            .map(|(e, c)| format!("{} {}", c, e))
            .collect();
        let result = format!("{} = {}", balr.join(" + "), balp.join(" + "));
        Ok(result)
    }
}

impl ReactionList {
    pub fn reaction(&self) -> Result<Reaction, String> {
        let mut substances: Vec<Substance> = vec![];
        for (i, pair) in
            self.substances.chunks(2).map(|c| c.to_vec()).enumerate()
        {
            if pair.len() < 2 {
                return Err(format!(
                    "Got substance with no mass at position {}",
                    i + 1
                ));
            }
            let stoich: Vec<&str> = pair[0].as_str().split('*').collect();
            let (coeff, formula): (Option<u32>, &str) = match stoich.len() {
                1 => (None, pair[0].as_str()),
                2 => (
                    Some(stoich.first().unwrap().parse::<u32>().map_err(
                        |_| {
                            format!(
                                "Invalid coeffcieint {} for substance {}",
                                stoich.first().unwrap(),
                                pair[0]
                            )
                        },
                    )?),
                    stoich.iter().cloned().last().unwrap(),
                ),
                _ => {
                    return Err(format!(
                        "Invalid formula {} at position {}",
                        pair[0],
                        i + 1
                    ))
                }
            };
            let substance = Substance::new(
                formula,
                pair[1].clone().parse::<f32>().map_err(|_| {
                    format!(
                        "Invalid mass {} for substance {}",
                        pair[1], pair[0]
                    )
                })?,
                coeff,
            )?;
            substances.push(substance);
        }
        let (product, reagents) = substances
            .split_last()
            .ok_or_else(|| "Invalid substance list!".to_string())?;
        Ok(Reaction::new(reagents.to_vec(), product.to_owned()))
    }
}

fn main() {
    env_logger::Builder::from_env("STOICHKIT_LOG").init();
    let opts: Cli = Cli::parse();
    match opts.command {
        Subcommand::Yield(r) => match r.reaction().map(|r| r.percent_yield()) {
            Ok(yld) => println!("Yield: {:?}", yld),
            Err(msg) => println!("ERROR: {:?}", msg),
        },
        Subcommand::Balance(u) => {
            let result = u.balance().unwrap_or_else(|e| e);
            println!("{}", result);
        }
    }
}
