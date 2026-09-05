-- Ten more Ukrainian oblast centres for the gazetteer.
--
-- Migration 0015 seeded 12 Ukrainian cities (Київ, Львів, Одеса, Харків, Дніпро, Запоріжжя,
-- Вінниця, Полтава, Тернопіль, Ужгород, Чернівці, Івано-Франківськ). The ones below were
-- missing, and a missing city is not a visible error: `cities::resolve` returns all-NULL, so
-- the event is stored with no country and no coordinates and then quietly drops out of the
-- distance boost, the `near` reason and the same-country boost in /api/discover. It also
-- never appears in the `GET /api/cities?q=` autocomplete behind every city picker.
--
-- `country` MUST be the Ukrainian spelling: `currency.rs` maps country → currency on the
-- literal string, and 'Ukraine' is not one of its keys — a city added under the English name
-- geocodes fine and then prices its events in the wrong default currency.
INSERT INTO cities (name, country, lat, lng) VALUES
  ('Житомир',       'Україна', 50.2547, 28.6587),
  ('Рівне',         'Україна', 50.6199, 26.2516),
  ('Луцьк',         'Україна', 50.7472, 25.3254),
  ('Хмельницький',  'Україна', 49.4229, 26.9871),
  ('Кропивницький', 'Україна', 48.5079, 32.2623),
  ('Суми',          'Україна', 50.9077, 34.7981),
  ('Чернігів',      'Україна', 51.4982, 31.2893),
  ('Черкаси',       'Україна', 49.4444, 32.0598),
  ('Кривий Ріг',    'Україна', 47.9105, 33.3918),
  ('Миколаїв',      'Україна', 46.9750, 31.9946)
ON CONFLICT (lower(name)) DO NOTHING;
