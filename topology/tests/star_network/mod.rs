//! A star-connected resistor network, modelled exactly like the hardware behaves, so the
//! solver can be driven from known resistances in tests. Ported from the simulator's
//! `src/core/network/topology.ts`.
//!
//! All resistances are in kΩ and all currents in mA, as in the solver itself.

use expad_topology::{ARM_COUNT, ArmDrive};

/// Rail an arm is driven to, or none at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    High,
    Low,
    Floating,
}

pub struct StarNetwork {
    arm_resistances: [f32; ARM_COUNT],
    pull_resistances: [f32; ARM_COUNT],
    low_rail: f32,
    high_rail: f32,
}

impl StarNetwork {
    /// A network of three arms, driven through 1 kΩ from a 0 V and a 3.3 V rail.
    pub fn new(arm_resistances: [f32; ARM_COUNT]) -> Self {
        Self {
            arm_resistances,
            pull_resistances: [1.0; ARM_COUNT],
            low_rail: 0.0,
            high_rail: 3.3,
        }
    }

    pub fn with_pull_resistances(mut self, pull_resistances: [f32; ARM_COUNT]) -> Self {
        self.pull_resistances = pull_resistances;
        self
    }

    pub fn with_rails(mut self, low_rail: f32, high_rail: f32) -> Self {
        self.low_rail = low_rail;
        self.high_rail = high_rail;
        self
    }

    /// What the driver of `arm` contributes while it is driven to `role`, as the solver's
    /// rail calibration would establish it: by driving that arm alone, leaving the other
    /// two floating so no current flows and its tap reads its own rail exactly.
    pub fn drive(&self, arm: usize, role: Role) -> ArmDrive {
        let mut roles = [Role::Floating; ARM_COUNT];
        roles[arm] = role;

        ArmDrive {
            rail_voltage: self.tap_voltages(roles)[arm],
            pull_resistance: self.pull_resistances[arm],
        }
    }

    /// Voltage each arm's tap settles at while the arms are driven to `roles`.
    pub fn tap_voltages(&self, roles: [Role; ARM_COUNT]) -> [f32; ARM_COUNT] {
        let star_point = self.star_point_voltage(roles);

        core::array::from_fn(|arm| {
            let resistance = self.arm_resistances[arm];

            let Some(rail) = self.rail_voltage(roles[arm]) else {
                // A floating arm only settles at the star point voltage if it has a
                // conductive path to it; an isolated arm's tap is undefined.
                return if resistance.is_finite() {
                    star_point
                } else {
                    f32::NAN
                };
            };

            // The current from this arm's rail to the star point drops across its pull
            // resistor, so its tap reads slightly off the rail. An isolated arm carries no
            // current no matter what the star point does, even if that is nothing at all.
            let current = if resistance.is_finite() {
                (rail - star_point) / (self.pull_resistances[arm] + resistance)
            } else {
                0.0
            };

            rail - current * self.pull_resistances[arm]
        })
    }

    /// Voltage the driven arms divide down to at the point where all three meet. Floating
    /// arms carry no current, so they have no say in it.
    fn star_point_voltage(&self, roles: [Role; ARM_COUNT]) -> f32 {
        let mut conductance_sum = 0.0;
        let mut weighted_voltage_sum = 0.0;

        for (arm, &role) in roles.iter().enumerate() {
            let Some(rail) = self.rail_voltage(role) else {
                continue;
            };

            // The pull resistor sits in series with the arm on the path to the star point,
            // so it lowers that path's conductance the same way the arm's own resistance
            // does.
            let conductance = 1.0 / (self.pull_resistances[arm] + self.arm_resistances[arm]);
            conductance_sum += conductance;
            weighted_voltage_sum += rail * conductance;
        }

        if conductance_sum == 0.0 {
            f32::NAN
        } else {
            weighted_voltage_sum / conductance_sum
        }
    }

    fn rail_voltage(&self, role: Role) -> Option<f32> {
        match role {
            Role::High => Some(self.high_rail),
            Role::Low => Some(self.low_rail),
            Role::Floating => None,
        }
    }
}
