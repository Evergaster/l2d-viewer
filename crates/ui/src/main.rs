//! `l2d-ui` — visor con interfaz para modelos Live2D sobre el **motor propio**.
//!
//! No carga el Core de Live2D: la geometría la calcula `pose` y la pinta `view`/`raster` en CPU.
//! La interfaz sólo muestra el resultado y mueve parámetros.
//!
//! Decisiones:
//!
//! 1. **El modelo se dibuja en CPU**, no con shaders. Es determinista, coincide con las pruebas
//!    (`l2d pose` mide 134/134 contra el Core) y evita depender del EGL del equipo, que en esta
//!    máquina es quisquilloso con la NVIDIA.
//! 2. **Cada cambio de parámetro redibuja.** A 560 px tarda unas décimas; si algún modelo grande
//!    lo hace incómodo, el ancho de dibujo es una constante y se baja sin tocar nada más.
//! 3. **Un botón por cosa**: reposo, guardar PNG. Sin menús ni estados ocultos.
//! 4. **El selector de archivos es `zenity`**, lanzado en una tarea aparte. No añade dependencias
//!    (ni GTK, ni portal, ni 30 crates de `rfd`) y en este escritorio ya está instalado. Si un día
//!    no está, el visor lo dice en el pie en vez de morir.
//! 5. **La animación manda mientras se reproduce.** Los deslizadores se pueden mover, pero el
//!    siguiente cuadro los sobrescribe con lo que diga el archivo; al pausar, el estado queda en la
//!    última pose dibujada y se puede editar a mano desde ahí.

use std::sync::Arc;
use std::time::{Duration, Instant};

use iced::widget::{
    button, checkbox, column, container, image, row, scrollable, slider, text, Space,
};
use iced::{event, keyboard, mouse, Alignment, Element, Length, Subscription, Task};

/// Ancho al que se dibuja el modelo para la ventana (el PNG se guarda al doble).
const ANCHO_DIBUJO: u32 = 560;
/// Ancho durante la reproducción. Medido con `cargo run --release -p view --example banco` y
/// confirmado dentro del bucle con `L2D_UI_LOG=1`: a 560 px una pose cuesta ~25 ms (40 cuadros/s
/// de techo) y a 420 px ~29 ms **dentro de la ventana** (más que en el banco porque la caché se
/// enfría entre cuadros), así que la ventana reproduce a 15 cuadros/s con ~44 % de un núcleo. En
/// pausa se redibuja a 560 px, que es el ancho de trabajo.
const ANCHO_ANIMACION: u32 = 420;
const ANCHO_PANEL: f32 = 300.0;
/// Cadencia de la reproducción. 15 cuadros/s bastan para juzgar una animación de prueba y dejan
/// un núcleo libre; con 24 el visor se comía ~75 % de uno.
const CUADRO_MS: u64 = 66;

/// Argumentos: `[modelo.moc3] [anim.json] [--zoom N] [--centro X,Y]`, en cualquier orden.
///
/// `--centro` va en píxeles del lienzo del modelo con origen arriba-izquierda (como se lee en el
/// PNG), así que `--zoom 4 --centro 1488,700` abre el visor en la cara.
fn argumentos() -> (
    Option<String>,
    Option<String>,
    Option<f32>,
    Option<(f32, f32)>,
) {
    let mut ruta = None;
    let mut anim = None;
    let mut zoom = None;
    let mut centro = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--zoom" => {
                zoom = args.get(i + 1).and_then(|s| s.parse::<f32>().ok());
                i += 2;
            }
            "--centro" => {
                centro = args.get(i + 1).and_then(|s| {
                    let (x, y) = s.split_once(',')?;
                    Some((x.trim().parse::<f32>().ok()?, y.trim().parse::<f32>().ok()?))
                });
                i += 2;
            }
            otro => {
                if ruta.is_none() {
                    ruta = Some(otro.to_string());
                } else if anim.is_none() {
                    anim = Some(otro.to_string());
                }
                i += 1;
            }
        }
    }
    (ruta, anim, zoom, centro)
}

fn main() -> iced::Result {
    let (ruta, anim, zoom, centro) = argumentos();
    iced::application(
        move || Visor::nuevo(ruta.clone(), anim.clone(), zoom, centro),
        Visor::actualizar,
        Visor::vista,
    )
    .title("l2d · visor (motor propio)")
    .theme(|v: &Visor| {
        if v.claro {
            iced::Theme::Light
        } else {
            iced::Theme::Dark
        }
    })
    .subscription(Visor::suscripcion)
    .window(iced::window::Settings {
        size: iced::Size::new(1180.0, 860.0),
        min_size: Some(iced::Size::new(860.0, 560.0)),
        ..iced::window::Settings::default()
    })
    .run()
}

struct Visor {
    modelo: Option<Arc<view::Vista>>,
    valores: Vec<f32>,
    /// Una casilla por capa (parte): `true` = oculta.
    ocultas: Vec<bool>,
    imagen: Option<image::Handle>,
    error: Option<String>,
    ms: u128,
    claro: bool,
    mensaje: String,
    /// Animación cargada, ya atada a los parámetros de este modelo.
    anim: Option<Arc<view::anim::Animacion>>,
    reproduciendo: bool,
    /// Instante dentro de la animación, en segundos.
    t: f32,
    ultimo_cuadro: Option<Instant>,
    /// Animación pedida por línea de comandos, para cargarla en cuanto haya modelo.
    anim_pendiente: Option<String>,
    /// Diagnóstico: cuadros y milisegundos acumulados de dibujo (con `L2D_UI_LOG=1`).
    cuadros: u32,
    ms_acumulados: f64,
    /// Cámara: 1.0 = modelo entero. El centro va en el eje del modelo (y hacia arriba).
    zoom: f32,
    centro: [f32; 2],
    /// Centro pedido por línea de comandos, para aplicarlo en cuanto haya modelo.
    centro_inicial: Option<(f32, f32)>,
}

#[derive(Clone)]
enum Msg {
    Param(usize, f32),
    Capa(usize, bool),
    Reposo,
    Guardar,
    Tema,
    Imagen(image::Handle, u128),
    AbrirModelo,
    ModeloElegido(Option<String>),
    ModeloCargado(Result<Arc<view::Vista>, String>),
    AbrirAnimacion,
    AnimacionElegida(Option<String>),
    AnimacionCargada(Box<Result<view::anim::Animacion, String>>),
    Alternar,
    Detener,
    Cuadro(Instant),
    CerrarError,
    Zoom(f32),
    Pan(f32, f32),
    Todo,
}

impl Visor {
    fn nuevo(
        ruta: Option<String>,
        anim: Option<String>,
        zoom: Option<f32>,
        centro: Option<(f32, f32)>,
    ) -> (Self, Task<Msg>) {
        let v = Self {
            modelo: None,
            valores: Vec::new(),
            ocultas: Vec::new(),
            imagen: None,
            error: None,
            ms: 0,
            claro: false,
            mensaje: String::new(),
            anim: None,
            reproduciendo: false,
            t: 0.0,
            ultimo_cuadro: None,
            anim_pendiente: anim,
            cuadros: 0,
            ms_acumulados: 0.0,
            zoom: zoom.unwrap_or(1.0),
            centro: [0.0, 0.0],
            centro_inicial: centro,
        };
        match ruta {
            Some(r) => (v, visor_cargar(r)),
            None => (v, Task::none()),
        }
    }

    /// El modelo nuevo entra con sus valores de reposo y sin animación heredada.
    fn poner_modelo(&mut self, r: Result<Arc<view::Vista>, String>) -> Task<Msg> {
        match r {
            Ok(vista) => {
                self.valores = vista.valores_iniciales();
                self.ocultas = vista.ocultas_por_defecto();
                self.mensaje = format!(
                    "{} · {} mallas · {} texturas",
                    vista.nombre,
                    vista.body.art_meshes.len(),
                    vista.texturas.len()
                );
                let lienzo = vista.lienzo;
                self.centro = [lienzo.0 / 2.0, lienzo.1 / 2.0];
                if let Some((x, y)) = self.centro_inicial.take() {
                    // `centrado_en` recibe coordenadas de imagen y las pasa al eje del modelo
                    let e = view::Encuadre {
                        zoom: self.zoom,
                        centro: self.centro,
                    }
                    .centrado_en(&vista, x, y);
                    self.centro = e.centro;
                }
                self.modelo = Some(vista);
                self.anim = None;
                self.reproduciendo = false;
                self.t = 0.0;
                self.ultimo_cuadro = None;
                self.error = None;

                // Animación pedida por línea de comandos: se carga en cuanto el modelo existe.
                if let Some(ruta) = self.anim_pendiente.take() {
                    let ids: Vec<String> = self
                        .modelo
                        .as_ref()
                        .map(|v| v.parametros.iter().map(|p| p.id.clone()).collect())
                        .unwrap_or_default();
                    match view::anim::Animacion::cargar(&ruta, &ids) {
                        Ok(a) => {
                            self.mensaje = format!("animación: {}", a.resumen());
                            a.aplicar(0.0, &mut self.valores);
                            self.anim = Some(Arc::new(a));
                            self.reproduciendo = true;
                        }
                        Err(e) => self.mensaje = format!("animación rechazada: {e}"),
                    }
                }
                if std::env::var_os("L2D_UI_LOG").is_some() {
                    eprintln!(
                        "l2d-ui: modelo {} · lienzo {:.0}×{:.0} · zoom {:.2}× · centro ({:.0},{:.0})",
                        self.modelo
                            .as_ref()
                            .map(|v| v.nombre.clone())
                            .unwrap_or_default(),
                        self.modelo.as_ref().map(|v| v.lienzo.0).unwrap_or(0.0),
                        self.modelo.as_ref().map(|v| v.lienzo.1).unwrap_or(0.0),
                        self.zoom,
                        self.centro[0],
                        self.centro[1]
                    );
                }
                self.redibujar()
            }
            Err(e) => {
                self.error = Some(e);
                Task::none()
            }
        }
    }

    /// Redibuja con los valores actuales y devuelve la tarea que entrega la imagen.
    ///
    /// Con `L2D_UI_LOG=1` escribe en stderr cada 24 cuadros el tiempo medio de dibujo: es la
    /// única forma de saber si el bucle se sostiene sin mirar la ventana.
    fn redibujar(&mut self) -> Task<Msg> {
        let Some(vista) = self.modelo.clone() else {
            return Task::none();
        };
        let valores = self.valores.clone();
        let ocultas = self.ocultas.clone();
        let ancho = if self.reproduciendo {
            ANCHO_ANIMACION
        } else {
            ANCHO_DIBUJO
        };
        let encuadre = view::Encuadre {
            zoom: self.zoom,
            centro: self.centro,
        };
        let t0 = Instant::now();
        let canvas = vista.render_encuadre(&valores, ancho, &ocultas, encuadre);
        let ms = t0.elapsed().as_millis();

        self.cuadros += 1;
        self.ms_acumulados += t0.elapsed().as_secs_f64() * 1000.0;
        if self.cuadros % 24 == 0 && std::env::var_os("L2D_UI_LOG").is_some() {
            eprintln!(
                "l2d-ui: {} cuadros · dibujo medio {:.1} ms · {} px · zoom {:.2}× centro ({:.0},{:.0})",
                self.cuadros,
                self.ms_acumulados / 24.0,
                ancho,
                self.zoom,
                self.centro[0],
                self.centro[1]
            );
            self.ms_acumulados = 0.0;
        }

        let handle = image::Handle::from_rgba(canvas.width, canvas.height, canvas.pixels);
        Task::done(Msg::Imagen(handle, ms))
    }

    fn suscripcion(&self) -> Subscription<Msg> {
        // Rueda del ratón para acercar, flechas para moverse, 0 para volver al modelo entero.
        // Se ignoran los eventos que algún widget ya consumió (`Status::Ignored`), así pasar la
        // rueda sobre la lista de parámetros no mueve la cámara.
        let camara = event::listen_with(|ev, estado, _id| {
            if estado != event::Status::Ignored {
                return None;
            }
            match ev {
                event::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    let d = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y,
                        mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                    };
                    if d.abs() < 0.01 {
                        None
                    } else if d > 0.0 {
                        Some(Msg::Zoom(1.15))
                    } else {
                        Some(Msg::Zoom(1.0 / 1.15))
                    }
                }
                event::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                    let paso = 0.08;
                    match key.as_ref() {
                        keyboard::Key::Character("+") | keyboard::Key::Character("=") => {
                            Some(Msg::Zoom(1.25))
                        }
                        keyboard::Key::Character("-") | keyboard::Key::Character("_") => {
                            Some(Msg::Zoom(0.8))
                        }
                        keyboard::Key::Character("0") => Some(Msg::Todo),
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            Some(Msg::Pan(-paso, 0.0))
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            Some(Msg::Pan(paso, 0.0))
                        }
                        // el eje del modelo apunta hacia arriba: subir la cámara es +y
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            Some(Msg::Pan(0.0, paso))
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            Some(Msg::Pan(0.0, -paso))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        });

        if self.reproduciendo {
            // `every` entrega el instante del disparo: el delta real sale de ahí, no de suponer
            // que cada tick llegó a tiempo (con el CPU ocupado llegan tarde y la animación se
            // arrastraría).
            Subscription::batch([
                iced::time::every(Duration::from_millis(CUADRO_MS)).map(Msg::Cuadro),
                camara,
            ])
        } else {
            camara
        }
    }

    /// Avanza el reloj de la animación y deja los valores de parámetro en esa pose.
    fn avanzar(&mut self, ahora: Instant) {
        let Some(anim) = self.anim.clone() else {
            self.reproduciendo = false;
            return;
        };
        let dt = match self.ultimo_cuadro {
            Some(prev) => ahora.saturating_duration_since(prev).as_secs_f32(),
            None => 0.0,
        };
        // Un salto grande (ventana minimizada, primer cuadro) no debe saltar media animación.
        let dt = dt.min(0.25);
        self.ultimo_cuadro = Some(ahora);
        self.t = anim.envolver(self.t + dt);
        anim.aplicar(self.t, &mut self.valores);
    }

    fn actualizar(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Param(i, valor) => {
                if let Some(v) = self.valores.get_mut(i) {
                    *v = valor;
                }
                return self.redibujar();
            }
            Msg::Capa(i, oculta) => {
                if let Some(c) = self.ocultas.get_mut(i) {
                    *c = oculta;
                }
                return self.redibujar();
            }
            Msg::Reposo => {
                if let Some(vista) = &self.modelo {
                    self.valores = vista.valores_iniciales();
                    self.ocultas = vista.ocultas_por_defecto();
                }
                self.reproduciendo = false;
                self.t = 0.0;
                self.ultimo_cuadro = None;
                return self.redibujar();
            }
            Msg::Guardar => {
                let Some(vista) = &self.modelo else {
                    return Task::none();
                };
                let encuadre = view::Encuadre {
                    zoom: self.zoom,
                    centro: self.centro,
                };
                let canvas = vista.render_encuadre(&self.valores, 1400, &self.ocultas, encuadre);
                let ruta = format!("{}.png", vista.nombre);
                match canvas.save_png(&ruta) {
                    Ok(()) => self.mensaje = format!("guardado {ruta}"),
                    Err(e) => self.mensaje = format!("no pude guardar: {e}"),
                }
            }
            Msg::Tema => self.claro = !self.claro,
            Msg::Imagen(handle, ms) => {
                self.imagen = Some(handle);
                self.ms = ms;
            }
            Msg::AbrirModelo => {
                return Task::perform(
                    elegir_archivo("Abrir modelo", "Modelos Live2D", "*.moc3"),
                    Msg::ModeloElegido,
                );
            }
            Msg::ModeloElegido(Some(ruta)) => return visor_cargar(ruta),
            Msg::ModeloElegido(None) => return Task::none(),
            Msg::ModeloCargado(r) => return self.poner_modelo(r),
            Msg::AbrirAnimacion => {
                if self.modelo.is_none() {
                    self.mensaje = "abre un modelo antes que la animación".to_string();
                    return Task::none();
                }
                return Task::perform(
                    elegir_archivo("Abrir animación", "Animación l2d", "*.json"),
                    Msg::AnimacionElegida,
                );
            }
            Msg::AnimacionElegida(Some(ruta)) => {
                let Some(vista) = self.modelo.clone() else {
                    return Task::none();
                };
                let ids: Vec<String> = vista.parametros.iter().map(|p| p.id.clone()).collect();
                return Task::perform(
                    async move { Box::new(view::anim::Animacion::cargar(&ruta, &ids)) },
                    Msg::AnimacionCargada,
                );
            }
            Msg::AnimacionElegida(None) => return Task::none(),
            Msg::AnimacionCargada(r) => match *r {
                Ok(anim) => {
                    self.mensaje = format!("animación: {}", anim.resumen());
                    anim.aplicar(0.0, &mut self.valores);
                    self.anim = Some(Arc::new(anim));
                    self.t = 0.0;
                    self.reproduciendo = true;
                    self.ultimo_cuadro = None;
                    self.error = None;
                    return self.redibujar();
                }
                Err(e) => {
                    // Un archivo rechazado no toca la animación abierta (spec §Reglas).
                    self.mensaje = format!("animación rechazada: {e}");
                }
            },
            Msg::Alternar => {
                if self.anim.is_none() {
                    self.mensaje = "no hay animación cargada".to_string();
                    return Task::none();
                }
                self.reproduciendo = !self.reproduciendo;
                self.ultimo_cuadro = None;
                // al pausar se redibuja al ancho de detalle; al reanudar, al de animación
                return self.redibujar();
            }
            Msg::Detener => {
                self.reproduciendo = false;
                self.t = 0.0;
                self.ultimo_cuadro = None;
                if let Some(vista) = &self.modelo {
                    self.valores = vista.valores_iniciales();
                }
                if let Some(anim) = &self.anim {
                    anim.aplicar(0.0, &mut self.valores);
                }
                return self.redibujar();
            }
            Msg::Cuadro(ahora) => {
                if !self.reproduciendo {
                    return Task::none();
                }
                self.avanzar(ahora);
                return self.redibujar();
            }
            Msg::CerrarError => self.error = None,
            Msg::Zoom(factor) => {
                self.zoom = (self.zoom * factor).clamp(0.5, 12.0);
                return self.redibujar();
            }
            Msg::Pan(fx, fy) => {
                if let Some(v) = &self.modelo {
                    let (cw, ch) = v.lienzo;
                    self.centro[0] = (self.centro[0] + fx * cw).clamp(0.0, cw);
                    self.centro[1] = (self.centro[1] + fy * ch).clamp(0.0, ch);
                }
                return self.redibujar();
            }
            Msg::Todo => {
                if let Some(v) = &self.modelo {
                    self.centro = [v.lienzo.0 / 2.0, v.lienzo.1 / 2.0];
                }
                self.zoom = 1.0;
                return self.redibujar();
            }
        }
        Task::none()
    }

    fn vista(&self) -> Element<'_, Msg> {
        if let Some(error) = &self.error {
            let mut col = column![text(error.clone()).size(15)].spacing(12);
            if self.modelo.is_none() {
                col = col.push(button(text("Abrir modelo…")).on_press(Msg::AbrirModelo));
            }
            if let Some(m) = self.modelo.clone() {
                // el modelo sigue vivo: se puede recuperar la vista sin reiniciar
                col = col.push(
                    button(text(format!("volver a {}", m.nombre))).on_press(Msg::CerrarError),
                );
            }
            return container(col).padding(24).center(Length::Fill).into();
        }

        let panel = self.panel();
        let lienzo = self.lienzo();
        let pie = self.pie();

        let cuerpo = row![
            container(panel)
                .width(Length::Fixed(ANCHO_PANEL))
                .height(Length::Fill),
            container(lienzo).width(Length::Fill).height(Length::Fill),
        ];

        container(column![cuerpo, pie])
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Panel izquierdo: archivos, animación, **capas** y **parámetros**.
    fn panel(&self) -> Element<'_, Msg> {
        let mut capas = column![].spacing(2);
        for (i, nombre) in self
            .modelo
            .as_ref()
            .map(|v| v.partes.as_slice())
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let oculta = self.ocultas.get(i).copied().unwrap_or(false);
            capas = capas.push(
                checkbox(!oculta)
                    .label(nombre.clone())
                    .on_toggle(move |visible: bool| Msg::Capa(i, !visible))
                    .size(16),
            );
        }

        let estado_anim = match (&self.anim, self.modelo.is_some()) {
            (Some(a), _) => format!(
                "{} · {:.1}/{:.1} s{}",
                a.nombre,
                self.t,
                a.duracion,
                if self.reproduciendo {
                    " · ▶"
                } else {
                    " · ⏸"
                }
            ),
            (None, true) => "sin animación".to_string(),
            (None, false) => "sin modelo".to_string(),
        };

        let mut cuerpo = column![
            button(text("Abrir modelo…"))
                .on_press(Msg::AbrirModelo)
                .width(Length::Fill),
            button(text("Abrir animación…"))
                .on_press(Msg::AbrirAnimacion)
                .width(Length::Fill),
            text(estado_anim).size(11),
            row![
                button(text(if self.reproduciendo {
                    "Pausar"
                } else {
                    "Reproducir"
                }))
                .on_press(Msg::Alternar)
                .width(Length::Fill),
                button(text("Detener"))
                    .on_press(Msg::Detener)
                    .width(Length::Fill),
            ]
            .spacing(4),
            Space::new().height(6),
            text("Capas").size(13),
            Space::new().height(4),
            scrollable(capas).height(Length::Fixed(140.0)),
            Space::new().height(8),
        ];
        cuerpo = cuerpo.push(self.lista_parametros());
        cuerpo = cuerpo.push(Space::new().height(6)).push(
            row![
                button(text("+"))
                    .on_press(Msg::Zoom(1.25))
                    .width(Length::Fill),
                button(text("−"))
                    .on_press(Msg::Zoom(0.8))
                    .width(Length::Fill),
                button(text("Todo")).on_press(Msg::Todo).width(Length::Fill),
            ]
            .spacing(4),
        );
        cuerpo
            .push(Space::new().height(8))
            .push(
                button(text("Volver a reposo"))
                    .on_press(Msg::Reposo)
                    .width(Length::Fill),
            )
            .push(
                button(text("Guardar PNG"))
                    .on_press(Msg::Guardar)
                    .width(Length::Fill),
            )
            .push(
                button(text("Cambiar tema"))
                    .on_press(Msg::Tema)
                    .width(Length::Fill),
            )
            .into()
    }

    fn lista_parametros(&self) -> Element<'_, Msg> {
        let mut lista = column![].spacing(6);
        for (i, p) in self
            .modelo
            .as_ref()
            .map(|v| v.parametros.as_slice())
            .unwrap_or(&[])
            .iter()
            .enumerate()
        {
            let valor = self.valores.get(i).copied().unwrap_or(0.0);
            let rango = p.min..=p.max;
            let nombre = p.id.strip_prefix("Param").unwrap_or(&p.id).to_string();
            lista = lista.push(
                column![
                    row![
                        text(nombre).size(12).width(Length::Fixed(130.0)),
                        text(format!("{valor:+.2}"))
                            .size(12)
                            .width(Length::Fixed(52.0)),
                    ],
                    slider(rango, valor, move |v| Msg::Param(i, v)).step(0.01_f32),
                ]
                .spacing(1),
            );
        }

        column![
            text("Parámetros").size(13),
            Space::new().height(4),
            scrollable(lista).height(Length::Fill),
        ]
        .spacing(2)
        .height(Length::Fill)
        .into()
    }

    fn lienzo(&self) -> Element<'_, Msg> {
        match &self.imagen {
            Some(h) => container(image(h.clone()).content_fit(iced::ContentFit::Contain))
                .padding(12)
                .center(Length::Fill)
                .into(),
            None => container(text("dibujando…")).center(Length::Fill).into(),
        }
    }

    fn pie(&self) -> Element<'_, Msg> {
        let camara = format!(
            "zoom {:.2}× · rueda/+/- · flechas mueven · 0 todo",
            self.zoom
        );
        let estado = if self.mensaje.is_empty() {
            format!("{} · {} · dibujo {} ms", self.info(), camara, self.ms)
        } else {
            format!(
                "{} · {} · {} · dibujo {} ms",
                self.info(),
                self.mensaje,
                camara,
                self.ms
            )
        };
        container(text(estado).size(11))
            .padding([6, 12])
            .width(Length::Fill)
            .align_x(Alignment::Start)
            .into()
    }

    fn info(&self) -> String {
        match &self.modelo {
            Some(v) => format!(
                "{} · {} mallas · {}×{} px",
                v.nombre,
                v.body.art_meshes.len(),
                v.lienzo.0 as u32,
                v.lienzo.1 as u32
            ),
            None => "sin modelo".to_string(),
        }
    }
}

// ---------------------------------------------------------------- archivos

/// Carga un modelo por la ruta que llega en la línea de comandos.
fn visor_cargar(ruta: String) -> Task<Msg> {
    Task::perform(
        async move {
            view::Vista::cargar(&ruta)
                .map(Arc::new)
                .map_err(|e| format!("no pude abrir «{ruta}»: {e}"))
        },
        Msg::ModeloCargado,
    )
}

/// Abre el selector nativo con `zenity` y devuelve la ruta elegida (`None` si se cancela).
///
/// Se ejecuta en una tarea aparte: el diálogo bloquea hasta que el usuario responde.
async fn elegir_archivo(
    titulo: &'static str,
    etiqueta: &'static str,
    patron: &'static str,
) -> Option<String> {
    let salida = std::process::Command::new("zenity")
        .arg("--file-selection")
        .arg(format!("--title={titulo}"))
        .arg(format!("--file-filter={etiqueta} | {patron}"))
        .arg("--file-filter=Todos | *")
        .output()
        .ok()?;
    if !salida.status.success() {
        return None; // el usuario canceló
    }
    let ruta = String::from_utf8_lossy(&salida.stdout).trim().to_string();
    if ruta.is_empty() {
        None
    } else {
        Some(ruta)
    }
}
