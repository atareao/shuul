# client-address-consistency

Todo consumidor de la dirección del cliente usa la dirección resuelta: el GeoIP y el filtro de Tor dejan de recibir la cabecera `X-Forwarded-For` cruda y pasan a usar `ip_address`, cerrando el fail-open que la cadena de saltos provocaba en ambos.
