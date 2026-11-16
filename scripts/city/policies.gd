extends RefCounted
## Gobierno de la ciudad: mercado de pan y carne, pósito de granos, sequías, opinión de los
## tres sectores y las ordenanzas del cabildo. Ver docs/sistemas/politicas-y-recursos.md.

signal event(text: String)

const SECTORS := ["vecinos", "comerciantes", "hacendados"]
const SECTOR_NAMES := {"vecinos": "Vecinos", "comerciantes": "Comerciantes", "hacendados": "Hacendados"}
const TOGGLE_COST := 100
const POSITO_CAP := 3000

## Ordenanzas de la época colonial.
const POLICIES := [
	{"id": "pan_fijo", "name": "Precio fijo del pan", "desc": "El pan no pasa de 1,3 aunque falte; la ciudad paga la diferencia.",
		"op": {"vecinos": 15, "comerciantes": -10, "hacendados": 0}, "monthly": 0},
	{"id": "posito", "name": "Pósito de granos", "desc": "La ciudad compra el trigo que sobra y lo reparte en la escasez.",
		"op": {"vecinos": 5, "comerciantes": -5, "hacendados": 5}, "monthly": 40},
	{"id": "no_exportar", "name": "Prohibir exportar en la escasez", "desc": "Si falta comida, la carne no se embarca: más comida, menos cueros y aduana.",
		"op": {"vecinos": 10, "comerciantes": -10, "hacendados": -15}, "monthly": 0},
	{"id": "libre_comercio", "name": "Libre comercio con ultramar", "desc": "+30 % de barcos y menos contrabando, pero la aduana cobra 30 % menos por barco.",
		"op": {"vecinos": -5, "comerciantes": 15, "hacendados": 10}, "monthly": 0},
	{"id": "turnos_agua", "name": "Turnos de agua en la sequía", "desc": "Durante la sequía las fuentes alcanzan para un 30 % más de gente.",
		"op": {"vecinos": -5, "comerciantes": 0, "hacendados": 0}, "monthly": 0},
	{"id": "aljibe_obligatorio", "name": "Aljibe obligatorio en casas de altos", "desc": "Las casas de altos tienen agua propia, pero cada una pide 10 ladrillos más.",
		"op": {"vecinos": 0, "comerciantes": -5, "hacendados": -5}, "monthly": 0},
	{"id": "sin_paja", "name": "Prohibido techar con paja cerca de la plaza", "desc": "Ranchos a menos de 400 m de la plaza: 70 % menos incendios.",
		"op": {"vecinos": -10, "comerciantes": 5, "hacendados": 0}, "monthly": 0},
	{"id": "limpieza", "name": "Limpieza de calles y basurales", "desc": "Las epidemias se contagian un 40 % menos. 20 pesos por mes por cada 1.000 habitantes.",
		"op": {"vecinos": 5, "comerciantes": 0, "hacendados": 0}, "monthly": 0},
	{"id": "faroles_vecinos", "name": "Faroles a cargo de los vecinos", "desc": "Los puestos de serenos no le cuestan a la ciudad, pero los vecinos pagan.",
		"op": {"vecinos": -8, "comerciantes": 0, "hacendados": 0}, "monthly": 0},
]

var active := {}                 # id -> true
var opinion := {"vecinos": 55.0, "comerciantes": 55.0, "hacendados": 55.0}
var bread_price := 1.0
var meat_price := 1.0
var posito := 0                  # raciones guardadas
var drought_months := 0
var last_subsidy := 0
var _protest_month := -99
var _rng := RandomNumberGenerator.new()


func _init() -> void:
	_rng.seed = 1780


func is_on(id: String) -> bool:
	return active.has(id)


func policy(id: String) -> Dictionary:
	for p in POLICIES:
		if p["id"] == id:
			return p
	return {}


func toggle(id: String) -> void:
	if active.has(id):
		active.erase(id)
		event.emit("El cabildo deroga la ordenanza: %s" % policy(id)["name"].to_lower())
	else:
		active[id] = true
		event.emit("El cabildo aprueba la ordenanza: %s" % policy(id)["name"].to_lower())


func in_drought() -> bool:
	return drought_months > 0


## Factor de capacidad de las fuentes de agua (sequía y turnos).
func water_factor() -> float:
	if not in_drought():
		return 1.0
	return 0.7 * (1.3 if is_on("turnos_agua") else 1.0)


## Mes a mes: sequía, mercado, pósito y opinión. Devuelve el gasto del mes.
func month_tick(pop: int, bread_capacity: int, estancias: int, epidemic: bool, water_ratio: float, ships_year: int, price_event: float) -> int:
	var cost := 0
	# Sequía.
	if drought_months > 0:
		drought_months -= 1
		if drought_months == 0:
			event.emit("Llovió: terminó la sequía")
	elif _rng.randf() < 0.03:
		drought_months = 5 + _rng.randi() % 4
		event.emit("Sequía: las chacras rinden menos y las fuentes dan menos agua")
	# Mercado: oferta y demanda de pan y carne.
	var harvest := 0.6 if in_drought() else 1.0
	var bread_supply := float(bread_capacity) * harvest
	var meat_supply := 500.0 + estancias * 900.0
	var demand := float(maxi(pop, 1))
	if is_on("no_exportar") and bread_supply < demand:
		meat_supply *= 1.25
	# Pósito: guarda lo que sobra y lo reparte cuando falta.
	if is_on("posito"):
		cost += 40
		if bread_supply > demand and posito < POSITO_CAP:
			var buy := mini(int(bread_supply - demand), POSITO_CAP - posito)
			posito += buy
			cost += buy / 50
		elif bread_supply < demand and posito > 0:
			var give := mini(int(demand - bread_supply), posito)
			posito -= give
			bread_supply += give
	bread_price = clampf(pow(demand / maxf(bread_supply, 1.0), 1.5), 0.6, 3.0)
	meat_price = clampf(pow(demand / maxf(meat_supply, 1.0), 1.2), 0.6, 3.0)
	last_subsidy = 0
	if is_on("pan_fijo") and bread_price > 1.3:
		last_subsidy = int((bread_price - 1.3) * pop * 0.08)
		cost += last_subsidy
		bread_price = 1.3
	if is_on("limpieza"):
		cost += int(20.0 * pop / 1000.0)
	# Opinión de los sectores: se mueve de a poco hacia su objetivo.
	var target := {"vecinos": 55.0, "comerciantes": 55.0, "hacendados": 55.0}
	for id in active:
		var op: Dictionary = policy(id)["op"]
		for s in SECTORS:
			target[s] += float(op[s])
	target["vecinos"] -= (bread_price - 1.0) * 25.0 + (meat_price - 1.0) * 10.0
	target["vecinos"] += (water_ratio - 0.5) * 20.0
	if epidemic:
		target["vecinos"] -= 12.0
	target["comerciantes"] += clampf((ships_year - 2) * 3.0, -10.0, 15.0)
	target["hacendados"] += (price_event - 1.0) * 20.0
	if in_drought():
		target["hacendados"] -= 10.0
	for s in SECTORS:
		opinion[s] = lerpf(float(opinion[s]), clampf(float(target[s]), 0.0, 100.0), 0.2)
	if float(opinion["vecinos"]) < 30.0 and _rng.randf() < 0.3:
		event.emit("Protesta en la plaza: los vecinos reclaman pan barato y agua")
	return cost


## Efectos de la opinión en el resto de la ciudad.
func arrivals_factor() -> float:
	var f := 1.0
	if float(opinion["vecinos"]) < 30.0:
		f *= 0.7
	elif float(opinion["vecinos"]) > 70.0:
		f *= 1.15
	if bread_price > 1.6:
		f *= 0.5
	return f


func ship_factor() -> float:
	var f := 1.3 if is_on("libre_comercio") else 1.0
	if float(opinion["comerciantes"]) > 70.0:
		f *= 1.1
	return f


func duty_factor() -> float:
	var f := 0.7 if is_on("libre_comercio") else 1.0
	if float(opinion["comerciantes"]) < 30.0:
		f *= 0.8   # contrabando
	return f


func export_blocked() -> bool:
	return is_on("no_exportar") and bread_price > 1.2


func countryside_bonus() -> int:
	if float(opinion["hacendados"]) < 30.0:
		return -1
	if float(opinion["hacendados"]) > 70.0:
		return 1
	return 0


func describe(money: int) -> String:
	var txt := "Ordenanzas del cabildo\n\n"
	txt += "Mercado: pan %.1f · carne %.1f (1,0 es normal)%s\n" % [bread_price, meat_price, " · subsidio %d pesos" % last_subsidy if last_subsidy > 0 else ""]
	txt += "Pósito: %d raciones guardadas%s\n" % [posito, "" if is_on("posito") else " (sin ordenanza)"]
	if in_drought():
		txt += "SEQUÍA: %d meses más (trigo −40 %%, agua −30 %%)\n" % drought_months
	txt += "\nOpinión:"
	for s in SECTORS:
		var v := int(opinion[s])
		txt += "  %s %d%s" % [SECTOR_NAMES[s], v, " (!)" if v < 30 else ""]
	txt += "\n\nCambiar una ordenanza cuesta %d pesos." % TOGGLE_COST
	return txt


## Botones: [texto, se puede, acción, id].
func actions(money: int) -> Array:
	var out := []
	for p in POLICIES:
		var on := is_on(p["id"])
		var op: Dictionary = p["op"]
		var ops := []
		for s in SECTORS:
			if int(op[s]) != 0:
				ops.append("%s %+d" % [SECTOR_NAMES[s].to_lower(), int(op[s])])
		out.append(["%s %s · %s%s" % ["[X]" if on else "[ ]", p["name"], p["desc"], (" (" + ", ".join(ops) + ")") if not ops.is_empty() else ""], money >= TOGGLE_COST, "policy", p["id"]])
	return out


func save_data() -> Dictionary:
	return {"active": active.keys(), "opinion": opinion, "posito": posito, "drought": drought_months}


func load_data(d: Dictionary) -> void:
	active.clear()
	for id in d.get("active", []):
		active[id] = true
	opinion = d.get("opinion", opinion)
	posito = d.get("posito", 0)
	drought_months = d.get("drought", 0)
