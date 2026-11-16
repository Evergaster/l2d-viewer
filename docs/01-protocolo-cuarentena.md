# 01 — Protocolo de cuarentena (clean room)

Esto no es burocracia. Es la única forma de que «reconstrucción independiente» signifique algo
ante un juez y no solo ante ti.

## El principio legal, en una línea

El derecho de autor protege **la expresión**, no **las ideas ni las funciones**. Un formato de
archivo, un algoritmo y una interfaz funcional no son protegibles; el código concreto que los
implementa, sí. Y **traducir código = crear una obra derivada**: si abres el bytecode de Live2D y
lo reescribes en Rust, eso es su código traducido, por más que cambien las llaves de sitio.

Por eso el método importa más que el lenguaje.

## Las cinco reglas

### R1 — El código y el análisis no se tocan
`l2d-viewer` no importa, copia, pega ni consulta nada de
`~/Proyectos/live2d-cubism-linux/` salvo **documentos de comportamiento** (`docs/07-bitacora`,
observaciones de qué hace la app al usarla). Nunca `Live2D_Cubism.jar`, nunca un `.class`
descompilado, nunca una traducción de una función suya.

### R2 — Dos sombreros, dos personas
El proceso clean room clásico (el que usó Compaq para la BIOS del PC) separa:

| Sombrero | Puede | No puede |
|---|---|---|
| **Analista** | ejecutar el producto, observar entradas/salidas, leer especificaciones públicas, escribir prosa técnica | escribir código del proyecto |
| **Implementador** | leer solo la especificación en prosa y las pruebas | mirar el binario, el bytecode o el análisis del analista |

En un proyecto de una sola persona esto se simula con una disciplina: se escribe la spec primero,
en `docs/`, y el código se escribe **desde la spec**, con el binario cerrado. Si en mitad de una
función hace falta «ver cómo lo hace», se cierra el editor de Rust, se documenta en prosa lo
observado, y se vuelve al código.

### R3 — Solo información pública y observable
Válido: la especificación pública del formato MOC3 (OpenL2D, Free Development Public License),
la documentación del SDK, la documentación del formato `model3.json`, y **lo que el programa hace
al usarlo** (abrir un archivo, ver un resultado).

No válido: desensamblado traducido, nombres de clases internas como guía de diseño, estructuras
de datos copiadas del bytecode, cadenas de texto internas usadas como especificación.

### R4 — Nada del sistema de licencias
Ni una línea. Ni para entenderlo. Ver `docs/05`.

### R5 — Los artefactos de Live2D no se redistribuyen
Modelos de ejemplo, `.moc3`, texturas, la librería Core: se usan en local bajo su licencia y no
se suben al repo. `.gitignore` debe cubrirlos.

## Qué sí se puede afirmar con esto

- El formato `.moc3` es un contenedor documentado públicamente → implementarlo es interoperabilidad.
- La API del Cubism Core es pública (header C) → enlazarla es uso previsto del SDK.
- El comportamiento (un modelo se deforma así) es función, no expresión.

## Qué no te salva de nada

Un README que diga «inspirado en» no cambia el análisis: si el código es una traducción, es
derivado. La única defensa real es **no haber traducido**, y poder demostrarlo (historial de git
que empieza por la spec, no por el binario).

## Riesgo que asumo y no oculto

Live2D ha actuado en el pasado contra desarrolladores de lectores de MOC3
(`undeleted.ronsor.com/live2d-why-full-disclosure/`). Un lector de formato es defendible como
interoperabilidad, pero **no es un camino sin fricción**. Consecuencias prácticas:

1. Nada de publicar el lector como producto comercial sin hablar con Live2D.
2. Nada de usar sus modelos o texturas en ejemplos del repo.
3. Si el objetivo es distribuir algo, el camino con bendición es el Cubism SDK for Native
   (ver `docs/05`).
