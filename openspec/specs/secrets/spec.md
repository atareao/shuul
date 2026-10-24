# secrets Specification

## Purpose
Shuul firma sus tokens con un secreto que vive en la variable de entorno `SECRET`. Este capability fija lo que la aplicación debe hacer con él: no escribirlo nunca en el log —ni siquiera al arrancar— y denunciar en voz alta cuando es demasiado débil para lo que protege, sin negarse por ello a arrancar.

## Requirements

### Requirement: El secreto de firma nunca se escribe en el log

El secreto con el que la aplicación firma y valida los JWT SHALL NOT aparecer en el log a ningún
nivel de `RUST_LOG`: ni su valor, ni truncado, ni su longitud, ni derivado alguno. La configuración
por defecto de la imagen SHALL NOT activar el registro de depuración, de modo que un `debug!`
inadvertido no acabe publicando una credencial en un despliegue estándar.

#### Scenario: El arranque no publica el secreto
- **GIVEN** una `SECRET` configurada y `RUST_LOG=backend=debug`
- **WHEN** la aplicación arranca
- **THEN** ninguna línea del log contiene el valor del secreto
- **AND** ninguna invocación de log del backend interpola la variable del secreto

#### Scenario: La configuración por defecto no registra en depuración
- **GIVEN** el `compose.yml` del repositorio
- **WHEN** se inspecciona su nivel de log por defecto
- **THEN** el objetivo `backend` vale `info`
- **AND** no contiene `debug`

### Requirement: Un secreto débil se denuncia al arrancar

Si la `SECRET` está presente pero mide menos de 32 caracteres —el mínimo que exigen el README y la
guía de autohospedaje—, la aplicación SHALL emitir un aviso que nombre el requisito y SHALL arrancar
igualmente. El servicio es el ForwardAuth de todos los servicios protegidos: negarse a arrancar
convertiría una variable mal puesta en una caída total.

#### Scenario: Un secreto largo no dispara el aviso
- **GIVEN** una `SECRET` de 40 caracteres
- **WHEN** la aplicación arranca
- **THEN** no emite ningún aviso sobre el secreto

#### Scenario: Un secreto corto se denuncia sin tumbar el servicio
- **GIVEN** una `SECRET` de 10 caracteres
- **WHEN** la aplicación arranca
- **THEN** emite un aviso que nombra el mínimo de 32 caracteres
- **AND** el servicio arranca y atiende peticiones
