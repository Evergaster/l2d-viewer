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

use std::sync::Arc;

use iced::widget::{button, column, container, image, row, scrollable, slider, text, Space};
use iced::{Alignment, Element, Length, Task};

/// Ancho al que se dibuja el modelo para la ventana (el PNG se guarda al doble).
const ANCHO_DIBUJO: u32 = 560;
const ANCHO_PANEL: f32 = 300.0;

fn main() -> iced::Result {
    let ruta = std::env::args().nth(1);
    iced::application(
        move || Visor::nuevo(ruta.clone()),
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
    .window(iced::window::Settings {
        size: iced::Size::new(1180.0, 820.0),
        min_size: Some(iced::Size::new(860.0, 560.0)),
        ..iced::window::Settings::default()
    })
    .run()
}

struct Visor {
    modelo: Option<Arc<view::Vista>>,
    valores: Vec<f32>,
    imagen: Option<image::Handle>,
    error: Option<String>,
    ms: u128,
    claro: bool,
    mensaje: String,
}

#[derive(Debug, Clone)]
enum Msg {
    Param(usize, f32),
    Reposo,
    Guardar,
    Tema,
    Imagen(image::Handle, u128),
}

impl Visor {
    fn nuevo(ruta: Option<String>) -> (Self, Task<Msg>) {
        let mut v = Self {
            modelo: None,
            valores: Vec::new(),
            imagen: None,
            error: None,
            ms: 0,
            claro: false,
            mensaje: String::new(),
        };
        match ruta {
            Some(r) => match view::Vista::cargar(&r) {
                Ok(vista) => {
                    v.valores = vista.valores_iniciales();
                    v.mensaje = format!(
                        "{} · {} mallas · {} texturas",
                        vista.nombre,
                        vista.body.art_meshes.len(),
                        vista.texturas.len()
                    );
                    v.modelo = Some(Arc::new(vista));
                    let t = v.redibujar();
                    return (v, t);
                }
                Err(e) => v.error = Some(format!("no pude abrir «{r}»: {e}")),
            },
            None => {
                v.error =
                    Some("pasa la ruta de un modelo: l2d-ui <modelo.moc3>".to_string());
            }
        }
        (v, Task::none())
    }

    /// Redibuja con los valores actuales y devuelve la tarea que entrega la imagen.
    fn redibujar(&self) -> Task<Msg> {
        let Some(vista) = self.modelo.clone() else {
            return Task::none();
        };
        let valores = self.valores.clone();
        let t0 = std::time::Instant::now();
        let canvas = vista.render(&valores, ANCHO_DIBUJO);
        let ms = t0.elapsed().as_millis();
        let handle = image::Handle::from_rgba(canvas.width, canvas.height, canvas.pixels.clone());
        Task::done(Msg::Imagen(handle, ms))
    }

    fn actualizar(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Param(i, valor) => {
                if let Some(v) = self.valores.get_mut(i) {
                    *v = valor;
                }
                return self.redibujar();
            }
            Msg::Reposo => {
                if let Some(vista) = &self.modelo {
                    self.valores = vista.valores_iniciales();
                }
                return self.redibujar();
            }
            Msg::Guardar => {
                let Some(vista) = &self.modelo else {
                    return Task::none();
                };
                let canvas = vista.render(&self.valores, 1400);
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
        }
        Task::none()
    }

    fn vista(&self) -> Element<'_, Msg> {
        if let Some(error) = &self.error {
            return container(text(error.clone()).size(15))
                .padding(24)
                .center(Length::Fill)
                .into();
        }

        let panel = self.panel_parametros();
        let lienzo = self.lienzo();
        let pie = self.pie();

        let cuerpo = row![
            container(panel).width(Length::Fixed(ANCHO_PANEL)).height(Length::Fill),
            container(lienzo).width(Length::Fill).height(Length::Fill),
        ];

        container(column![cuerpo, pie])
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn panel_parametros(&self) -> Element<'_, Msg> {
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
                        text(format!("{valor:+.2}")).size(12).width(Length::Fixed(52.0)),
                    ],
                    slider(rango, valor, move |v| Msg::Param(i, v)).step(0.01_f32),
                ]
                .spacing(1),
            );
        }

        column![
            text("Parámetros").size(13),
            Space::new().height(6),
            scrollable(lista).height(Length::Fill),
            Space::new().height(8),
            button(text("Volver a reposo")).on_press(Msg::Reposo).width(Length::Fill),
            button(text("Guardar PNG")).on_press(Msg::Guardar).width(Length::Fill),
            button(text("Cambiar tema")).on_press(Msg::Tema).width(Length::Fill),
        ]
        .spacing(6)
        .padding(12)
        .into()
    }

    fn lienzo(&self) -> Element<'_, Msg> {
        match &self.imagen {
            Some(h) => container(
                image(h.clone()).content_fit(iced::ContentFit::Contain),
            )
            .padding(12)
            .center(Length::Fill)
            .into(),
            None => container(text("dibujando…")).center(Length::Fill).into(),
        }
    }

    fn pie(&self) -> Element<'_, Msg> {
        let estado = if self.mensaje.is_empty() {
            format!("{} · dibujo {} ms", self.info(), self.ms)
        } else {
            format!("{} · {} · dibujo {} ms", self.info(), self.mensaje, self.ms)
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
