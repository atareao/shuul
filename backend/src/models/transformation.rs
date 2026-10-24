//! # Transformaciones del valor de la petición
//!
//! Una regla puede declarar una *pipeline* de transformaciones —la misma idea
//! que Coraza— que se aplica al **valor de la petición** antes de compararlo
//! con el patrón de la regla. La especificación es una lista de tokens
//! canónicos separados por comas que se aplican de izquierda a derecha, una
//! sola vez cada uno.
//!
//! La cadena vacía es una pipeline vacía: [`TransformationPipeline::apply`]
//! devuelve [`Cow::Borrowed`] y el valor no se copia ni se toca, de modo que
//! una regla que no declare transformaciones se comporta byte a byte como
//! antes de existir esta funcionalidad.
//!
//! El marcador `none` limpia la pipeline construida hasta ese punto y un token
//! que no se reconozca es un error ([`TransformError`]) que lo **nombra**:
//! nunca se ignora en silencio.

use std::borrow::Cow;
use std::fmt;

/// Una transformación aplicable al valor de la petición.
///
/// Los nombres canónicos —los que se escriben en la especificación— los
/// devuelve [`Transformation::token`] y son sensibles a mayúsculas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transformation {
    /// Decodifica los escapes porcentuales de una URL (`%2e` → `.`).
    ///
    /// El `+` **no** se convierte en espacio: la misma transformación se aplica
    /// también al `path`, donde `+` es un carácter literal y no el espacio de
    /// `application/x-www-form-urlencoded`.
    ///
    /// Un `%` que no encabece dos dígitos hexadecimales se deja tal cual (`%zz`
    /// → `%zz`), y los bytes decodificados se resuelven como texto con
    /// [`String::from_utf8_lossy`]: un `%FF` suelto —que no forma una secuencia
    /// UTF-8 válida— produce un carácter de reemplazo, nunca un pánico.
    UrlDecode,
    /// Como [`Transformation::UrlDecode`] y además los escapes de IIS
    /// (`%u002e` → `.`).
    ///
    /// El escape de IIS son cuatro dígitos hexadecimales que se interpretan como
    /// un punto de código Unicode y se codifican en UTF-8; un punto de código
    /// fuera del rango escalar (`%uD800`) deja la secuencia tal cual.
    UrlDecodeUni,
    /// Pasa el valor a minúsculas.
    Lowercase,
    /// Pasa el valor a mayúsculas.
    Uppercase,
    /// Elimina los bytes NUL.
    RemoveNulls,
    /// Sustituye cada byte NUL por un espacio.
    ReplaceNulls,
    /// Convierte cada carácter de espacio en blanco en un espacio y colapsa las
    /// secuencias consecutivas en uno solo.
    ///
    /// **No** recorta los extremos —eso es [`Transformation::Trim`]— ni reduce
    /// los espacios ya únicos: `a  b` → `a b`, pero `  a ` → ` a `.
    ///
    /// «Espacio en blanco» es el de [`char::is_whitespace`], el mismo que
    /// reconoce `\s` en el regex del matcher, de modo que la transformación y el
    /// patrón hablan del mismo conjunto de caracteres.
    CompressWhitespace,
    /// Elimina todo el espacio en blanco, con el mismo criterio que
    /// [`Transformation::CompressWhitespace`].
    RemoveWhitespace,
    /// Resuelve los segmentos `..` y `.` de una ruta con separador `/`.
    ///
    /// Colapsa los separadores repetidos, descarta los segmentos `.` y resuelve
    /// cada `..` sacando el segmento anterior. Un `..` que no tenga segmento que
    /// sacar —ya en la raíz— se conserva, y el separador inicial se conserva si
    /// lo había. Los separadores finales se pierden, porque el resultado es la
    /// unión de los segmentos que sobreviven.
    NormalizePath,
    /// Lo mismo que [`Transformation::NormalizePath`] con separador `\`.
    ///
    /// Los `/` que aparezcan se tratan como caracteres literales: la
    /// transformación **no** convierte el separador de una convención a la otra.
    NormalizePathWin,
    /// Decodifica las entidades HTML (`&lt;` → `<`).
    ///
    /// Reconoce las entidades con nombre `&lt;`, `&gt;`, `&amp;`, `&quot;`,
    /// `&apos;`, `&nbsp;`, `&colon;`, `&sol;`, `&bsol;`, `&Tab;` y `&NewLine;`,
    /// y las numéricas decimales (`&#39;`) y hexadecimales (`&#x27;`). La
    /// comparación es exacta y sensible a mayúsculas, y una entidad que no se
    /// reconozca se deja tal cual.
    ///
    /// El `;` final es **obligatorio** —una divergencia deliberada del estándar,
    /// que admite la forma sin punto y coma cuando el siguiente carácter no es un
    /// dígito—: `&#39;` se decodifica y `&#39` se deja tal cual.
    HtmlEntityDecode,
    /// Decodifica los escapes de JavaScript (`\u003c` → `<`).
    ///
    /// Reconoce `\uXXXX`, `\xHH` y los escapes de un solo carácter `\0`, `\n`,
    /// `\r`, `\t`, `\v`, `\f`, `\b`, `\\`, `\"`, `\'` y `\/`. Lo que no se
    /// reconozca se deja tal cual.
    JsDecode,
    /// Decodifica los escapes de CSS (`\3c` → `<`).
    ///
    /// El escape es una barra invertida seguida de uno a seis dígitos
    /// hexadecimales; si son menos de seis, consume además **un** espacio en
    /// blanco que actúa de terminador (`\3c ` → `<`). El punto de código se
    /// codifica en UTF-8 y, como manda la especificación de CSS, un punto de
    /// código nulo o fuera del rango escalar produce U+FFFD.
    CssDecode,
    /// Decodifica las secuencias de escape `\xNN` (`\x3c` → `<`).
    ///
    /// Reconoce `\xHH`, el octal de hasta tres dígitos (`\0`, `\012`) y los
    /// escapes de C `\n`, `\r`, `\t`, `\v`, `\f`, `\a`, `\b`, `\\`, `\"`, `\'` y
    /// `\?`. Lo que no se reconozca se deja tal cual.
    EscapeSeqDecode,
    /// Decodifica el valor como Base64. Un valor que no sea Base64 válido se
    /// deja tal cual.
    ///
    /// El alfabeto es el estándar de RFC 4648 —`A-Z a-z 0-9 + /` y `=` de
    /// relleno— y la comprobación es estricta: un carácter fuera del alfabeto,
    /// una longitud que no sea múltiplo de cuatro, un relleno mal puesto o unos
    /// bytes que no sean UTF-8 válido dejan el valor **intacto**. Es una
    /// transformación de detección: devolver algo raro sería peor que no tocar
    /// nada.
    Base64Decode,
    /// Normaliza una línea de comandos (elimina `\`, comillas y `^`).
    ///
    /// Elimina las barras invertidas, las comillas simples y dobles y los
    /// acentos circunflejos, **y además** colapsa las secuencias de espacio en
    /// blanco en un único espacio, sin recortar los extremos.
    CmdLine,
    /// Recorta el espacio en blanco de los extremos.
    Trim,
    /// Sustituye el valor por su longitud en bytes, en decimal.
    Length,
}

impl Transformation {
    /// Las 18 transformaciones, en el orden canónico de la especificación.
    pub const ALL: [Self; 18] = [
        Self::UrlDecode,
        Self::UrlDecodeUni,
        Self::Lowercase,
        Self::Uppercase,
        Self::RemoveNulls,
        Self::ReplaceNulls,
        Self::CompressWhitespace,
        Self::RemoveWhitespace,
        Self::NormalizePath,
        Self::NormalizePathWin,
        Self::HtmlEntityDecode,
        Self::JsDecode,
        Self::CssDecode,
        Self::EscapeSeqDecode,
        Self::Base64Decode,
        Self::CmdLine,
        Self::Trim,
        Self::Length,
    ];

    /// El nombre canónico con el que se escribe en la especificación.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::UrlDecode => "urlDecode",
            Self::UrlDecodeUni => "urlDecodeUni",
            Self::Lowercase => "lowercase",
            Self::Uppercase => "uppercase",
            Self::RemoveNulls => "removeNulls",
            Self::ReplaceNulls => "replaceNulls",
            Self::CompressWhitespace => "compressWhitespace",
            Self::RemoveWhitespace => "removeWhitespace",
            Self::NormalizePath => "normalizePath",
            Self::NormalizePathWin => "normalizePathWin",
            Self::HtmlEntityDecode => "htmlEntityDecode",
            Self::JsDecode => "jsDecode",
            Self::CssDecode => "cssDecode",
            Self::EscapeSeqDecode => "escapeSeqDecode",
            Self::Base64Decode => "base64Decode",
            Self::CmdLine => "cmdLine",
            Self::Trim => "trim",
            Self::Length => "length",
        }
    }

    /// El token canónico → variante. Exacto y sensible a mayúsculas.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.token() == token)
    }

    /// Aplica la transformación a un valor.
    ///
    /// Una secuencia que la transformación no reconozca deja el valor **tal
    /// cual** en la parte que no reconoce: ninguna transformación corrompe el
    /// valor, entra en pánico ni itera sin avanzar.
    ///
    /// Devuelve un [`String`] y no un [`Cow`] porque una transformación puede
    /// cambiar el valor por definición; el [`Cow`] que evita la copia cuando no
    /// hay ninguna transformación vive en
    /// [`TransformationPipeline::apply`].
    #[must_use]
    pub fn apply(self, value: &str) -> String {
        match self {
            Self::UrlDecode => percent_decode(value, false),
            Self::UrlDecodeUni => percent_decode(value, true),
            Self::Lowercase => value.to_lowercase(),
            Self::Uppercase => value.to_uppercase(),
            Self::RemoveNulls => value.replace('\0', ""),
            Self::ReplaceNulls => value.replace('\0', " "),
            Self::CompressWhitespace => compress_whitespace(value),
            Self::RemoveWhitespace => value.replace(char::is_whitespace, ""),
            Self::NormalizePath => normalize_path(value, '/'),
            Self::NormalizePathWin => normalize_path(value, '\\'),
            Self::HtmlEntityDecode => html_entity_decode(value),
            Self::JsDecode => js_decode(value),
            Self::CssDecode => css_decode(value),
            Self::EscapeSeqDecode => escape_seq_decode(value),
            Self::Base64Decode => base64_decode(value).unwrap_or_else(|| value.to_owned()),
            Self::CmdLine => cmd_line(value),
            Self::Trim => value.trim().to_owned(),
            Self::Length => value.len().to_string(),
        }
    }
}

/// El valor de un dígito hexadecimal, o `None` si el byte no lo es.
const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Los dos dígitos hexadecimales que empiezan en `start`, como byte.
const fn hex_pair(bytes: &[u8], start: usize) -> Option<u8> {
    if bytes.len() < start + 2 {
        return None;
    }
    let (Some(high), Some(low)) = (hex_value(bytes[start]), hex_value(bytes[start + 1])) else {
        return None;
    };
    Some((high << 4) | low)
}

/// Los cuatro dígitos hexadecimales que empiezan en `start`, como punto de
/// código.
const fn hex_quadruple(bytes: &[u8], start: usize) -> Option<u32> {
    if bytes.len() < start + 4 {
        return None;
    }
    let mut value = 0_u32;
    let mut position = 0;
    while position < 4 {
        let Some(digit) = hex_value(bytes[start + position]) else {
            return None;
        };
        value = value * 16 + digit as u32;
        position += 1;
    }
    Some(value)
}

/// Decodifica los escapes porcentuales (`%XX`) de `value`.
///
/// Con `iis_escapes` reconoce además `%uXXXX`. El `+` **no** se convierte en
/// espacio, porque la misma transformación se aplica al `path`, donde `+` es un
/// carácter literal. Un `%` que no encabece una secuencia válida se copia tal
/// cual, y los bytes resultantes se resuelven con [`String::from_utf8_lossy`].
fn percent_decode(value: &str, iis_escapes: bool) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'%' {
            out.push(bytes[index]);
            index += 1;
            continue;
        }

        if iis_escapes
            && bytes.get(index + 1) == Some(&b'u')
            && let Some(code_point) = hex_quadruple(bytes, index + 2)
        {
            match char::from_u32(code_point) {
                Some(character) => {
                    let mut buffer = [0_u8; 4];
                    out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                },
                // Fuera del rango escalar (`%uD800`): la secuencia entera se
                // deja tal cual, en lugar de corromper el valor.
                None => out.extend_from_slice(&bytes[index..index + 6]),
            }
            index += 6;
            continue;
        }

        let high = bytes.get(index + 1).copied().and_then(hex_value);
        let low = bytes.get(index + 2).copied().and_then(hex_value);

        if let (Some(high), Some(low)) = (high, low) {
            out.push((high << 4) | low);
            index += 3;
            continue;
        }

        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Convierte cada espacio en blanco en un espacio y colapsa las secuencias
/// consecutivas en uno solo, sin recortar los extremos.
fn compress_whitespace(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_whitespace = false;

    for character in value.chars() {
        if character.is_whitespace() {
            if !in_whitespace {
                out.push(' ');
                in_whitespace = true;
            }
        } else {
            out.push(character);
            in_whitespace = false;
        }
    }

    out
}

/// Resuelve los segmentos `..` y `.` de una ruta con separador `separator`.
///
/// Colapsa los separadores repetidos, descarta los segmentos `.`, resuelve cada
/// `..` sacando el segmento anterior —y lo conserva si no hay ninguno que
/// sacar— y conserva el separador inicial. Los separadores finales se pierden:
/// el resultado es la unión de los segmentos que sobreviven.
fn normalize_path(value: &str, separator: char) -> String {
    let mut segments: Vec<&str> = Vec::new();

    for segment in value.split(separator) {
        match segment {
            "" | "." => {},
            ".." => {
                if matches!(segments.last(), Some(&last) if last != "..") {
                    segments.pop();
                } else {
                    segments.push("..");
                }
            },
            other => segments.push(other),
        }
    }

    let mut out = String::with_capacity(value.len());
    if value.starts_with(separator) {
        out.push(separator);
    }
    for (position, segment) in segments.iter().enumerate() {
        if position > 0 {
            out.push(separator);
        }
        out.push_str(segment);
    }

    out
}

/// Las entidades HTML con nombre, con su texto y el orden de comparación.
const HTML_ENTITIES: [(&str, &str); 11] = [
    ("lt", "<"),
    ("gt", ">"),
    ("amp", "&"),
    ("quot", "\""),
    ("apos", "'"),
    ("nbsp", "\u{a0}"),
    ("colon", ":"),
    ("sol", "/"),
    ("bsol", "\\"),
    ("Tab", "\t"),
    ("NewLine", "\n"),
];

/// Decodifica la entidad HTML que encabeza `entity`, que empieza por `&`.
///
/// Devuelve el texto decodificado y cuántos bytes consumió, o `None` si no
/// reconoce la entidad —también si la numérica nombra un punto de código fuera
/// del rango escalar, que se deja tal cual—.
fn decode_html_entity(entity: &str) -> Option<(String, usize)> {
    let body = entity.strip_prefix('&')?;

    if let Some(numeric) = body.strip_prefix('#') {
        let (digits, radix, base_marker) = numeric
            .strip_prefix(['x', 'X'])
            .map_or((numeric, 10_u32, 0), |hexadecimal| (hexadecimal, 16_u32, 1));
        let length = digits
            .find(|character: char| character.to_digit(radix).is_none())
            .unwrap_or(digits.len());

        if length == 0 || !digits[length..].starts_with(';') {
            return None;
        }

        let code_point = u32::from_str_radix(&digits[..length], radix).ok()?;
        let character = char::from_u32(code_point)?;
        // `&` + `#` + la marca de base + los dígitos + `;`.
        return Some((character.to_string(), 3 + base_marker + length));
    }

    for (name, replacement) in HTML_ENTITIES {
        if let Some(rest) = body.strip_prefix(name)
            && rest.starts_with(';')
        {
            // `&` + el nombre + `;`.
            return Some((replacement.to_owned(), name.len() + 2));
        }
    }

    None
}

/// Decodifica las entidades HTML de `value`; una que no se reconozca se deja
/// tal cual. La sustitución es de una sola pasada: `&amp;lt;` → `&lt;`.
fn html_entity_decode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;

    while let Some(position) = rest.find('&') {
        out.push_str(&rest[..position]);
        let entity = &rest[position..];

        if let Some((decoded, consumed)) = decode_html_entity(entity) {
            out.push_str(&decoded);
            rest = &entity[consumed..];
        } else {
            out.push('&');
            rest = &entity[1..];
        }
    }

    out.push_str(rest);
    out
}

/// El byte al que decodifica un escape de un solo carácter de JavaScript, o
/// `None` si no es uno de los reconocidos.
const fn js_single_escape(escape: u8) -> Option<u8> {
    match escape {
        b'0' => Some(0),
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b't' => Some(b'\t'),
        b'v' => Some(0x0b),
        b'f' => Some(0x0c),
        b'b' => Some(0x08),
        b'\\' => Some(b'\\'),
        b'"' => Some(b'"'),
        b'\'' => Some(b'\''),
        b'/' => Some(b'/'),
        _ => None,
    }
}

/// Decodifica los escapes de JavaScript de `value`: `\uXXXX`, `\xHH` y los de
/// un solo carácter. Lo que no reconozca se deja tal cual.
fn js_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'\\' || index + 1 >= bytes.len() {
            out.push(bytes[index]);
            index += 1;
            continue;
        }

        match bytes[index + 1] {
            b'u' => {
                if let Some(code_point) = hex_quadruple(bytes, index + 2)
                    && let Some(character) = char::from_u32(code_point)
                {
                    let mut buffer = [0_u8; 4];
                    out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                    index += 6;
                    continue;
                }
            },
            b'x' => {
                if let Some(byte) = hex_pair(bytes, index + 2) {
                    out.push(byte);
                    index += 4;
                    continue;
                }
            },
            escape => {
                if let Some(byte) = js_single_escape(escape) {
                    out.push(byte);
                    index += 2;
                    continue;
                }
            },
        }

        // Secuencia no reconocida: se copia la barra invertida y se sigue.
        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Decodifica los escapes de CSS de `value`: `\` seguido de uno a seis dígitos
/// hexadecimales, con un espacio en blanco de terminador cuando son menos de
/// seis. Un `\` que no encabece un hexadecimal se deja tal cual.
fn css_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'\\' {
            out.push(bytes[index]);
            index += 1;
            continue;
        }

        let mut nibble_count = 0;
        let mut code_point = 0_u32;
        while nibble_count < 6 {
            let Some(nibble) = bytes
                .get(index + 1 + nibble_count)
                .copied()
                .and_then(hex_value)
            else {
                break;
            };
            code_point = code_point * 16 + u32::from(nibble);
            nibble_count += 1;
        }

        if nibble_count == 0 {
            out.push(bytes[index]);
            index += 1;
            continue;
        }

        index += 1 + nibble_count;

        // Con menos de seis dígitos, un espacio en blanco actúa de terminador y
        // se consume.
        if nibble_count < 6
            && let Some(terminator) = bytes.get(index)
            && terminator.is_ascii_whitespace()
        {
            index += 1;
        }

        // La especificación de CSS resuelve el punto de código nulo y el que
        // queda fuera del rango escalar al carácter de reemplazo.
        let character = match char::from_u32(code_point) {
            Some('\0') | None => char::REPLACEMENT_CHARACTER,
            Some(character) => character,
        };
        let mut buffer = [0_u8; 4];
        out.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// El byte al que decodifica un escape de un solo carácter de C, o `None` si no
/// es uno de los reconocidos.
const fn c_single_escape(escape: u8) -> Option<u8> {
    match escape {
        b'n' => Some(b'\n'),
        b'r' => Some(b'\r'),
        b't' => Some(b'\t'),
        b'v' => Some(0x0b),
        b'f' => Some(0x0c),
        b'a' => Some(0x07),
        b'b' => Some(0x08),
        b'\\' => Some(b'\\'),
        b'"' => Some(b'"'),
        b'\'' => Some(b'\''),
        b'?' => Some(b'?'),
        _ => None,
    }
}

/// El valor de un dígito octal, o `None` si el byte no lo es.
const fn octal_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'7' => Some(byte - b'0'),
        _ => None,
    }
}

/// Decodifica las secuencias de escape de C de `value`: `\xHH`, el octal de
/// hasta tres dígitos y los escapes de un solo carácter. Lo que no reconozca se
/// deja tal cual.
///
/// El octal puede llegar a `\777` (511): se conserva el byte bajo, porque el
/// resultado de la transformación es una secuencia de bytes.
fn escape_seq_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'\\' || index + 1 >= bytes.len() {
            out.push(bytes[index]);
            index += 1;
            continue;
        }

        match bytes[index + 1] {
            b'x' => {
                if let Some(byte) = hex_pair(bytes, index + 2) {
                    out.push(byte);
                    index += 4;
                    continue;
                }
            },
            digit @ b'0'..=b'7' => {
                let mut byte = digit - b'0';
                let mut digits = 1;
                while digits < 3 {
                    let Some(next) = bytes.get(index + 1 + digits).copied().and_then(octal_value)
                    else {
                        break;
                    };
                    byte = byte.wrapping_mul(8).wrapping_add(next);
                    digits += 1;
                }
                out.push(byte);
                index += 1 + digits;
                continue;
            },
            escape => {
                if let Some(byte) = c_single_escape(escape) {
                    out.push(byte);
                    index += 2;
                    continue;
                }
            },
        }

        // Secuencia no reconocida: se copia la barra invertida y se sigue.
        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// El valor de un carácter del alfabeto Base64 estándar, o `None` si no
/// pertenece a él.
const fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodifica `value` como Base64 estándar, o `None` si no lo es.
///
/// La comprobación es estricta: exige el alfabeto de RFC 4648, una longitud
/// múltiplo de cuatro, el relleno al final y como mucho dos veces, la forma
/// canónica de los bits sobrantes y que los bytes decodificados sean UTF-8
/// válido. Ante cualquier duda devuelve `None`, que deja el valor intacto.
fn base64_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();

    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return None;
    }

    let padding = bytes
        .iter()
        .rev()
        .take_while(|octet| **octet == b'=')
        .count();
    if padding > 2 {
        return None;
    }

    let data = &bytes[..bytes.len() - padding];
    if data.contains(&b'=') {
        return None;
    }

    let mut out = Vec::with_capacity(data.len() / 4 * 3 + 3);
    let mut accumulator = 0_u16;
    let mut bits = 0_u8;

    for octet in data {
        accumulator = (accumulator << 6) | u16::from(base64_value(*octet)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(accumulator >> bits).ok()?);
            accumulator &= (1 << bits) - 1;
        }
    }

    // Los bits sobrantes del último cuanteto tienen que ser cero: es la forma
    // canónica de RFC 4648, y un relleno que los deje puestos está mal puesto.
    if accumulator != 0 {
        return None;
    }

    String::from_utf8(out).ok()
}

/// Normaliza una línea de comandos: elimina las barras invertidas, las comillas
/// simples y dobles y los acentos circunflejos, y colapsa las secuencias de
/// espacio en blanco en un único espacio, sin recortar los extremos.
fn cmd_line(value: &str) -> String {
    let mut stripped = String::with_capacity(value.len());

    for character in value.chars() {
        if !matches!(character, '\\' | '\'' | '"' | '^') {
            stripped.push(character);
        }
    }

    compress_whitespace(&stripped)
}

/// El error de una especificación de transformaciones irresoluble.
///
/// Nombra el token que no se reconoció; [`fmt::Display`] lista además los
/// nombres válidos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformError {
    token: String,
}

impl TransformError {
    /// Construye el error a partir del token que no se reconoció.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    /// El token que no se reconoció.
    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown transformation token '{}'; valid tokens are: ",
            self.token
        )?;
        for (index, transformation) in Transformation::ALL.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            f.write_str(transformation.token())?;
        }
        Ok(())
    }
}

impl std::error::Error for TransformError {}

/// Una pipeline de transformaciones ya validada.
///
/// Se construye con [`TransformationPipeline::parse`] y se aplica con
/// [`TransformationPipeline::apply`]. Guarda la especificación canónica que se
/// almacenó en la regla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformationPipeline {
    steps: Vec<Transformation>,
    spec: String,
}

impl TransformationPipeline {
    /// Parsea la especificación `t:` de una regla.
    ///
    /// La cadena vacía es la pipeline vacía y el marcador `none` limpia la
    /// pipeline construida hasta ese punto. `spec` guarda la especificación
    /// **canónica resultante**: los tokens efectivos unidos por comas, ya sin
    /// los que limpió un `none`.
    ///
    /// El parser es **estricto**: no recorta los espacios alrededor de los
    /// tokens ni admite otra capitalización. Es el mismo criterio que el de
    /// [`crate::models::validate_pipeline`], que compara el pipeline de la regla
    /// de forma exacta y sensible a mayúsculas: aceptar aquí una variante que el
    /// resto del sistema no reconoce sería admitir un valor que ninguna
    /// comparación entiende.
    ///
    /// # Errors
    ///
    /// Devuelve [`TransformError`] si la especificación contiene un token que
    /// no se reconoce. El error **nombra el token** y lista los válidos.
    pub fn parse(spec: &str) -> Result<Self, TransformError> {
        if spec.is_empty() {
            return Ok(Self::default());
        }

        let mut steps = Vec::new();

        for token in spec.split(',') {
            if token == "none" {
                steps.clear();
                continue;
            }

            match Transformation::from_token(token) {
                Some(transformation) => steps.push(transformation),
                None => return Err(TransformError::new(token)),
            }
        }

        let mut canonical = String::with_capacity(spec.len());
        for (position, step) in steps.iter().enumerate() {
            if position > 0 {
                canonical.push(',');
            }
            canonical.push_str(step.token());
        }

        Ok(Self {
            steps,
            spec: canonical,
        })
    }

    /// Aplica la pipeline al valor, de izquierda a derecha y una sola vez cada
    /// transformación.
    ///
    /// Con la pipeline vacía devuelve [`Cow::Borrowed`]: el valor no se copia ni
    /// se toca. Con la pipeline no vacía devuelve [`Cow::Owned`] aunque ninguna
    /// transformación cambie el valor: encadenar pasos sobre el valor exige
    /// construir el resultado, y prometer `Borrowed` obligaría a comparar el
    /// resultado con el original en cada petición.
    #[must_use]
    pub fn apply<'a>(&self, value: &'a str) -> Cow<'a, str> {
        if self.steps.is_empty() {
            return Cow::Borrowed(value);
        }

        Cow::Owned(
            self.steps
                .iter()
                .fold(value.to_owned(), |accumulated, step| {
                    step.apply(&accumulated)
                }),
        )
    }

    /// Indica si la pipeline no tiene ninguna transformación.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// La especificación canónica almacenada.
    #[must_use]
    pub fn spec(&self) -> &str {
        &self.spec
    }
}

impl Default for TransformationPipeline {
    /// La pipeline vacía.
    ///
    /// Es el valor por defecto de una regla que no declara transformaciones: el
    /// valor no se copia ni se toca.
    fn default() -> Self {
        Self {
            steps: Vec::new(),
            spec: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    // ── Contrato de la tabla de tokens ──

    /// Escenario: la lista canónica tiene las 18 transformaciones y su orden
    /// es el de la especificación.
    #[test]
    fn test_all_lists_eighteen_transformations_in_canonical_order() {
        assert_eq!(Transformation::ALL.len(), 18);
        assert_eq!(Transformation::ALL[0].token(), "urlDecode");
        assert_eq!(Transformation::ALL[17].token(), "length");
    }

    /// Escenario: `token` y `from_token` son inversas exactas para las 18.
    #[test]
    fn test_token_and_from_token_round_trip() {
        for transformation in Transformation::ALL {
            assert_eq!(
                Transformation::from_token(transformation.token()),
                Some(transformation),
                "el token `{}` debe volver a su variante",
                transformation.token()
            );
        }
    }

    /// Escenario: los nombres son sensibles a mayúsculas y un token vacío no
    /// resuelve.
    #[test]
    fn test_from_token_is_case_sensitive() {
        assert_eq!(
            Transformation::from_token("urlDecodeUni"),
            Some(Transformation::UrlDecodeUni)
        );
        assert_eq!(Transformation::from_token("urldecodeuni"), None);
        assert_eq!(Transformation::from_token(""), None);
    }

    // ── 1.1: un test por transformación, con un caso de evasión real ──

    /// Escenario: `urlDecode` decodifica el traversal codificado en porcentaje.
    #[test]
    fn test_url_decode_percent_encoded_traversal() {
        assert_eq!(Transformation::UrlDecode.apply("%2e%2e%2f"), "../");
    }

    /// Escenario: `urlDecodeUni` decodifica el traversal codificado en
    /// porcentaje.
    #[test]
    fn test_url_decode_uni_percent_encoded_traversal() {
        assert_eq!(Transformation::UrlDecodeUni.apply("%2e%2e%2f"), "../");
    }

    /// Escenario: `urlDecodeUni` decodifica además el escape de IIS
    /// (`%u002e`), que `urlDecode` no reconoce.
    #[test]
    fn test_url_decode_uni_iis_unicode_escape() {
        assert_eq!(Transformation::UrlDecodeUni.apply("%u002e%u002e/"), "../");
    }

    /// Escenario: `lowercase` normaliza la caja de la palabra clave SQL.
    #[test]
    fn test_lowercase_select_keyword() {
        assert_eq!(Transformation::Lowercase.apply("SELECT"), "select");
    }

    /// Escenario: `uppercase` normaliza la caja de la palabra clave SQL.
    #[test]
    fn test_uppercase_select_keyword() {
        assert_eq!(Transformation::Uppercase.apply("select"), "SELECT");
    }

    /// Escenario: `removeNulls` elimina el NUL que parte la etiqueta para
    /// esquivar el patrón `<script`.
    #[test]
    fn test_remove_nulls_script_tag() {
        assert_eq!(
            Transformation::RemoveNulls.apply("<scr\u{0}ipt>"),
            "<script>"
        );
    }

    /// Escenario: `replaceNulls` sustituye el NUL por un espacio.
    #[test]
    fn test_replace_nulls_with_space() {
        assert_eq!(Transformation::ReplaceNulls.apply("a\u{0}b"), "a b");
    }

    /// Escenario: `compressWhitespace` colapsa los huecos que se usan para
    /// esquivar `union\s+select`.
    #[test]
    fn test_compress_whitespace_sql_injection() {
        assert_eq!(
            Transformation::CompressWhitespace.apply("1   union   select"),
            "1 union select"
        );
    }

    /// Escenario: `removeWhitespace` elimina todo el espacio en blanco.
    #[test]
    fn test_remove_whitespace_sql_injection() {
        assert_eq!(
            Transformation::RemoveWhitespace.apply("1 union select"),
            "1unionselect"
        );
    }

    /// Escenario: `normalizePath` resuelve el segmento `..` de una ruta.
    #[test]
    fn test_normalize_path_dotdot_segments() {
        assert_eq!(Transformation::NormalizePath.apply("/a/b/../c"), "/a/c");
    }

    /// Escenario: `normalizePathWin` resuelve el segmento `..` de una ruta con
    /// separador de Windows.
    #[test]
    fn test_normalize_path_win_dotdot_segments() {
        assert_eq!(
            Transformation::NormalizePathWin.apply(r"\a\b\..\c"),
            r"\a\c"
        );
    }

    /// Escenario: `htmlEntityDecode` decodifica la etiqueta escapada con
    /// entidades con nombre.
    #[test]
    fn test_html_entity_decode_script_tag() {
        assert_eq!(
            Transformation::HtmlEntityDecode.apply("&lt;script&gt;"),
            "<script>"
        );
    }

    /// Escenario: `htmlEntityDecode` decodifica la entidad numérica
    /// hexadecimal que esconde la comilla.
    #[test]
    fn test_html_entity_decode_hex_apostrophe() {
        assert_eq!(Transformation::HtmlEntityDecode.apply("&#x27;"), "'");
    }

    /// Escenario: `jsDecode` decodifica los escapes `\uNNNN` de JavaScript.
    #[test]
    fn test_js_decode_unicode_script_tag() {
        assert_eq!(
            Transformation::JsDecode.apply(r"\u003cscript\u003e"),
            "<script>"
        );
    }

    /// Escenario: `cssDecode` decodifica el escape `\3c` de CSS.
    #[test]
    fn test_css_decode_escaped_angle_bracket() {
        assert_eq!(Transformation::CssDecode.apply(r"\3c"), "<");
    }

    /// Escenario: `escapeSeqDecode` decodifica las secuencias `\xNN`.
    #[test]
    fn test_escape_seq_decode_hex_script_tag() {
        assert_eq!(
            Transformation::EscapeSeqDecode.apply(r"\x3cscript\x3e"),
            "<script>"
        );
    }

    /// Escenario: `base64Decode` decodifica el payload codificado.
    #[test]
    fn test_base64_decode_etc_passwd() {
        assert_eq!(
            Transformation::Base64Decode.apply("L2V0Yy9wYXNzd2Q="),
            "/etc/passwd"
        );
    }

    /// Escenario: `cmdLine` elimina las barras invertidas que separan las
    /// letras del comando.
    #[test]
    fn test_cmd_line_escaped_cat() {
        assert_eq!(Transformation::CmdLine.apply(r"\c\a\t"), "cat");
    }

    /// Escenario: `trim` recorta el espacio en blanco de los extremos.
    #[test]
    fn test_trim_surrounding_whitespace() {
        assert_eq!(
            Transformation::Trim.apply("  1 union select  "),
            "1 union select"
        );
    }

    /// Escenario: `length` sustituye el valor por su longitud en bytes.
    ///
    /// `%2e%2e%2f` tiene **9** bytes: `%2e`, `%2e` y `%2f`. La tabla de la
    /// tarea anotaba `7` para esa entrada; `7` es la longitud de `%2e%2e/`, que
    /// se comprueba también aquí para dejar constancia del valor que sí
    /// correspondía a esa cifra.
    #[test]
    fn test_length_percent_encoded_traversal() {
        assert_eq!(Transformation::Length.apply("%2e%2e%2f"), "9");
        assert_eq!(Transformation::Length.apply("%2e%2e/"), "7");
    }

    /// Escenario: la cadena `urlDecodeUni,compressWhitespace` deja el payload
    /// SQL en una forma que casa el patrón `union\s+select`.
    #[test]
    fn test_chained_url_decode_uni_compress_whitespace_matches_sql_regex() {
        let pipeline = TransformationPipeline::parse("urlDecodeUni,compressWhitespace").unwrap();
        let transformed = pipeline.apply("1%20union%20select%201");
        let pattern = Regex::new(r"union\s+select").unwrap();

        assert!(
            pattern.is_match(&transformed),
            "tras la pipeline, `{transformed}` debe casar `union\\s+select`"
        );
    }

    // ── 1.2: la especificación vacía es la pipeline vacía ──

    /// Escenario: `parse("")` devuelve una pipeline vacía y su `apply` sobre el
    /// valor devuelve `Cow::Borrowed` —no `Owned`— con el valor idéntico.
    #[test]
    fn test_empty_spec_is_empty_pipeline_and_borrows_value() {
        let pipeline = TransformationPipeline::parse("").unwrap();

        assert!(pipeline.is_empty(), "la cadena vacía es la pipeline vacía");

        let value = "%2e%2e%2f";
        let transformed = pipeline.apply(value);

        assert!(
            matches!(transformed, Cow::Borrowed(_)),
            "la pipeline vacía no debe copiar el valor"
        );
        assert_eq!(transformed, value);
    }

    // ── 1.3: el orden de la pipeline importa ──

    /// Escenario: `urlDecodeUni,lowercase` y `lowercase,urlDecodeUni` dan
    /// resultados distintos.
    ///
    /// El valor `%41BC` distingue los dos órdenes: `%41` decodifica a `A`
    /// —mayúscula—, así que pasar a minúsculas **antes** de decodificar deja
    /// ese `A` recién nacido en mayúsculas (`Abc`), mientras que pasar a
    /// minúsculas **después** de decodificar lo alcanza (`abc`).
    #[test]
    fn test_pipeline_order_matters() {
        let decoded_then_lower = TransformationPipeline::parse("urlDecodeUni,lowercase").unwrap();
        let lower_then_decoded = TransformationPipeline::parse("lowercase,urlDecodeUni").unwrap();

        assert_eq!(decoded_then_lower.apply("%41BC"), "abc");
        assert_eq!(lower_then_decoded.apply("%41BC"), "Abc");

        // La especificación canónica se conserva para poder devolverla por la
        // API; con una especificación válida sin `none` coincide con la escrita.
        assert_eq!(decoded_then_lower.spec(), "urlDecodeUni,lowercase");
    }

    // ── 1.4: el marcador `none` limpia lo construido ──

    /// Escenario: `none` y `urlDecodeUni,none` dejan la pipeline vacía, y
    /// `urlDecodeUni,none,lowercase` deja sólo `lowercase`.
    #[test]
    fn test_none_marker_clears_pipeline() {
        assert!(TransformationPipeline::parse("none").unwrap().is_empty());
        assert!(
            TransformationPipeline::parse("urlDecodeUni,none")
                .unwrap()
                .is_empty()
        );

        let pipeline = TransformationPipeline::parse("urlDecodeUni,none,lowercase").unwrap();

        assert!(!pipeline.is_empty(), "`lowercase` sobrevive al `none`");
        assert_eq!(pipeline.apply("ABC"), "abc");
    }

    // ── 1.5: un token desconocido es un error que lo nombra ──

    /// Escenario: `parse("urlDecodeUni,noExiste")` devuelve `Err` y el
    /// `Display` del error nombra el token y lista los válidos.
    #[test]
    fn test_unknown_token_is_error_naming_token() {
        let error = TransformationPipeline::parse("urlDecodeUni,noExiste").unwrap_err();

        assert_eq!(error.token(), "noExiste");
        assert!(
            error.to_string().contains("noExiste"),
            "el error debe nombrar el token: {error}"
        );
        assert!(
            error.to_string().contains("urlDecodeUni"),
            "el error debe listar los tokens válidos: {error}"
        );
    }

    // ── 1.6: una secuencia inválida no corrompe el valor ──

    /// Escenario: una secuencia de escape inválida deja el valor tal cual.
    ///
    /// El primer aserto es el control que hace el test no vacuo: la misma
    /// transformación **sí** decodifica una secuencia válida.
    #[test]
    fn test_invalid_url_decode_sequence_keeps_value() {
        assert_eq!(Transformation::UrlDecode.apply("%2e"), ".");
        assert_eq!(Transformation::UrlDecode.apply("%zz"), "%zz");
    }

    /// Escenario: un valor que no es Base64 válido deja el valor tal cual.
    ///
    /// El primer aserto es el control que hace el test no vacuo.
    #[test]
    fn test_invalid_base64_sequence_keeps_value() {
        assert_eq!(Transformation::Base64Decode.apply("L2V0Yw=="), "/etc");
        assert_eq!(
            Transformation::Base64Decode.apply("no-es-base64"),
            "no-es-base64"
        );
    }
}
