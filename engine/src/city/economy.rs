//! Gobierno de la ciudad en el motor: recursos con existencias y precio, comercio por el
//! puerto, impuestos ajustables, presupuesto mensual, opinión de los tres sectores y las
//! ordenanzas del cabildo. Porte ampliado de `scripts/city/policies.gd`; el diseño está en
//! `docs/sistemas/politicas-y-recursos.md`.
//!
//! Es lógica pura (sin vóxeles ni interfaz): `City` le pasa cada mes un [`MonthInput`] con lo
//! que produce y necesita la ciudad, y recibe un [`MonthReport`] con el presupuesto y las
//! noticias. Los factores (`arrivals_factor`, `fire_factor`…) los consultan los demás sistemas.

use crate::buildings::Rng;

// --- Recursos -----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Res {
    /// Trigo y harina (en raciones mensuales: 1 por habitante).
    Pan,
    /// Carne fresca y tasajo (raciones).
    Carne,
    /// Ladrillos (unidades de obra).
    Ladrillos,
    /// Piedra de cantera (para empedrar y fortificar).
    Piedra,
    /// Cueros secos y salados (para exportar).
    Cueros,
    /// Sebo (velas y faroles).
    Sebo,
}

impl Res {
    pub const ALL: [Res; 6] = [Res::Pan, Res::Carne, Res::Ladrillos, Res::Piedra, Res::Cueros, Res::Sebo];

    pub fn name(self) -> &'static str {
        match self {
            Res::Pan => "Pan",
            Res::Carne => "Carne",
            Res::Ladrillos => "Ladrillos",
            Res::Piedra => "Piedra",
            Res::Cueros => "Cueros",
            Res::Sebo => "Sebo",
        }
    }

    fn idx(self) -> usize {
        self as usize
    }

    /// ¿Se come? (la escasez sube el precio y enoja a los vecinos).
    pub fn is_food(self) -> bool {
        matches!(self, Res::Pan | Res::Carne)
    }

    /// Precio de referencia en pesos por unidad (en el puerto, con el mundo en calma).
    pub fn base_price(self) -> f32 {
        match self {
            Res::Pan => 0.25,
            Res::Carne => 0.15,
            Res::Ladrillos => 1.0,
            Res::Piedra => 0.6,
            Res::Cueros => 2.0,
            Res::Sebo => 0.8,
        }
    }

    /// Cuánto se puede guardar sin depósitos (meses de consumo para la comida).
    fn base_capacity(self) -> f32 {
        match self {
            Res::Pan | Res::Carne => 2000.0,
            Res::Ladrillos => 3000.0,
            Res::Piedra => 4000.0,
            Res::Cueros => 1500.0,
            Res::Sebo => 600.0,
        }
    }
}

/// Existencias, producción y precio de un recurso.
#[derive(Clone, Debug, Default)]
pub struct Stock {
    pub amount: f32,
    pub capacity: f32,
    /// Lo producido y lo consumido el último mes.
    pub produced: f32,
    pub consumed: f32,
    pub imported: f32,
    pub exported: f32,
    /// Precio relativo (1,0 = normal; 0,6 a 3,0).
    pub price: f32,
}

// --- Impuestos ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tax {
    /// Contribución de los vecinos (por casa).
    Contribucion,
    /// Alcabala: sobre las ventas de comercios y pulperías.
    Alcabala,
    /// Derechos de aduana sobre lo que entra y sale por el puerto.
    Aduana,
}

impl Tax {
    pub const ALL: [Tax; 3] = [Tax::Contribucion, Tax::Alcabala, Tax::Aduana];

    pub fn name(self) -> &'static str {
        match self {
            Tax::Contribucion => "Contribución de los vecinos",
            Tax::Alcabala => "Alcabala (ventas)",
            Tax::Aduana => "Derechos de aduana",
        }
    }

    /// Tasa habitual (%): por encima molesta, por debajo conforma.
    pub fn normal(self) -> i32 {
        match self {
            Tax::Contribucion => 6,
            Tax::Alcabala => 4,
            Tax::Aduana => 10,
        }
    }

    /// Sector al que más le pesa.
    pub fn sector(self) -> Sector {
        match self {
            Tax::Contribucion => Sector::Vecinos,
            Tax::Alcabala => Sector::Comerciantes,
            Tax::Aduana => Sector::Hacendados,
        }
    }
}

pub const TAX_MAX: i32 = 20;

// --- Sectores y ordenanzas ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sector {
    Vecinos,
    Comerciantes,
    Hacendados,
}

impl Sector {
    pub const ALL: [Sector; 3] = [Sector::Vecinos, Sector::Comerciantes, Sector::Hacendados];

    pub fn name(self) -> &'static str {
        match self {
            Sector::Vecinos => "Vecinos",
            Sector::Comerciantes => "Comerciantes",
            Sector::Hacendados => "Hacendados",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ordinance {
    PanFijo,
    Posito,
    NoExportar,
    LibreComercio,
    TurnosAgua,
    AljibeObligatorio,
    SinPaja,
    Limpieza,
    FarolesVecinos,
}

pub struct OrdInfo {
    pub id: Ordinance,
    pub name: &'static str,
    pub desc: &'static str,
    /// Efecto en la opinión de vecinos, comerciantes y hacendados.
    pub opinion: [i32; 3],
    /// Costo fijo mensual (pesos).
    pub monthly: i64,
}

pub const ORDINANCES: [OrdInfo; 9] = [
    OrdInfo { id: Ordinance::PanFijo, name: "Precio fijo del pan", desc: "El pan no pasa de 1,3 aunque falte; la ciudad paga la diferencia.", opinion: [15, -10, 0], monthly: 0 },
    OrdInfo { id: Ordinance::Posito, name: "Pósito de granos", desc: "La ciudad compra el trigo que sobra y lo reparte en la escasez.", opinion: [5, -5, 5], monthly: 40 },
    OrdInfo { id: Ordinance::NoExportar, name: "Prohibir exportar en la escasez", desc: "Si falta comida, la carne y el trigo no se embarcan.", opinion: [10, -10, -15], monthly: 0 },
    OrdInfo { id: Ordinance::LibreComercio, name: "Libre comercio con ultramar", desc: "+30 % de barcos y menos contrabando, pero la aduana cobra 30 % menos.", opinion: [-5, 15, 10], monthly: 0 },
    OrdInfo { id: Ordinance::TurnosAgua, name: "Turnos de agua en la sequía", desc: "Durante la sequía las fuentes alcanzan para un 30 % más de gente.", opinion: [-5, 0, 0], monthly: 0 },
    OrdInfo { id: Ordinance::AljibeObligatorio, name: "Aljibe obligatorio en casas de altos", desc: "Las casas de altos tienen agua propia, pero cada una pide 10 ladrillos más.", opinion: [0, -5, -5], monthly: 0 },
    OrdInfo { id: Ordinance::SinPaja, name: "Prohibido techar con paja cerca de la plaza", desc: "Ranchos a menos de 400 m de la plaza: 70 % menos incendios.", opinion: [-10, 5, 0], monthly: 0 },
    OrdInfo { id: Ordinance::Limpieza, name: "Limpieza de calles y basurales", desc: "Las epidemias se contagian un 40 % menos. 20 pesos por mes cada 1.000 habitantes.", opinion: [5, 0, 0], monthly: 0 },
    OrdInfo { id: Ordinance::FarolesVecinos, name: "Faroles a cargo de los vecinos", desc: "El sebo de los faroles lo pagan los vecinos, no la ciudad.", opinion: [-8, 0, 0], monthly: 0 },
];

pub fn ord_info(o: Ordinance) -> &'static OrdInfo {
    ORDINANCES.iter().find(|i| i.id == o).unwrap()
}

/// Cambiar una ordenanza tiene un costo único (papeleo del cabildo).
pub const TOGGLE_COST: i64 = 100;

// --- Entrada y salida del mes -------------------------------------------------------------

/// Lo que la ciudad le cuenta a la economía cada mes.
#[derive(Clone, Debug, Default)]
pub struct MonthInput {
    pub pop: i32,
    /// Casas por nivel (rancho, azotea, altos).
    pub houses: [i32; 3],
    /// Comercios y talleres por nivel (de las zonas comerciales e industriales).
    pub shops: [i32; 3],
    pub workshops: [i32; 3],
    /// Empleos de comercio y de industria.
    pub jobs_c: i32,
    pub jobs_i: i32,
    /// Producción mensual por recurso (de chacras, estancias, hornos, canteras…).
    pub production: [f32; 6],
    /// Ladrillos y piedra que pidieron las obras este mes (se descuentan si hay).
    pub construction: [f32; 6],
    /// Depósitos (barracas, pósito): capacidad extra por recurso.
    pub storage: [f32; 6],
    /// ¿Hay muelle? Sin él no se importa ni exporta.
    pub port: bool,
    /// Barcos del mes (los trae el puerto, o el valor por defecto de la economía).
    pub ships: i32,
    /// Fracción de casas con agua (0 a 1).
    pub water_ratio: f32,
    pub epidemic: bool,
    /// Mantenimiento de calles, servicios y faroles (pesos).
    pub upkeep_roads: i64,
    pub upkeep_services: i64,
    pub lamps: i32,
    /// Precio de los cueros en el mundo (los acontecimientos lejanos lo mueven; 1,0 = normal).
    pub world_hides: f32,
    /// Flujos cortados por un conflicto lejano (no llegan barcos de ultramar).
    pub blockade: bool,
}

/// Una línea del presupuesto (positiva = ingreso).
#[derive(Clone, Debug)]
pub struct Line {
    pub concept: String,
    pub amount: i64,
}

#[derive(Clone, Debug, Default)]
pub struct MonthReport {
    pub lines: Vec<Line>,
    pub news: Vec<String>,
    /// Ladrillos y piedra entregados a las obras (lo que no alcanzó queda esperando).
    pub delivered: [f32; 6],
}

impl MonthReport {
    pub fn balance(&self) -> i64 {
        self.lines.iter().map(|l| l.amount).sum()
    }
    pub fn income(&self) -> i64 {
        self.lines.iter().filter(|l| l.amount > 0).map(|l| l.amount).sum()
    }
    pub fn expenses(&self) -> i64 {
        -self.lines.iter().filter(|l| l.amount < 0).map(|l| l.amount).sum::<i64>()
    }
    fn add(&mut self, concept: impl Into<String>, amount: i64) {
        if amount != 0 {
            self.lines.push(Line { concept: concept.into(), amount });
        }
    }
}

// --- Economía -----------------------------------------------------------------------------

pub const POSITO_CAP: f32 = 3000.0;

#[derive(Clone, Debug)]
pub struct Economy {
    pub stocks: [Stock; 6],
    pub taxes: [i32; 3],
    pub active: Vec<Ordinance>,
    /// Opinión de vecinos, comerciantes y hacendados (0 a 100).
    pub opinion: [f32; 3],
    pub posito: f32,
    pub drought_months: u32,
    /// Último presupuesto cerrado (para el panel).
    pub last: MonthReport,
    pub last_subsidy: i64,
    rng: Rng,
}

impl Default for Economy {
    fn default() -> Self {
        let mut stocks: [Stock; 6] = Default::default();
        for r in Res::ALL {
            let s = &mut stocks[r.idx()];
            s.price = 1.0;
            s.capacity = r.base_capacity();
        }
        // La fundación llega con algo de provisiones y materiales.
        stocks[Res::Pan.idx()].amount = 400.0;
        stocks[Res::Carne.idx()].amount = 400.0;
        stocks[Res::Ladrillos.idx()].amount = 200.0;
        stocks[Res::Piedra.idx()].amount = 150.0;
        Economy {
            stocks,
            taxes: Tax::ALL.map(|t| t.normal()),
            active: Vec::new(),
            opinion: [55.0; 3],
            posito: 0.0,
            drought_months: 0,
            last: MonthReport::default(),
            last_subsidy: 0,
            rng: Rng::new(1780),
        }
    }
}

impl Economy {
    pub fn stock(&self, r: Res) -> &Stock {
        &self.stocks[r.idx()]
    }

    pub fn price(&self, r: Res) -> f32 {
        self.stocks[r.idx()].price
    }

    pub fn is_on(&self, o: Ordinance) -> bool {
        self.active.contains(&o)
    }

    pub fn in_drought(&self) -> bool {
        self.drought_months > 0
    }

    pub fn tax(&self, t: Tax) -> i32 {
        self.taxes[t as usize]
    }

    /// Cambia una tasa (0 a 20 %). Devuelve la tasa resultante.
    pub fn set_tax(&mut self, t: Tax, rate: i32) -> i32 {
        self.taxes[t as usize] = rate.clamp(0, TAX_MAX);
        self.taxes[t as usize]
    }

    /// Activa o deroga una ordenanza; cobra el papeleo. `Err` si no alcanza el dinero.
    pub fn toggle(&mut self, o: Ordinance, money: &mut i64) -> Result<String, String> {
        if *money < TOGGLE_COST {
            return Err(format!("Cambiar una ordenanza cuesta {TOGGLE_COST} pesos"));
        }
        *money -= TOGGLE_COST;
        let name = ord_info(o).name.to_lowercase();
        Ok(if let Some(i) = self.active.iter().position(|&x| x == o) {
            self.active.remove(i);
            format!("El cabildo deroga la ordenanza: {name}")
        } else {
            self.active.push(o);
            format!("El cabildo aprueba la ordenanza: {name}")
        })
    }

    /// Toma ladrillos o piedra para una obra inmediata (empedrar, iglesia…). `false` si no hay.
    pub fn take(&mut self, r: Res, n: f32) -> bool {
        let s = &mut self.stocks[r.idx()];
        if s.amount + 1e-3 < n {
            return false;
        }
        s.amount -= n;
        true
    }

    // --- Factores para los demás sistemas ---------------------------------------------

    /// Fuentes de agua: la sequía las achica; los turnos reparten mejor.
    pub fn water_factor(&self) -> f32 {
        if !self.in_drought() {
            return 1.0;
        }
        0.7 * if self.is_on(Ordinance::TurnosAgua) { 1.3 } else { 1.0 }
    }

    /// Llegada de pobladores: el pan caro, los impuestos altos y el enojo los espantan.
    pub fn arrivals_factor(&self) -> f32 {
        let mut f = 1.0;
        let v = self.opinion[0];
        if v < 30.0 {
            f *= 0.7;
        } else if v > 70.0 {
            f *= 1.15;
        }
        if self.price(Res::Pan) > 1.6 {
            f *= 0.5;
        }
        f * (1.0 - (self.tax(Tax::Contribucion) - Tax::Contribucion.normal()) as f32 * 0.03).clamp(0.6, 1.2)
    }

    pub fn ship_factor(&self) -> f32 {
        let mut f = if self.is_on(Ordinance::LibreComercio) { 1.3 } else { 1.0 };
        if self.opinion[1] > 70.0 {
            f *= 1.1;
        }
        f
    }

    /// Parte de la aduana que efectivamente se cobra (el contrabando crece con tasas altas).
    pub fn duty_factor(&self) -> f32 {
        let mut f = if self.is_on(Ordinance::LibreComercio) { 0.7 } else { 1.0 };
        if self.opinion[1] < 30.0 {
            f *= 0.8;
        }
        let over = (self.tax(Tax::Aduana) - Tax::Aduana.normal()).max(0) as f32;
        f * (1.0 - over * 0.025)
    }

    pub fn export_blocked(&self) -> bool {
        self.is_on(Ordinance::NoExportar) && (self.price(Res::Pan) > 1.2 || self.price(Res::Carne) > 1.2)
    }

    /// Riesgo de incendio en ranchos cerca de la plaza.
    pub fn thatch_fire_factor(&self) -> f32 {
        if self.is_on(Ordinance::SinPaja) { 0.3 } else { 1.0 }
    }

    pub fn epidemic_factor(&self) -> f32 {
        if self.is_on(Ordinance::Limpieza) { 0.6 } else { 1.0 }
    }

    /// Ladrillos extra que pide cada casa de altos.
    pub fn extra_l3_bricks(&self) -> f32 {
        if self.is_on(Ordinance::AljibeObligatorio) { 10.0 } else { 0.0 }
    }

    /// Producción del campo: los hacendados descontentos mandan menos.
    pub fn countryside_factor(&self) -> f32 {
        let h = self.opinion[2];
        let mut f = if h < 30.0 { 0.8 } else if h > 70.0 { 1.15 } else { 1.0 };
        let over = (self.tax(Tax::Aduana) - Tax::Aduana.normal()).max(0) as f32;
        f *= 1.0 - over * 0.015;
        f
    }

    // --- Mes ------------------------------------------------------------------------------

    /// Cierra el mes: produce, consume, comercia, cobra y paga. Devuelve el presupuesto.
    pub fn month(&mut self, inp: &MonthInput) -> MonthReport {
        let mut rep = MonthReport::default();
        let pop = inp.pop.max(0) as f32;

        // Sequía.
        if self.drought_months > 0 {
            self.drought_months -= 1;
            if self.drought_months == 0 {
                rep.news.push("Llovió: terminó la sequía".into());
            }
        } else if self.rng.f() < 0.03 {
            self.drought_months = 5 + self.rng.range(0, 4) as u32;
            rep.news.push("Sequía: las chacras rinden menos y las fuentes dan menos agua".into());
        }

        // Producción y consumo.
        let country = self.countryside_factor();
        for r in Res::ALL {
            let s = &mut self.stocks[r.idx()];
            s.capacity = r.base_capacity() + inp.storage[r.idx()];
            let mut p = inp.production[r.idx()];
            if r == Res::Pan && self.drought_months > 0 {
                p *= 0.6;
            }
            if matches!(r, Res::Carne | Res::Cueros | Res::Sebo) {
                p *= country;
            }
            s.produced = p;
            s.amount += p;
            s.imported = 0.0;
            s.exported = 0.0;
        }
        let demand = |r: Res| -> f32 {
            match r {
                Res::Pan => pop,
                Res::Carne => pop * 0.8,
                Res::Sebo => inp.lamps as f32 * 2.0 + pop * 0.05,
                _ => inp.construction[r.idx()],
            }
        };

        // Pósito: guarda trigo cuando sobra, lo reparte cuando falta.
        if self.is_on(Ordinance::Posito) {
            rep.add("Pósito de granos", -ord_info(Ordinance::Posito).monthly);
            let pan = &mut self.stocks[Res::Pan.idx()];
            let need = demand(Res::Pan);
            if pan.amount > need * 2.0 && self.posito < POSITO_CAP {
                let buy = (pan.amount - need * 2.0).min(POSITO_CAP - self.posito);
                pan.amount -= buy;
                self.posito += buy;
                rep.add("Compras del pósito", -(buy * Res::Pan.base_price()) as i64);
            } else if pan.amount < need && self.posito > 0.0 {
                let give = (need - pan.amount).min(self.posito);
                self.posito -= give;
                pan.amount += give;
                rep.news.push(format!("El pósito reparte {} raciones de trigo", give as i32));
            }
        }

        // Comercio por el puerto: se importa lo que escasea y se exporta lo que sobra.
        let ships = if inp.blockade { 0 } else { inp.ships };
        let can_trade = inp.port && ships > 0;
        let duty_rate = self.tax(Tax::Aduana) as f32 / 100.0 * self.duty_factor();
        let mut duty = 0.0;
        let mut traders_balance = 0.0;
        let export_blocked = self.export_blocked();
        if can_trade {
            let cap_per_ship = 300.0;
            let hold = ships as f32 * cap_per_ship;
            for r in Res::ALL {
                let need = demand(r);
                let s = &mut self.stocks[r.idx()];
                let world = r.base_price() * if r == Res::Cueros { inp.world_hides } else { 1.0 };
                if r.is_food() || matches!(r, Res::Ladrillos | Res::Piedra) {
                    // Importa hasta cubrir un mes y medio de necesidad.
                    let want = (need * 1.5 - s.amount).max(0.0).min(hold * 0.25);
                    if want > 0.0 {
                        s.amount += want;
                        s.imported = want;
                        duty += want * world * duty_rate;
                        traders_balance += want * world * 0.2;
                    }
                }
                // Exporta lo que pasa de tres meses de reserva (los cueros, todo lo juntado).
                let keep = if r == Res::Cueros { 0.0 } else { need * 3.0 };
                let blocked = r.is_food() && export_blocked;
                if !blocked && r != Res::Ladrillos && r != Res::Piedra {
                    let out = (s.amount - keep).max(0.0).min(hold * 0.5);
                    if out > 1.0 {
                        s.amount -= out;
                        s.exported = out;
                        duty += out * world * duty_rate;
                        traders_balance += out * world * 0.3;
                    }
                }
            }
            if export_blocked {
                rep.news.push("Escasez: el cabildo prohíbe embarcar comida".into());
            }
        }

        // Consumo, entrega a las obras y precios.
        for r in Res::ALL {
            let need = demand(r);
            let s = &mut self.stocks[r.idx()];
            let used = need.min(s.amount);
            s.amount -= used;
            s.consumed = used;
            if matches!(r, Res::Ladrillos | Res::Piedra) {
                rep.delivered[r.idx()] = used;
            }
            // Precio según cuántos meses de reserva quedan después de comer.
            let cover = if need > 0.5 { (s.amount + used) / need } else { 3.0 };
            let target = (1.5 / cover.max(0.2)).powf(0.8).clamp(0.6, 3.0);
            s.price = s.price + (target - s.price) * 0.6;
            if s.amount > s.capacity {
                s.amount = s.capacity; // lo que no entra se pierde (se pudre o se malvende)
            }
        }
        self.last_subsidy = 0;
        if self.is_on(Ordinance::PanFijo) {
            let s = &mut self.stocks[Res::Pan.idx()];
            if s.price > 1.3 {
                self.last_subsidy = ((s.price - 1.3) * pop * 0.08) as i64;
                s.price = 1.3;
                rep.add("Subsidio del pan", -self.last_subsidy);
            }
        }

        // Ingresos.
        let houses = inp.houses;
        let weight = |b: [i32; 3], w: [f32; 3]| b.iter().zip(w).map(|(n, w)| *n as f32 * w).sum::<f32>();
        let contrib_base = weight(houses, [1.0, 3.0, 7.0]) + weight(inp.shops, [2.0, 4.0, 9.0]) + weight(inp.workshops, [2.0, 5.0, 10.0]);
        rep.add("Contribución de los vecinos", (contrib_base * self.tax(Tax::Contribucion) as f32 / 6.0 * 2.0) as i64);
        // Las ventas crecen con los comercios: sin tiendas, los vecinos se arreglan con los
        // pulperos ambulantes y la alcabala casi no cobra.
        let coverage = (inp.jobs_c as f32 / (pop * 0.08).max(1.0)).min(1.2);
        let sales = pop * 0.9 * (self.price(Res::Pan) * 0.5 + 0.5) * (0.4 + 0.6 * coverage) + inp.jobs_i as f32 * 4.0;
        rep.add("Alcabala", (sales * self.tax(Tax::Alcabala) as f32 / 100.0 * 3.0) as i64);
        rep.add("Aduana", duty as i64);

        // Gastos.
        rep.add("Mantenimiento de calles", -inp.upkeep_roads);
        rep.add("Servicios (pozos, aljibes, puestos)", -inp.upkeep_services);
        if !self.is_on(Ordinance::FarolesVecinos) {
            let sebo = inp.lamps as i64 * (2.0 * self.price(Res::Sebo)) as i64;
            rep.add("Faroles (sebo)", -sebo);
        }
        if self.is_on(Ordinance::Limpieza) {
            rep.add("Limpieza de calles", -(20.0 * pop / 1000.0) as i64);
        }

        // Opinión: se mueve de a poco hacia su objetivo.
        let mut target = [55.0f32; 3];
        for &o in &self.active {
            for (k, v) in ord_info(o).opinion.iter().enumerate() {
                target[k] += *v as f32;
            }
        }
        for t in Tax::ALL {
            let over = (self.tax(t) - t.normal()) as f32;
            target[t.sector() as usize] -= over * 2.5;
            // Todos sienten un poco los impuestos de los demás.
            for k in 0..3 {
                target[k] -= over * 0.5;
            }
        }
        target[0] -= (self.price(Res::Pan) - 1.0) * 25.0 + (self.price(Res::Carne) - 1.0) * 10.0;
        target[0] += (inp.water_ratio - 0.5) * 20.0;
        if inp.epidemic {
            target[0] -= 12.0;
        }
        target[1] += (ships as f32 - 1.0).clamp(-3.0, 5.0) * 3.0 + (traders_balance / 50.0).min(10.0);
        if inp.blockade {
            target[1] -= 10.0;
        }
        target[2] += (inp.world_hides - 1.0) * 20.0;
        if self.in_drought() {
            target[2] -= 10.0;
        }
        for k in 0..3 {
            self.opinion[k] += (target[k].clamp(0.0, 100.0) - self.opinion[k]) * 0.2;
        }
        if self.opinion[0] < 30.0 && self.rng.f() < 0.3 {
            rep.news.push("Protesta en la plaza: los vecinos reclaman pan barato y agua".into());
        }
        if self.opinion[1] < 30.0 && self.rng.f() < 0.2 {
            rep.news.push("Crece el contrabando: los comerciantes esquivan la aduana".into());
        }

        self.last = rep.clone();
        rep
    }

    // --- Textos para la interfaz -------------------------------------------------------

    /// Líneas del panel de recursos: nombre, existencias, variación del mes y precio.
    pub fn resource_lines(&self) -> Vec<String> {
        Res::ALL
            .iter()
            .map(|&r| {
                let s = self.stock(r);
                let delta = s.produced + s.imported - s.consumed - s.exported;
                format!("{}: {} ({:+}/mes) · precio {:.1}{}", r.name(), s.amount as i64, delta as i64, s.price, if s.price > 1.6 { " (!)" } else { "" })
            })
            .collect()
    }

    pub fn opinion_line(&self) -> String {
        Sector::ALL
            .iter()
            .enumerate()
            .map(|(k, s)| format!("{} {}{}", s.name(), self.opinion[k] as i32, if self.opinion[k] < 30.0 { " (!)" } else { "" }))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(pop: i32, pan: f32) -> MonthInput {
        let mut i = MonthInput { pop, houses: [pop / 8, 0, 0], water_ratio: 0.8, world_hides: 1.0, ships: 1, ..Default::default() };
        i.production[Res::Pan.idx()] = pan;
        i.production[Res::Carne.idx()] = pop as f32;
        i
    }

    #[test]
    fn la_escasez_sube_el_precio_y_enoja() {
        let mut e = Economy::default();
        for _ in 0..6 {
            e.month(&input(1000, 400.0));
        }
        assert!(e.price(Res::Pan) > 1.6, "pan {}", e.price(Res::Pan));
        assert!(e.opinion[0] < 45.0, "vecinos {}", e.opinion[0]);
        let mut a = Economy::default();
        for _ in 0..6 {
            a.month(&input(1000, 1600.0));
        }
        assert!(a.price(Res::Pan) < 1.0, "pan con abundancia {}", a.price(Res::Pan));
        assert!(a.arrivals_factor() > e.arrivals_factor());
    }

    #[test]
    fn precio_fijo_cuesta_dinero() {
        let mut e = Economy::default();
        let mut money = 1000;
        e.toggle(Ordinance::PanFijo, &mut money).unwrap();
        assert_eq!(money, 1000 - TOGGLE_COST);
        let mut subsidy = 0;
        for _ in 0..6 {
            let r = e.month(&input(2000, 600.0));
            subsidy += e.last_subsidy;
            assert!(r.lines.iter().all(|l| !l.concept.is_empty()));
        }
        assert!(e.price(Res::Pan) <= 1.3 + 1e-4);
        assert!(subsidy > 0);
    }

    #[test]
    fn el_posito_amortigua_un_mal_año() {
        let run = |posito: bool| {
            let mut e = Economy::default();
            let mut money = 1000;
            if posito {
                e.toggle(Ordinance::Posito, &mut money).unwrap();
            }
            for _ in 0..12 {
                e.month(&input(800, 2000.0)); // años buenos
            }
            let mut worst: f32 = 0.0;
            for _ in 0..4 {
                e.month(&input(800, 300.0)); // mal año
                worst = worst.max(e.price(Res::Pan));
            }
            worst
        };
        let (sin, con) = (run(false), run(true));
        assert!(con < sin, "con pósito {con} vs sin {sin}");
    }

    #[test]
    fn impuestos_altos_recaudan_mas_y_molestan() {
        let mut a = Economy::default();
        let mut b = Economy::default();
        b.set_tax(Tax::Contribucion, 14);
        b.set_tax(Tax::Alcabala, 10);
        let (mut ia, mut ib) = (0, 0);
        for _ in 0..8 {
            ia += a.month(&input(1500, 1500.0)).income();
            ib += b.month(&input(1500, 1500.0)).income();
        }
        assert!(ib > ia, "recaudación {ib} vs {ia}");
        assert!(b.opinion[0] < a.opinion[0] - 5.0);
        assert!(b.opinion[1] < a.opinion[1] - 5.0);
        assert!(b.arrivals_factor() < a.arrivals_factor());
        assert_eq!(b.set_tax(Tax::Aduana, 99), TAX_MAX);
    }

    #[test]
    fn el_puerto_importa_en_la_escasez_y_el_bloqueo_lo_corta() {
        let mut e = Economy::default();
        let mut inp = input(1000, 300.0);
        inp.port = true;
        inp.ships = 2;
        inp.production[Res::Cueros.idx()] = 400.0;
        let r = e.month(&inp);
        assert!(e.stock(Res::Pan).imported > 0.0);
        assert!(e.stock(Res::Cueros).exported > 0.0);
        assert!(r.lines.iter().any(|l| l.concept == "Aduana" && l.amount > 0));
        let mut b = Economy::default();
        inp.blockade = true;
        b.month(&inp);
        assert_eq!(b.stock(Res::Pan).imported, 0.0);
        assert_eq!(b.stock(Res::Cueros).exported, 0.0);
    }

    #[test]
    fn prohibir_exportar_retiene_la_comida() {
        let mut e = Economy::default();
        let mut money = 1000;
        e.toggle(Ordinance::NoExportar, &mut money).unwrap();
        let mut inp = input(1000, 200.0);
        inp.port = true;
        inp.ships = 2;
        inp.production[Res::Carne.idx()] = 6000.0;
        for _ in 0..3 {
            e.month(&inp);
        }
        assert!(e.export_blocked());
        assert_eq!(e.stock(Res::Carne).exported, 0.0);
    }

    #[test]
    fn obras_toman_ladrillos_y_derogar_vuelve_atras() {
        let mut e = Economy::default();
        assert!(e.take(Res::Ladrillos, 150.0));
        assert!(!e.take(Res::Ladrillos, 100.0));
        let mut money = 150;
        e.toggle(Ordinance::SinPaja, &mut money).unwrap();
        assert!(e.thatch_fire_factor() < 0.5);
        assert!(e.toggle(Ordinance::SinPaja, &mut money).is_err(), "sin fondos para el papeleo");
        money = 500;
        e.toggle(Ordinance::SinPaja, &mut money).unwrap();
        assert_eq!(e.thatch_fire_factor(), 1.0);
        assert_eq!(e.resource_lines().len(), 6);
    }
}
