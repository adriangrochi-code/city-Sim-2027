//! Recorrido automático para probar el motor sin intervención: mueve la cámara, construye,
//! excava, viaja lejos (streaming) y guarda capturas en puntos clave con métricas.
//! Funciona igual con ventana o sin pantalla.

use crate::menu::{Menu, MenuAction, Screen};
use crate::render::Renderer;
use crate::save;
use crate::ui::{self, paint::UiPainter, GameUi};
use crate::session::{memory_mb, Session};
use glam::Vec2;
use std::time::{Duration, Instant};

pub enum Act {
    /// Cámara: x, z, distancia, giro, inclinación.
    Cam([f32; 5]),
    Hour(f32),
    Smooth(bool),
    WaitIdle(&'static str),
    Shot(&'static str),
    /// Coloca una estructura del catálogo (índice, giro) en x, z.
    Place(usize, u32, f32, f32),
    /// Coloca usando el rayo del centro de la pantalla (como un clic).
    PlaceCenter(usize, u32),
    Dig(f32, f32, i32),
    Fill(f32, f32, i32),
    /// Viaja con la cámara hasta x, z en N cuadros (el mundo se va cargando en el camino).
    Travel(f32, f32, u32),
    /// Lleva la cámara al puente que construyó la demostración.
    Bridge,
    /// Cámara calculada a partir de la partida (un servicio, el muelle, la muralla…); si no
    /// hay nada que mirar, la deja donde está.
    CamFn(fn(&Session) -> Option<[f32; 5]>),
    /// Una acción del modo juego (fundar, trazar, zonificar, crecer); devuelve lo que pasó.
    Game(&'static str, fn(&mut Session) -> String),
    /// Captura con la interfaz del juego encima, después de prepararla (herramienta, ficha,
    /// avisos, panel) con la función dada.
    UiShot(&'static str, fn(&mut Session, &mut GameUi)),
    /// Captura con el menú (principal, pausa, cargar…) armado por la función dada.
    MenuShot(&'static str, fn(&mut Session, &mut GameUi, &mut Menu)),
    /// Guarda la partida, la vuelve a cargar (mundo regenerado desde el archivo) y compara.
    SaveLoad(&'static str),
    /// Mide el cuadro con y sin los vecinos dibujados.
    MedirVecinos(&'static str),
}

/// Interfaz para las capturas sin pantalla: contexto de egui, pintor y estado del juego.
struct TourUi {
    ctx: egui::Context,
    painter: UiPainter,
    game_ui: GameUi,
    menu: Menu,
    ms: Vec<f64>,
}

/// Paisajes del mapa real de Montevideo (`motor --mapa real --auto-test carpeta`): para revisar
/// el relieve, la costa, los médanos y los biomas con el mismo generador que usa el juego.
pub fn landscape_steps() -> Vec<Act> {
    use Act::*;
    let views: [(&'static str, [f32; 5]); 29] = [
        ("paisaje-general", [14000.0, 12500.0, 7000.0, 200.0, 50.0]),
        // Hidrografía oficial (IDE): la playa de Pocitos y los bañados del Carrasco y el Pantanoso.
        ("paisaje-playa-pocitos", [16500.0, 15000.0, 700.0, 200.0, 28.0]),
        ("paisaje-banado-carrasco", [24000.0, 5000.0, 1500.0, 230.0, 30.0]),
        ("paisaje-banado-pantanoso", [7000.0, 9000.0, 1200.0, 200.0, 32.0]),
        ("paisaje-ciudad-vieja", [11300.0, 14350.0, 900.0, 200.0, 38.0]),
        ("paisaje-medanos-carrasco", [22000.0, 13100.0, 520.0, 200.0, 30.0]),
        ("paisaje-costa-este-lejos", [21500.0, 13000.0, 2600.0, 230.0, 28.0]),
        ("paisaje-arroyo-monte", [12752.5, 5055.0, 260.0, 200.0, 38.0]),
        ("paisaje-cerro", [5924.0, 12270.0, 1600.0, 200.0, 32.0]),
        ("paisaje-miguelete-cerca", [10431.2, 9144.2, 45.0, 120.0, 14.0]),
        ("paisaje-miguelete-medio", [10431.2, 9144.2, 220.0, 120.0, 32.0]),
        ("paisaje-ladera-cerca", [9000.0, 9000.0, 120.0, 200.0, 30.0]),
        ("paisaje-isla-ratas", [8294.0, 12280.0, 420.0, 200.0, 32.0]),
        // La cámara no sale del mapa: la isla (x 36 200, z 18 400) se ve a 12 km, en el horizonte.
        ("paisaje-isla-flores", [24000.0, 18400.0, 300.0, 270.0, 3.0]),
        ("paisaje-cenital-este", [21500.0, 13000.0, 14000.0, 230.0, 85.0]),
        // Monte nativo de 1726 (bosque.bin): galería del Miguelete de cerca, a media distancia
        // y el monte del Miguelete alto (que hoy sigue en pie).
        ("vegetacion-galeria-cerca", [12140.0, 7560.0, 40.0, 200.0, 12.0]),
        ("vegetacion-galeria-media", [12120.0, 7600.0, 260.0, 210.0, 26.0]),
        ("vegetacion-monte-norte", [13000.0, 6140.0, 140.0, 160.0, 20.0]),
        // Flora baja por zonas, de cerca: bañado del Pantanoso, médanos de Carrasco y el Cerro.
        ("vegetacion-banado-cerca", [5840.0, 8300.0, 35.0, 200.0, 14.0]),
        ("vegetacion-medanos-cerca", [22600.0, 13150.0, 35.0, 180.0, 14.0]),
        ("vegetacion-cerro-cerca", [5900.0, 12350.0, 40.0, 200.0, 16.0]),
        // Afloramientos del basamento cristalino (CONEAT, rocas.bin): colina rocosa del
        // interior de cerca y de lejos, y la costa rocosa.
        ("roca-colina-cerca", [13480.0, 8480.0, 45.0, 200.0, 14.0]),
        ("roca-colina-media", [14640.0, 8200.0, 300.0, 200.0, 24.0]),
        ("roca-costa-cerca", [12580.0, 15240.0, 50.0, 200.0, 16.0]),
        // Puntas de piedra de la costa sur (costa.json): Punta Carretas y Punta Gorda de cerca.
        ("costa-punta-carretas", [15190.0, 17380.0, 150.0, 200.0, 22.0]),
        ("costa-punta-gorda", [22420.0, 13470.0, 150.0, 200.0, 22.0]),
        ("costa-isla-gaviotas", [20175.0, 13600.0, 260.0, 200.0, 18.0]),
        ("costa-punta-yeguas", [1120.0, 14000.0, 200.0, 200.0, 22.0]),
        ("costa-pocitos", [16100.0, 15400.0, 180.0, 200.0, 20.0]),
    ];
    let mut out = vec![Hour(10.5)];
    // `MOTOR_VISTAS=vegetacion` saca solo las vistas cuyo nombre contiene ese texto.
    let filtro = std::env::var("MOTOR_VISTAS").unwrap_or_default();
    for (name, cam) in views.into_iter().filter(|(n, _)| n.contains(filtro.as_str())) {
        out.push(Cam(cam));
        out.push(WaitIdle(name));
        out.push(Shot(name));
    }
    out
}

/// Recorrido del modo juego: funda la plaza, traza un damero y una calle curva, zonifica,
/// hace crecer la ciudad y saca capturas (general, de cerca sobre la curva y de noche).
pub fn city_steps() -> Vec<Act> {
    use crate::city::game_demo as d;
    use Act::*;
    let c = d::PLAZA;
    vec![
        Cam([c.x, c.y, 700.0, 200.0, 45.0]),
        Hour(10.5),
        WaitIdle("terreno virgen"),
        Game("fundar la plaza", d::found),
        Cam([c.x + 20.0, c.y + 30.0, 250.0, 200.0, 40.0]),
        WaitIdle("plaza fundada de cerca"),
        Shot("motor-plaza-fundada-cerca"),
        Cam([c.x + 20.0, c.y + 30.0, 700.0, 200.0, 40.0]),
        WaitIdle("plaza fundada de lejos"),
        Shot("motor-plaza-fundada-lejos"),
        Cam([c.x, c.y, 700.0, 200.0, 45.0]),
        Game("damero", d::grid),
        Game("calle curva", d::curve),
        Game("zonificar", d::zone),
        Game("servicios, puerto, muralla y campo", d::servicios),
        Game("escuelas, salud, seguridad y saneamiento", d::barrio),
        Game("puente", d::bridge),
        Bridge,
        WaitIdle("puente"),
        Shot("motor-puente"),
        Cam([c.x, c.y, 700.0, 200.0, 45.0]),
        WaitIdle("trazado"),
        Shot("motor-ciudad-trazado"),
        Game("crecer 60 meses", d::grow),
        WaitIdle("ciudad crecida"),
        Game("escuelas, salud y delito después de crecer", d::barrio_informe),
        Game("un mes al ritmo del juego", d::month_realtime),
        WaitIdle("re-mallado de un mes"),
        // Vecinos y vehículos: la mañana de un día hábil.
        Game("unir los pedazos del damero", d::unir_calles),
        Game("vecinos y vehículos", d::vecinos),
        Hour(7.5),
        CamFn(|s| d::calle_concurrida(s, 38.0, 30.0)),
        WaitIdle("vecinos de cerca"),
        Shot("motor-vecinos-cerca"),
        CamFn(|s| d::calle_concurrida(s, 110.0, 38.0)),
        WaitIdle("vecinos en las calles"),
        Shot("motor-vecinos-calles"),
        MedirVecinos("dibujo con y sin vecinos"),
        CamFn(|_| crate::city::game_demo::BRIDGE_AT.lock().unwrap().map(|p| [p.x, p.y, 34.0, 50.0, 24.0])),
        WaitIdle("vecinos en el puente"),
        Shot("motor-vecinos-puente"),
        Hour(10.5),
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        WaitIdle("vista general"),
        Shot("motor-ciudad-general"),
        Cam([d::CURVE_VIEW.x, d::CURVE_VIEW.y, 110.0, 160.0, 38.0]),
        WaitIdle("curva de cerca"),
        Shot("motor-ciudad-curva"),
        Cam([c.x, c.y, 150.0, 200.0, 32.0]),
        WaitIdle("plaza de cerca"),
        Shot("motor-ciudad-plaza"),
        Cam([c.x + 60.0, c.y + 40.0, 520.0, 215.0, 40.0]),
        Hour(20.5),
        WaitIdle("noche"),
        Shot("motor-ciudad-noche"),
        // Servicios (Fase 2 #5): cada edificio de cerca, el puerto, la muralla, la iglesia en
        // obra, los faroles de noche y un incendio con sus ruinas.
        CamFn(d::view_lamps),
        WaitIdle("faroles de noche"),
        Shot("motor-servicios-faroles-noche"),
        Hour(10.5),
        Game("iglesia por fases", d::iglesia),
        CamFn(d::view_church),
        Hour(11.0),
        WaitIdle("iglesia en obra"),
        Shot("motor-servicios-iglesia"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Aljibe, 40.0)),
        Hour(11.0),
        WaitIdle("aljibe"),
        Shot("motor-servicios-aljibe"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Pozo, 28.0)),
        Hour(11.0),
        WaitIdle("pozo"),
        Shot("motor-servicios-pozo"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Aguadores, 50.0)),
        Hour(11.0),
        WaitIdle("aguadores"),
        Shot("motor-servicios-aguadores"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Serenos, 35.0)),
        Hour(11.0),
        WaitIdle("serenos"),
        Shot("motor-servicios-serenos"),
        CamFn(d::view_pier),
        Hour(11.0),
        WaitIdle("muelle"),
        Shot("motor-servicios-muelle"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Aduana, 75.0)),
        Hour(11.0),
        WaitIdle("aduana"),
        Shot("motor-servicios-aduana"),
        CamFn(d::view_wall),
        Hour(11.0),
        WaitIdle("muralla"),
        Shot("motor-servicios-muralla"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Lazareto, 95.0)),
        Hour(11.0),
        WaitIdle("lazareto"),
        Shot("motor-servicios-lazareto"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Chacra, 170.0)),
        Hour(11.0),
        WaitIdle("chacra"),
        Shot("motor-servicios-chacra"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Estancia, 120.0)),
        Hour(11.0),
        WaitIdle("estancia"),
        Shot("motor-servicios-estancia"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Horno, 75.0)),
        Hour(11.0),
        WaitIdle("horno"),
        Shot("motor-servicios-horno"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Cantera, 80.0)),
        Hour(11.0),
        WaitIdle("cantera"),
        Shot("motor-servicios-cantera"),
        // Educación, salud, seguridad y saneamiento: cada edificio de cerca, un basural y las capas.
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Escuela, 34.0)),
        Hour(11.0),
        WaitIdle("escuela"),
        Shot("motor-barrio-escuela"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Convento, 85.0)),
        Hour(11.0),
        WaitIdle("convento"),
        Shot("motor-barrio-convento"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Botica, 26.0)),
        Hour(11.0),
        WaitIdle("botica"),
        Shot("motor-barrio-botica"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Medico, 32.0)),
        Hour(11.0),
        WaitIdle("médico"),
        Shot("motor-barrio-medico"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Hospital, 85.0)),
        Hour(11.0),
        WaitIdle("hospital"),
        Shot("motor-barrio-hospital"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Carcel, 60.0)),
        Hour(11.0),
        WaitIdle("cárcel"),
        Shot("motor-barrio-carcel"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Alcalde, 18.0)),
        Hour(11.0),
        WaitIdle("alcalde"),
        Shot("motor-barrio-alcalde"),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Basurero, 70.0)),
        Hour(11.0),
        WaitIdle("basurero"),
        Shot("motor-barrio-basurero"),
        CamFn(d::view_basural),
        Hour(11.0),
        WaitIdle("basural"),
        Shot("motor-barrio-basural"),
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        Hour(11.0),
        WaitIdle("capas del barrio"),
        UiShot("motor-ui-capa-educacion", |s, u| ui_layer(s, u, crate::city::services::Layer::Educacion)),
        UiShot("motor-ui-capa-salud", |s, u| ui_layer(s, u, crate::city::services::Layer::Salud)),
        UiShot("motor-ui-capa-saneamiento", |s, u| ui_layer(s, u, crate::city::services::Layer::Saneamiento)),
        UiShot("motor-ui-capa-delito", |s, u| ui_layer(s, u, crate::city::services::Layer::Delito)),
        // Recursos del suelo, molestias y valor del suelo (ver `city::recursos`).
        UiShot("motor-ui-capa-valor", |s, u| ui_layer(s, u, crate::city::services::Layer::Valor)),
        Cam([c.x + 300.0, c.y - 20.0, 950.0, 215.0, 50.0]),
        WaitIdle("molestias"),
        UiShot("motor-ui-capa-molestias", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Molestias, Vec2::new(300.0, 0.0))),
        UiShot("motor-ui-capa-valor-industria", |s, u| ui_layer(s, u, crate::city::services::Layer::Valor)),
        Cam([c.x + 1500.0, c.y - 1700.0, 5200.0, 200.0, 58.0]),
        WaitIdle("el campo de lejos"),
        UiShot("motor-ui-capa-cultivo", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Cultivo, Vec2::new(1500.0, -1700.0))),
        UiShot("motor-ui-capa-pasturas", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Pasturas, Vec2::new(1500.0, -1700.0))),
        UiShot("motor-ui-capa-piedra", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Piedra, Vec2::new(1500.0, -1700.0))),
        UiShot("motor-ui-capa-arcilla", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Arcilla, Vec2::new(1500.0, -1700.0))),
        UiShot("motor-ui-capa-arena", |s, u| ui_capa_terreno(s, u, crate::city::services::Layer::Arena, Vec2::new(1500.0, -1700.0))),
        UiShot("motor-ui-ficha-terreno", ui_ficha_terreno),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Cantera, 90.0)),
        WaitIdle("ficha de la cantera"),
        UiShot("motor-ui-ficha-cantera", |s, u| ui_ficha_servicio(s, u, crate::city::services::Kind::Cantera)),
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        WaitIdle("capas del barrio (cajones)"),
        UiShot("motor-ui-cajon-educacion", |s, u| ui_grupo(s, u, crate::city::services::Grupo::Educacion)),
        UiShot("motor-ui-cajon-salud", |s, u| ui_grupo(s, u, crate::city::services::Grupo::Salud)),
        UiShot("motor-ui-cajon-seguridad", |s, u| ui_grupo(s, u, crate::city::services::Grupo::Seguridad)),
        UiShot("motor-ui-cajon-saneamiento", |s, u| ui_grupo(s, u, crate::city::services::Grupo::Saneamiento)),
        CamFn(|s| d::view_of(s, crate::city::services::Kind::Escuela, 120.0)),
        WaitIdle("ficha de la escuela"),
        UiShot("motor-ui-ficha-escuela", |s, u| ui_ficha_servicio(s, u, crate::city::services::Kind::Escuela)),
        Game("incendio con viento del oeste", d::incendio),
        CamFn(d::view_fire),
        Hour(11.0),
        WaitIdle("incendio"),
        Shot("motor-servicios-incendio"),
        UiShot("motor-ui-capa-fuego", |s, u| ui_layer(s, u, crate::city::services::Layer::Fuego)),
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        Hour(11.0),
        WaitIdle("capa de agua"),
        UiShot("motor-ui-capa-agua", |s, u| ui_layer(s, u, crate::city::services::Layer::Agua)),
        UiShot("motor-ui-cajon-puerto", ui_port),
        CamFn(d::view_church),
        Hour(11.0),
        WaitIdle("ficha de la iglesia"),
        UiShot("motor-ui-ficha-iglesia", ui_church),
        Cam([c.x + 40.0, c.y + 30.0, 330.0, 205.0, 40.0]),
        WaitIdle("vista para la interfaz"),
        UiShot("motor-ui-barra", ui_toolbar),
        UiShot("motor-ui-trazar-calle", ui_trazar),
        UiShot("motor-ui-cajon-servicios", ui_services),
        UiShot("motor-ui-zonas", ui_zones),
        UiShot("motor-ui-detalle-pan", |s, u| ui_detail(s, u, ui::Detail::Recurso(crate::city::economy::Res::Pan))),
        UiShot("motor-ui-detalle-clima", |s, u| ui_detail(s, u, ui::Detail::Clima)),
        UiShot("motor-ui-detalle-vecinos", |s, u| ui_detail(s, u, ui::Detail::Opinion(0))),
        UiShot("motor-ui-transito", |s, u| ui_detail(s, u, ui::Detail::Transito)),
        UiShot("motor-ui-ficha-calle", ui_inspect_calle),
        UiShot("motor-ui-ficha", ui_inspect),
        UiShot("motor-ui-gaceta", ui_gazette),
        UiShot("motor-ui-panel", ui_panel),
        // La población: cuarenta años de historia y sus paneles.
        Game("cuarenta años de historia", d::historia),
        WaitIdle("ciudad con historia"),
        UiShot("motor-ui-poblacion", |s, u| ui_pob(s, u, ui::Detail::Poblacion(0))),
        UiShot("motor-ui-poblacion-desigualdad", |s, u| ui_pob(s, u, ui::Detail::Poblacion(1))),
        UiShot("motor-ui-poblacion-felicidad", |s, u| ui_pob(s, u, ui::Detail::Poblacion(2))),
        UiShot("motor-ui-poblacion-cronica", |s, u| ui_pob(s, u, ui::Detail::Poblacion(3))),
        UiShot("motor-ui-vecinos", ui_vecinos),
        UiShot("motor-ui-persona", ui_persona),
        UiShot("motor-ui-persona-liberta", ui_persona_liberta),
        // Guardar y cargar: la ciudad cargada tiene que verse igual que la general.
        Cam([c.x + 60.0, c.y + 40.0, 620.0, 215.0, 42.0]),
        SaveLoad("guardar y cargar la ciudad"),
        WaitIdle("ciudad cargada"),
        Shot("motor-ciudad-cargada"),
        Cam([c.x + 40.0, c.y + 30.0, 330.0, 205.0, 40.0]),
        WaitIdle("vista para el menú de pausa"),
        MenuShot("motor-menu-pausa", menu_pause),
        Cam(crate::menu::BACKGROUND_CAM),
        Hour(crate::menu::BACKGROUND_HOUR),
        WaitIdle("fondo del menú principal"),
        MenuShot("motor-menu-principal", menu_main),
        MenuShot("motor-menu-cargar", menu_load),
        MenuShot("motor-menu-opciones", menu_options),
        MenuShot("motor-menu-nueva", menu_new),
    ]
}

/// Menú de pausa sobre la partida (con la interfaz del juego detrás).
fn menu_pause(s: &mut Session, u: &mut GameUi, m: &mut Menu) {
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    u.open = None;
    u.time += 30.0;
    m.open_pause();
}

/// Menú principal con la ciudad de fondo; deja guardadas otras dos partidas para el listado.
fn menu_main(s: &mut Session, u: &mut GameUi, m: &mut Menu) {
    let t = s.terrain.clone();
    let (cam, hour) = (s.cam_array(), s.hour);
    let g = s.game.as_mut().unwrap();
    // Un autoguardado de hace dos días y una fundación nueva en otro sitio.
    let mut auto = save::snapshot(g, cam, hour, true);
    auto.guardada -= 2 * 86_400 + 3600;
    let _ = save::write(&save::path_for(&m.saves_dir, &g.city_name, true), &auto);
    let mut colonia = crate::city::game::Game::default();
    colonia.city_name = "Colonia del Sacramento".into();
    let _ = colonia.city.found_plaza(&t, crate::city::game_demo::PLAZA, 0.4);
    colonia.city.fast_forward(&t, 7);
    let mut f = save::snapshot(&colonia, crate::menu::new_game_cam(), 10.5, false);
    f.guardada -= 5 * 3600;
    let _ = save::write(&save::unique_path(&m.saves_dir, "Colonia del Sacramento"), &f);
    u.time += 30.0;
    // Como al volver al menú principal: sin partida en curso.
    m.current = None;
    m.stack.clear();
    m.push(Screen::Main);
}

fn menu_load(_s: &mut Session, _u: &mut GameUi, m: &mut Menu) {
    m.push(Screen::Load);
}

fn menu_options(_s: &mut Session, _u: &mut GameUi, m: &mut Menu) {
    m.stack.truncate(1);
    m.push(Screen::Options);
}

fn menu_new(_s: &mut Session, _u: &mut GameUi, m: &mut Menu) {
    m.stack.truncate(1);
    m.push(Screen::NewGame);
    m.name_input = "San Felipe de Montevideo".into();
}

/// Barra de categorías con el cajón de calles abierto (calle recta elegida).
fn ui_toolbar(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(1);
    u.open = Some(ui::Category::Calles);
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(60.0, 90.0));
    u.sync_news(g);
    u.time += 30.0; // sin avisos en esta captura
}

/// Trazando una calle desde una esquina de la plaza con el cursor algo torcido: la calle se
/// engancha a 90° del damero, con la guía punteada y el largo, el ángulo y el costo.
fn ui_trazar(s: &mut Session, u: &mut GameUi) {
    let (corner, f) = {
        let g = s.game.as_ref().unwrap();
        let (c, a) = g.city.plaza.unwrap();
        let (f, side) = crate::city::plaza_frame(a);
        (c + (f + side) * (crate::city::BLOCK * 0.5), f)
    };
    s.cam.target = glam::Vec3::new(corner.x + f.x * 120.0, s.terrain.height(corner.x, corner.y), corner.y + f.y * 120.0);
    s.cam.distance = 380.0;
    s.cam.pitch = 58.0;
    s.update(0.0);
    let g = s.game.as_mut().unwrap();
    let side = crate::city::plaza_frame(g.city.plaza.unwrap().1).1;
    g.inspected = None;
    g.set_tool(1);
    g.p0 = corner;
    g.stage = 1;
    g.hover = Some(corner + f * 260.0 + side * 22.0);
    u.open = Some(ui::Category::Calles);
    u.time += 30.0;
}

/// Cajón de servicios: tarjetas de lo que llega más adelante, desactivadas con el motivo.
fn ui_services(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    g.set_tool(4);
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(60.0, 90.0));
    u.open = Some(ui::Category::Servicios);
}

/// Una pestaña del cajón de Servicios (educación, salud, seguridad, saneamiento).
fn ui_grupo(s: &mut Session, u: &mut GameUi, g: crate::city::services::Grupo) {
    let game = s.game.as_mut().unwrap();
    game.layer = None;
    game.inspected = None;
    game.set_tool(7);
    game.grupo = g;
    u.open = Some(ui::Category::Servicios);
    u.time += 30.0;
}

/// La ficha del primer edificio de un tipo.
fn ui_ficha_servicio(s: &mut Session, u: &mut GameUi, k: crate::city::services::Kind) {
    let game = s.game.as_mut().unwrap();
    game.layer = None;
    game.set_tool(7);
    game.inspected = game.city.services.of_kind(k).next().map(|b| b.c);
    u.open = None;
    u.time += 30.0;
}

/// Una capa de datos (agua o fuego) con el cajón de capas abierto.
fn ui_layer(s: &mut Session, u: &mut GameUi, l: crate::city::services::Layer) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    g.inspected = None;
    g.layer = Some(l);
    u.open = Some(ui::Category::Capas);
    u.sync_news(g);
}

/// Una capa sobre el terreno (recursos o molestias) alrededor de un punto relativo a la plaza.
fn ui_capa_terreno(s: &mut Session, u: &mut GameUi, l: crate::city::services::Layer, rel: Vec2) {
    ui_layer(s, u, l);
    let g = s.game.as_mut().unwrap();
    g.hover = Some(crate::city::game_demo::PLAZA + rel);
}

/// Ficha del terreno en el campo, tierra adentro (suelo CONEAT, geología, recursos).
fn ui_ficha_terreno(s: &mut Session, u: &mut GameUi) {
    u.open = None;
    let g = s.game.as_mut().unwrap();
    g.layer = Some(crate::city::services::Layer::Cultivo);
    g.set_tool(7);
    let p = crate::city::game_demo::PLAZA + Vec2::new(1500.0, -1700.0);
    g.inspected = Some(p);
    g.hover = Some(p);
}

/// Cajón del puerto y la muralla, con la herramienta de muralla elegida.
fn ui_port(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.layer = None;
    g.set_service(crate::city::services::Placeable::Muralla);
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(300.0, 60.0));
    u.open = Some(ui::Category::Puerto);
    u.time += 30.0;
}

/// Ficha de la iglesia matriz en obra.
fn ui_church(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    g.inspected = g.city.church_site().map(|(c, _)| c);
    u.open = None;
}

/// Panel de detalle de una ficha de la barra superior.
fn ui_detail(s: &mut Session, u: &mut GameUi, d: ui::Detail) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    u.open = None;
    u.budget = false;
    u.detail = Some(d);
}

/// Un panel de la población (sin el listado de ordenanzas de la captura anterior).
fn ui_pob(s: &mut Session, u: &mut GameUi, d: ui::Detail) {
    ui_detail(s, u, d);
    u.list.open = false;
    s.game.as_mut().unwrap().inspected = None;
}

/// Los vecinos de la casa con más familias, con su ficha de solar abierta.
fn ui_vecinos(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    u.open = None;
    u.budget = false;
    u.list.open = false;
    let p = &g.city.poblacion;
    let lote = g.city.lots.lots.values().filter(|l| l.level > 0).max_by_key(|l| (p.hogares_de_lote(l.id).count(), p.habitantes_de_lote(l.id).count(), std::cmp::Reverse(l.id))).map(|l| (l.id, l.c));
    if let Some((id, c)) = lote {
        g.inspected = Some(c);
        u.detail = Some(ui::Detail::Hogares(id));
    }
}

/// Ficha de la persona viva con el linaje más largo (y más historia).
fn ui_persona(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    u.open = None;
    let p = &g.city.poblacion;
    let id = p.vivos().filter(|x| x.edad(p.mes) >= 18).max_by_key(|x| (p.linaje(x.id).len(), x.historial.len(), std::cmp::Reverse(x.id))).map(|x| x.id);
    u.detail = id.map(ui::Detail::Persona);
}

/// Ficha de una persona que obtuvo la libertad (o, si no hay, de alguien esclavizado).
fn ui_persona_liberta(s: &mut Session, u: &mut GameUi) {
    use crate::city::poblacion::Condicion;
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    u.open = None;
    let p = &g.city.poblacion;
    let id = p.vivos().filter(|x| x.condicion == Condicion::Liberta).max_by_key(|x| (x.historial.len(), std::cmp::Reverse(x.id))).or_else(|| p.vivos().find(|x| x.condicion == Condicion::Esclavizada)).map(|x| x.id);
    u.detail = id.map(ui::Detail::Persona);
}

/// Cajón de zonas: residencial, comercial e industrial, densidad y medidor de demanda.
fn ui_zones(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.set_tool(4);
    g.zone_kind = crate::city::lots::ZoneKind::Comercial;
    g.density = crate::city::lots::Density::Media;
    g.hover = Some(crate::city::game_demo::PLAZA + Vec2::new(60.0, 90.0));
    u.open = Some(ui::Category::Zonas);
}

/// Ficha de un solar que todavía puede crecer (el de nivel más bajo cerca de la plaza).
fn ui_inspect(s: &mut Session, u: &mut GameUi) {
    u.open = None;
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    // El más cercano al centro de la vista que tenga un motivo concreto para no crecer.
    let view = crate::city::game_demo::PLAZA + Vec2::new(40.0, 30.0);
    let mut cands: Vec<Vec2> = g.city.lots.lots.values().filter(|l| l.level < 2).map(|l| l.c).collect();
    cands.sort_by_key(|p| p.distance(view) as i32);
    let pick = cands.iter().copied().find(|&p| {
        g.inspected = Some(p);
        g.inspect_info().is_some_and(|i| !i.why.is_empty() && !i.why[0].starts_with("Nada"))
    });
    g.inspected = pick.or(cands.first().copied());
    g.hover = g.inspected;
}

/// Ficha de la calle con más gente ahora (tránsito y tiempo de paso medido).
fn ui_inspect_calle(s: &mut Session, u: &mut GameUi) {
    u.open = None;
    u.detail = None;
    let g = s.game.as_mut().unwrap();
    g.set_tool(7);
    let c = &g.city;
    let tramo = c.net.edges.keys().copied().max_by_key(|&e| c.agentes.info_tramo(&c.net, e).map_or(0, |i| i.por_tipo.iter().sum::<u32>()));
    if let Some(e) = tramo {
        let ed = &c.net.edges[&e];
        g.inspected = Some(ed.point(ed.len() * 0.5).0);
        g.hover = g.inspected;
    }
}

/// Un aviso de la gaceta y una calle recta empezada que no se puede trazar (cruza el río).
fn ui_gazette(s: &mut Session, u: &mut GameUi) {
    let terrain = s.terrain.clone();
    let g = s.game.as_mut().unwrap();
    g.inspected = None;
    g.set_tool(1);
    let c = crate::city::game_demo::PLAZA;
    let _ = g.press(&terrain, c + Vec2::new(0.0, 160.0));
    g.hover = Some(Vec2::new(c.x, 19000.0));
    g.say("Llegaron 40 pobladores en la fragata «Santa Rosa»; piden solares cerca del puerto.");
    u.sync_news(g);
}

/// El panel genérico de decisiones con el ejemplo de ordenanzas.
fn ui_panel(s: &mut Session, u: &mut GameUi) {
    let g = s.game.as_mut().unwrap();
    g.cancel();
    g.set_tool(7);
    g.hover = None;
    u.time += 30.0;
    u.list.open = true;
    u.budget = true;
    u.open = Some(ui::Category::Ordenanzas);
}

pub struct Tour {
    steps: Vec<Act>,
    i: usize,
    dir: String,
    width: u32,
    height: u32,
    step_start: Instant,
    travel: Option<(Vec2, Vec2, u32, u32)>,
    pub report: Vec<String>,
    ui: Option<TourUi>,
}

pub fn default_steps() -> Vec<Act> {
    use Act::*;
    let site = (13600.0, 15800.0);
    vec![
        Cam([13500.0, 16300.0, 1600.0, 200.0, 42.0]),
        WaitIdle("vista general"),
        Shot("motor-vista-general"),
        Cam([13200.0, 15000.0, 4200.0, 190.0, 28.0]),
        Smooth(false),
        WaitIdle("vista lejana"),
        Shot("motor-escalones-antes"),
        Smooth(true),
        Shot("motor-escalones-despues"),
        Cam([13500.0, 16300.0, 170.0, 205.0, 30.0]),
        WaitIdle("plaza de cerca"),
        Shot("motor-plaza"),
        Cam([site.0, site.1, 150.0, 200.0, 38.0]),
        WaitIdle("terreno para construir"),
        PlaceCenter(2, 0),
        Place(1, 0, site.0 + 16.0, site.1),
        Place(3, 0, site.0 - 16.0, site.1),
        Place(4, 0, site.0, site.1 + 20.0),
        Place(5, 0, site.0 - 30.0, site.1 + 25.0),
        Place(0, 1, site.0 + 40.0, site.1 + 22.0),
        WaitIdle("construcción"),
        Shot("motor-construir"),
        Cam([site.0, site.1 + 40.0, 70.0, 200.0, 40.0]),
        Dig(site.0 + 10.0, site.1 + 45.0, 9),
        Dig(site.0 + 14.0, site.1 + 45.0, 9),
        Dig(site.0 + 18.0, site.1 + 45.0, 9),
        Fill(site.0 - 12.0, site.1 + 45.0, 7),
        Fill(site.0 - 16.0, site.1 + 45.0, 7),
        WaitIdle("excavar y rellenar"),
        Place(2, 2, site.0 + 2.0, site.1 + 30.0),
        WaitIdle("otra casa, girada"),
        Shot("motor-excavar"),
        Cam([site.0, site.1, 700.0, 200.0, 35.0]),
        Travel(10400.0, 14900.0, 150),
        Cam([10000.0, 15100.0, 1100.0, 200.0, 22.0]),
        WaitIdle("viaje al cerro (3 km)"),
        Shot("motor-cerro"),
        Hour(20.5),
        Cam([13500.0, 16300.0, 450.0, 200.0, 35.0]),
        WaitIdle("vuelta a la plaza de noche"),
        Shot("motor-noche"),
    ]
}

impl Tour {
    pub fn with_steps(dir: &str, width: u32, height: u32, steps: Vec<Act>) -> Tour {
        let _ = std::fs::create_dir_all(dir);
        Tour { steps, i: 0, dir: dir.to_string(), width, height, step_start: Instant::now(), travel: None, report: Vec::new(), ui: None }
    }

    /// La interfaz de las capturas (se arma la primera vez que hace falta).
    fn tour_ui(&mut self, r: &Renderer) -> &mut TourUi {
        let dir = self.dir.clone();
        self.ui.get_or_insert_with(|| {
            let ctx = egui::Context::default();
            ui::theme::install(&ctx);
            // Las partidas del recorrido van a una carpeta propia (se vacía al empezar).
            let base = std::path::Path::new(&dir).join("datos");
            let _ = std::fs::remove_dir_all(&base);
            let mut menu = Menu::new(base, save::Settings::default(), 8, 2, false);
            menu.threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(1).max(1);
            TourUi { ctx, painter: UiPainter::new(r.device(), r.format()), game_ui: GameUi::default(), menu, ms: Vec::new() }
        })
    }

    /// Captura con la interfaz del juego y el menú encima.
    fn ui_capture(&mut self, name: &str, s: &mut Session, r: &mut Renderer) {
        let u = self.ui.as_mut().unwrap();
        r.set_overlay(&if u.menu.in_main_menu() { Vec::new() } else { s.overlay() });
        let ppp = ui::zoom_for(self.height, 1.0);
        let (w, h) = (self.width, self.height);
        let terrain = s.terrain.clone();
        let hour = s.hour;
        let g = s.game.as_ref();
        // Varias pasadas: egui mide las áreas en la primera y las muestra en la siguiente.
        // Las texturas (la fuente) de las pasadas descartadas se suben en orden.
        let mut full = None;
        let t0 = Instant::now();
        for k in 0..3 {
            u.game_ui.time += 0.05;
            let (game_ui, menu) = (&mut u.game_ui, &mut u.menu);
            let mut out = u.ctx.run(ui::headless_input(w, h, ppp, game_ui.time), |ctx| {
                if let Some(g) = g.filter(|_| !menu.in_main_menu()) {
                    let _ = game_ui.draw(ctx, g, &terrain, hour, "");
                }
                let _ = menu.draw(ctx, g, 0.0);
            });
            if k == 2 {
                full = Some(out);
            } else {
                u.painter.update_textures(r.device(), r.queue(), &std::mem::take(&mut out.textures_delta));
            }
        }
        let full = full.unwrap();
        let ms_run = t0.elapsed().as_secs_f64() * 1000.0 / 3.0;
        let path = format!("{}/{}.png", self.dir, name);
        let (ctx, painter) = (&u.ctx, &mut u.painter);
        let mut paint_ms = 0.0;
        let mut out = Some(full);
        let mut hook = |d: &wgpu::Device, q: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size: (u32, u32)| {
            if let Some(f) = out.take() {
                let t = Instant::now();
                painter.paint(d, q, enc, view, size, ctx, f);
                paint_ms = t.elapsed().as_secs_f64() * 1000.0;
            }
        };
        match r.capture_with(&s.cam, s.hour, w, h, &path, Some(&mut hook)) {
            Ok(()) => {
                u.ms.push(ms_run + paint_ms);
                let line = format!("Captura con interfaz {path}: interfaz {:.2} ms de CPU (egui {ms_run:.2} + pintar {paint_ms:.2})", ms_run + paint_ms);
                println!("{line}");
                self.report.push(line);
            }
            Err(e) => eprintln!("Error en la captura {path}: {e}"),
        }
    }

    pub fn done(&self) -> bool {
        self.i >= self.steps.len()
    }

    fn next(&mut self) {
        self.i += 1;
        self.step_start = Instant::now();
    }

    /// Ejecuta los pasos que se puedan en este cuadro. Devuelve `true` al terminar.
    pub fn step(&mut self, s: &mut Session, r: &mut Renderer) -> bool {
        while !self.done() {
            match &self.steps[self.i] {
                Act::Cam(c) => {
                    s.cam.target.x = c[0];
                    s.cam.target.z = c[1];
                    s.cam.target.y = s.terrain.height(c[0], c[1]).max(0.0);
                    s.cam.distance = c[2];
                    s.cam.yaw = c[3];
                    s.cam.pitch = c[4];
                    s.update(0.0);
                    self.next();
                }
                Act::CamFn(f) => {
                    if let Some(c) = f(s) {
                        s.cam.target.x = c[0];
                        s.cam.target.z = c[1];
                        s.cam.target.y = s.terrain.height(c[0], c[1]).max(0.0);
                        s.cam.distance = c[2];
                        s.cam.yaw = c[3];
                        s.cam.pitch = c[4];
                        s.update(0.0);
                    } else {
                        self.report.push("cámara: no hay nada que mirar".into());
                    }
                    self.next();
                }
                Act::Bridge => {
                    if let Some(p) = *crate::city::game_demo::BRIDGE_AT.lock().unwrap() {
                        s.cam.target.x = p.x;
                        s.cam.target.z = p.y;
                        s.cam.target.y = s.terrain.height(p.x, p.y).max(0.0);
                        s.cam.distance = 90.0;
                        s.cam.yaw = 30.0;
                        s.cam.pitch = 28.0;
                        s.update(0.0);
                    }
                    self.next();
                }
                Act::Game(label, f) => {
                    let t0 = Instant::now();
                    let out = f(s);
                    let line = format!("{label}: {out} ({:.2?})", t0.elapsed());
                    println!("{line}");
                    self.report.push(line);
                    self.next();
                }
                Act::Hour(h) => {
                    s.hour = *h;
                    self.next();
                }
                Act::Smooth(on) => {
                    r.smooth_terrain = *on;
                    self.next();
                }
                Act::WaitIdle(label) => {
                    // La escena tiene que estar cargada y todo lo visible generado.
                    if !(s.scene_loaded() && s.streamer.idle()) {
                        if self.step_start.elapsed() > Duration::from_secs(300) {
                            self.report.push(format!("{label}: ¡tiempo agotado esperando el streaming! ({} pendientes)", s.streamer.pending()));
                            self.next();
                        }
                        return false;
                    }
                    crate::session::trim_memory();
                    let (rss, peak) = memory_mb();
                    let line = format!(
                        "{label}: listo en {:.2?} · {} columnas pedidas, {} con malla · {:.2} M triángulos · mallas {:.0} MB · memoria {rss:.0} MB (máx. {peak:.0} MB)",
                        self.step_start.elapsed(),
                        s.streamer.wanted_columns(),
                        r.column_count(),
                        r.triangles() as f32 / 1e6,
                        r.mesh_mb()
                    );
                    println!("{line}");
                    self.report.push(line);
                    self.next();
                }
                Act::Shot(name) => {
                    r.set_overlay(&s.overlay());
                    let path = format!("{}/{}.png", self.dir, name);
                    let t0 = Instant::now();
                    match r.capture(&s.cam, s.hour, self.width, self.height, &path) {
                        Ok(()) => println!("Captura {path} ({:.2?})", t0.elapsed()),
                        Err(e) => eprintln!("Error en la captura {path}: {e}"),
                    }
                    self.next();
                }
                Act::UiShot(name, setup) => {
                    let (name, setup) = (*name, *setup);
                    let u = self.tour_ui(r);
                    setup(s, &mut u.game_ui);
                    self.ui_capture(name, s, r);
                    self.next();
                }
                Act::MenuShot(name, setup) => {
                    let (name, setup) = (*name, *setup);
                    let u = self.tour_ui(r);
                    setup(s, &mut u.game_ui, &mut u.menu);
                    self.ui_capture(name, s, r);
                    self.next();
                }
                Act::SaveLoad(label) => {
                    let label = *label;
                    let u = self.tour_ui(r);
                    let summary = |s: &Session| {
                        let c = &s.game.as_ref().unwrap().city;
                        let sv = &c.services;
                        let mut valor: Vec<(u32, [f32; 3])> = c.lots.valor.iter().map(|(&k, v)| (k, [v.valor, v.molestia, v.atractivo])).collect();
                        valor.sort_by_key(|x| x.0);
                        let rinde: Vec<f32> = sv.buildings.values().map(|b| b.rinde).collect();
                        (c.net.edges.len(), c.lots.lots.len(), c.lots.lots.values().map(|l| l.level as usize).sum::<usize>(), c.population(), c.money, c.month, c.economy.taxes, (sv.buildings.len(), sv.church.view_phase(), sv.walls.tramos.len(), sv.ruins.len(), sv.lamps.len(), sv.water.len()), (sv.barrio.guardar(), sv.barrio.escuela.len(), sv.barrio.delito.len()), serde_json::to_string(&c.poblacion).unwrap_or_default(), (valor, rinde, format!("{:?}", c.suelo.molestias)))
                    };
                    // Lo derivado del barrio (escuelas, delito por solar) y del suelo (molestias,
                    // valor), al día con el estado que se guarda (al cargar se recalcula igual).
                    let terreno = s.terrain.clone();
                    s.game.as_mut().unwrap().city.refresh_barrio();
                    s.game.as_mut().unwrap().city.refresh_suelo(&terreno);
                    let before = summary(s);
                    let t0 = Instant::now();
                    u.menu.perform(MenuAction::Save, s, Some(r), &mut u.game_ui);
                    let t_save = t0.elapsed();
                    let path = u.menu.current.clone().unwrap();
                    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    let t1 = Instant::now();
                    let res = u.menu.load(&path, s, Some(r), &mut u.game_ui);
                    let t_load = t1.elapsed();
                    s.update(0.0);
                    let after = summary(s);
                    let ok = res.is_ok() && before == after;
                    if !ok {
                        println!("  antes:   {before:?}\n  después: {after:?}");
                    }
                    let line = format!(
                        "{label}: {} · guardada en {t_save:.2?} ({} KB) · cargada y regenerada en {t_load:.2?} · {} tramos, {} solares, {} vecinos",
                        if ok { "IGUAL" } else { "¡DISTINTA!" },
                        bytes / 1024,
                        after.0,
                        after.1,
                        after.3
                    );
                    if let Err(e) = res {
                        self.report.push(format!("{label}: error {e}"));
                    }
                    println!("{line}");
                    self.report.push(line);
                    self.next();
                }
                Act::MedirVecinos(label) => {
                    let (w, h, cam, hour) = (self.width, self.height, s.cam, s.hour);
                    let medir = |r: &mut Renderer| {
                        r.bench_frame(&cam, hour, w, h);
                        (0..9).map(|_| r.bench_frame(&cam, hour, w, h).total_ms).fold(f64::INFINITY, f64::min)
                    };
                    let cajas = r.agent_count();
                    let con = medir(r);
                    r.set_agents(&[]);
                    let sin = medir(r);
                    // Multitud de prueba: 2500 agentes (el tope de dibujo) alrededor de la vista.
                    let gente = crate::city::agentes::dibujo::multitud(&s.terrain, glam::Vec2::new(cam.target.x, cam.target.z), crate::city::agentes::dibujo::MAX_DIBUJADOS);
                    let t0 = Instant::now();
                    r.set_agents(&gente);
                    let subir = t0.elapsed();
                    let mucha = medir(r);
                    r.set_agents(&[]);
                    let sin2 = medir(r);
                    let line = format!(
                        "{label}: {cajas} cajas · cuadro (mínimo de 9) con vecinos {con:.1} ms, sin vecinos {sin:.1} ms · con {} agentes de prueba ({} cajas, {} KB, subidas en {subir:.2?}) {mucha:.1} ms · sin vecinos otra vez {sin2:.1} ms",
                        crate::city::agentes::dibujo::MAX_DIBUJADOS,
                        gente.len(),
                        gente.len() * std::mem::size_of::<crate::city::agentes::dibujo::Caja>() / 1024
                    );
                    println!("{line}");
                    self.report.push(line);
                    self.next();
                }
                Act::Place(kind, rot, x, z) => {
                    s.tool = *kind;
                    s.rot = *rot;
                    s.pick_xz(*x, *z);
                    let n = s.place();
                    self.report.push(format!("colocar {} en ({x:.0}, {z:.0}): {:?} columnas re-malladas", s.tool_name(), n));
                    self.next();
                }
                Act::PlaceCenter(kind, rot) => {
                    s.tool = *kind;
                    s.rot = *rot;
                    let (o, d) = s.cam.ray(self.width as f32 * 0.5, self.height as f32 * 0.5, self.width as f32, self.height as f32);
                    let hit = s.pick(o, d);
                    let n = s.place();
                    self.report.push(format!("colocar {} con el rayo del centro de pantalla (vóxel {:?}): {:?} columnas re-malladas", s.tool_name(), hit.map(|h| h.voxel), n));
                    self.next();
                }
                Act::Dig(x, z, rad) => {
                    s.pick_xz(*x, *z);
                    let n = s.dig(*rad);
                    self.report.push(format!("excavar en ({x:.0}, {z:.0}) radio {rad}: {:?} columnas re-malladas", n));
                    self.next();
                }
                Act::Fill(x, z, rad) => {
                    s.pick_xz(*x, *z);
                    let n = s.fill(*rad);
                    self.report.push(format!("rellenar en ({x:.0}, {z:.0}) radio {rad}: {:?} columnas re-malladas", n));
                    self.next();
                }
                Act::Travel(x, z, frames) => {
                    let (from, to, k, n) = *self.travel.get_or_insert((Vec2::new(s.cam.target.x, s.cam.target.z), Vec2::new(*x, *z), 0, *frames));
                    let p = from.lerp(to, (k + 1) as f32 / n as f32);
                    s.cam.target.x = p.x;
                    s.cam.target.z = p.y;
                    s.update(1.0 / 30.0);
                    if k + 1 >= n {
                        self.travel = None;
                        self.report.push(format!("viaje de {:.0} m en {n} cuadros: {:.2?}", from.distance(to), self.step_start.elapsed()));
                        self.next();
                    } else {
                        self.travel = Some((from, to, k + 1, n));
                    }
                    return false;
                }
            }
        }
        true
    }
}
