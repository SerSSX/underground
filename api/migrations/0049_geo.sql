-- TASK-303 G1: global geo. Idempotent (IF NOT EXISTS / WHERE ... IS NULL), like 0048.
--
-- 1. geo_cache: answers from the OpenStreetMap geocoder (Photon) proxied by /api/geo/*, kept
--    7 days so a repeated search never reaches the upstream again. `value` is always the
--    normalised JSON array the endpoint returned, never the upstream body.
CREATE TABLE IF NOT EXISTS geo_cache (
    key        TEXT PRIMARY KEY,
    value      JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS geo_cache_created_idx ON geo_cache (created_at);

-- 2. The gazetteer learns ISO country codes and IANA time zones. `source` tells seeded rows
--    from rows added on demand from a geocoder answer ('osm'), so the latter can be audited.
--    The one-row-per-name index from 0015 stays: discovery joins cities by name.
ALTER TABLE cities ADD COLUMN IF NOT EXISTS country_code TEXT;
ALTER TABLE cities ADD COLUMN IF NOT EXISTS timezone TEXT;
ALTER TABLE cities ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'seed';

UPDATE cities c SET country_code = v.cc, timezone = v.tz
FROM (VALUES
  ('Київ','UA','Europe/Kyiv'), ('Львів','UA','Europe/Kyiv'), ('Харків','UA','Europe/Kyiv'),
  ('Одеса','UA','Europe/Kyiv'), ('Дніпро','UA','Europe/Kyiv'), ('Запоріжжя','UA','Europe/Kyiv'),
  ('Вінниця','UA','Europe/Kyiv'), ('Івано-Франківськ','UA','Europe/Kyiv'), ('Тернопіль','UA','Europe/Kyiv'),
  ('Ужгород','UA','Europe/Kyiv'), ('Чернівці','UA','Europe/Kyiv'), ('Полтава','UA','Europe/Kyiv'),
  ('Житомир','UA','Europe/Kyiv'), ('Рівне','UA','Europe/Kyiv'), ('Луцьк','UA','Europe/Kyiv'),
  ('Хмельницький','UA','Europe/Kyiv'), ('Кропивницький','UA','Europe/Kyiv'), ('Суми','UA','Europe/Kyiv'),
  ('Чернігів','UA','Europe/Kyiv'), ('Черкаси','UA','Europe/Kyiv'), ('Кривий Ріг','UA','Europe/Kyiv'),
  ('Миколаїв','UA','Europe/Kyiv'),
  ('Warsaw','PL','Europe/Warsaw'), ('Krakow','PL','Europe/Warsaw'),
  ('Berlin','DE','Europe/Berlin'), ('Munich','DE','Europe/Berlin'),
  ('London','GB','Europe/London'), ('Manchester','GB','Europe/London'),
  ('Paris','FR','Europe/Paris'), ('Lyon','FR','Europe/Paris'),
  ('Amsterdam','NL','Europe/Amsterdam'), ('Brussels','BE','Europe/Brussels'),
  ('Madrid','ES','Europe/Madrid'), ('Barcelona','ES','Europe/Madrid'),
  ('Lisbon','PT','Europe/Lisbon'), ('Rome','IT','Europe/Rome'), ('Milan','IT','Europe/Rome'),
  ('Vienna','AT','Europe/Vienna'), ('Prague','CZ','Europe/Prague'), ('Budapest','HU','Europe/Budapest'),
  ('Zurich','CH','Europe/Zurich'), ('Stockholm','SE','Europe/Stockholm'),
  ('Copenhagen','DK','Europe/Copenhagen'), ('Oslo','NO','Europe/Oslo'),
  ('Helsinki','FI','Europe/Helsinki'), ('Dublin','IE','Europe/Dublin'), ('Athens','GR','Europe/Athens'),
  ('Istanbul','TR','Europe/Istanbul'), ('Bucharest','RO','Europe/Bucharest'), ('Sofia','BG','Europe/Sofia'),
  ('Vilnius','LT','Europe/Vilnius'), ('Riga','LV','Europe/Riga'), ('Tallinn','EE','Europe/Tallinn'),
  ('New York','US','America/New_York'), ('Los Angeles','US','America/Los_Angeles'),
  ('Chicago','US','America/Chicago'), ('San Francisco','US','America/Los_Angeles'),
  ('Austin','US','America/Chicago'), ('Miami','US','America/New_York'),
  ('Toronto','CA','America/Toronto'), ('Montreal','CA','America/Toronto'),
  ('Mexico City','MX','America/Mexico_City'), ('Sao Paulo','BR','America/Sao_Paulo'),
  ('Buenos Aires','AR','America/Argentina/Buenos_Aires'),
  ('Tokyo','JP','Asia/Tokyo'), ('Seoul','KR','Asia/Seoul'), ('Shanghai','CN','Asia/Shanghai'),
  ('Hong Kong','HK','Asia/Hong_Kong'), ('Singapore','SG','Asia/Singapore'), ('Bangkok','TH','Asia/Bangkok'),
  ('Mumbai','IN','Asia/Kolkata'), ('Delhi','IN','Asia/Kolkata'), ('Dubai','AE','Asia/Dubai'),
  ('Tel Aviv','IL','Asia/Jerusalem'), ('Cairo','EG','Africa/Cairo'),
  ('Cape Town','ZA','Africa/Johannesburg'), ('Lagos','NG','Africa/Lagos'),
  ('Sydney','AU','Australia/Sydney'), ('Melbourne','AU','Australia/Melbourne'),
  ('Auckland','NZ','Pacific/Auckland')
) AS v(name, cc, tz)
WHERE c.name = v.name AND c.timezone IS NULL;

-- 3. Events keep the ISO country code of the place the organiser picked.
ALTER TABLE events ADD COLUMN IF NOT EXISTS country_code TEXT;
