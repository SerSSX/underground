-- TASK-017 M2: built-in city gazetteer (worldwide) for geo-aware discovery.
-- No external geocoding: events/users resolve coordinates by matching this table.

CREATE TABLE cities (
    name    TEXT NOT NULL,
    country TEXT NOT NULL,
    lat     DOUBLE PRECISION NOT NULL,
    lng     DOUBLE PRECISION NOT NULL
);
CREATE UNIQUE INDEX cities_lower_name_idx ON cities (lower(name));

ALTER TABLE user_prefs ADD COLUMN country TEXT;

INSERT INTO cities (name, country, lat, lng) VALUES
  -- Ukraine
  ('Київ','Україна',50.4501,30.5234),
  ('Львів','Україна',49.8397,24.0297),
  ('Харків','Україна',49.9935,36.2304),
  ('Одеса','Україна',46.4825,30.7233),
  ('Дніпро','Україна',48.4647,35.0462),
  ('Запоріжжя','Україна',47.8388,35.1396),
  ('Вінниця','Україна',49.2331,28.4682),
  ('Івано-Франківськ','Україна',48.9226,24.7111),
  ('Тернопіль','Україна',49.5535,25.5948),
  ('Ужгород','Україна',48.6208,22.2879),
  ('Чернівці','Україна',48.2917,25.9354),
  ('Полтава','Україна',49.5883,34.5514),
  -- Europe
  ('Warsaw','Poland',52.2297,21.0122),
  ('Krakow','Poland',50.0647,19.9450),
  ('Berlin','Germany',52.5200,13.4050),
  ('Munich','Germany',48.1351,11.5820),
  ('London','United Kingdom',51.5074,-0.1278),
  ('Manchester','United Kingdom',53.4808,-2.2426),
  ('Paris','France',48.8566,2.3522),
  ('Lyon','France',45.7640,4.8357),
  ('Amsterdam','Netherlands',52.3676,4.9041),
  ('Brussels','Belgium',50.8503,4.3517),
  ('Madrid','Spain',40.4168,-3.7038),
  ('Barcelona','Spain',41.3851,2.1734),
  ('Lisbon','Portugal',38.7223,-9.1393),
  ('Rome','Italy',41.9028,12.4964),
  ('Milan','Italy',45.4642,9.1900),
  ('Vienna','Austria',48.2082,16.3738),
  ('Prague','Czechia',50.0755,14.4378),
  ('Budapest','Hungary',47.4979,19.0402),
  ('Zurich','Switzerland',47.3769,8.5417),
  ('Stockholm','Sweden',59.3293,18.0686),
  ('Copenhagen','Denmark',55.6761,12.5683),
  ('Oslo','Norway',59.9139,10.7522),
  ('Helsinki','Finland',60.1699,24.9384),
  ('Dublin','Ireland',53.3498,-6.2603),
  ('Athens','Greece',37.9838,23.7275),
  ('Istanbul','Turkey',41.0082,28.9784),
  ('Bucharest','Romania',44.4268,26.1025),
  ('Sofia','Bulgaria',42.6977,23.3219),
  ('Vilnius','Lithuania',54.6872,25.2797),
  ('Riga','Latvia',56.9496,24.1052),
  ('Tallinn','Estonia',59.4370,24.7536),
  -- Americas
  ('New York','USA',40.7128,-74.0060),
  ('Los Angeles','USA',34.0522,-118.2437),
  ('Chicago','USA',41.8781,-87.6298),
  ('San Francisco','USA',37.7749,-122.4194),
  ('Austin','USA',30.2672,-97.7431),
  ('Miami','USA',25.7617,-80.1918),
  ('Toronto','Canada',43.6532,-79.3832),
  ('Montreal','Canada',45.5019,-73.5674),
  ('Mexico City','Mexico',19.4326,-99.1332),
  ('Sao Paulo','Brazil',-23.5558,-46.6396),
  ('Buenos Aires','Argentina',-34.6037,-58.3816),
  -- Asia / Middle East
  ('Tokyo','Japan',35.6762,139.6503),
  ('Seoul','South Korea',37.5665,126.9780),
  ('Shanghai','China',31.2304,121.4737),
  ('Hong Kong','China',22.3193,114.1694),
  ('Singapore','Singapore',1.3521,103.8198),
  ('Bangkok','Thailand',13.7563,100.5018),
  ('Mumbai','India',19.0760,72.8777),
  ('Delhi','India',28.6139,77.2090),
  ('Dubai','UAE',25.2048,55.2708),
  ('Tel Aviv','Israel',32.0853,34.7818),
  -- Africa / Oceania
  ('Cairo','Egypt',30.0444,31.2357),
  ('Cape Town','South Africa',-33.9249,18.4241),
  ('Lagos','Nigeria',6.5244,3.3792),
  ('Sydney','Australia',-33.8688,151.2093),
  ('Melbourne','Australia',-37.8136,144.9631),
  ('Auckland','New Zealand',-36.8485,174.7633);
