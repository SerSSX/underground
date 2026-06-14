-- Dev content seed (idempotent via fixed UUIDs + ON CONFLICT). Safe to re-run.
-- Diverse categories across cities + a few free events.
INSERT INTO events
  (id, title, description, poster_url, category, city, venue_address, starts_at, ends_at,
   age_rating, price, currency, tickets_total, tickets_left, cost_type, status)
VALUES
 ('11111111-0000-0000-0000-000000000001', 'LOUNA — 10 років гурту',
  'Ювілейний концерт LOUNA: преміумний звук, відеоінсталяції та фірмова енергетика. Усі хіти за 10 років на одній сцені.',
  'https://picsum.photos/seed/louna/800/1000', 'Rock', 'Київ', 'Atlas, вул. Січових Стрільців 37/41',
  now() + interval '20 day', now() + interval '20 day' + interval '4 hour', '16+', 650.00, 'UAH', 754, 173, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000002', 'METALLICA Show — S&M Tribute',
  'Симфонічний метал-шоу: трибют Metallica із живим оркестром. 20 років легендарного S&M.',
  'https://picsum.photos/seed/metallica/800/1000', 'Metal', 'Київ', 'Палац Україна',
  now() + interval '35 day', now() + interval '35 day' + interval '3 hour', '12+', 350.00, 'UAH', 1135, 465, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000003', 'ТАРТАК — Далі буде',
  'Новий тур легендарного гурту ТАРТАК. Драйв, гумор і улюблені треки наживо.',
  'https://picsum.photos/seed/tartak/800/1000', 'Rock', 'Київ', 'Docker Pub, вул. Полтавська 5',
  now() + interval '8 day', now() + interval '8 day' + interval '3 hour', '18+', 275.00, 'UAH', 300, 93, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000004', 'STAND UP: ТОПовий гумор',
  'Найкращі стендап-коміки країни в одному вечорі. Гострі жарти та повний зал сміху.',
  'https://picsum.photos/seed/standup/800/1000', 'Stand-Up', 'Київ', 'вул. Кирилівська 134',
  now() + interval '12 day', now() + interval '12 day' + interval '2 hour', '18+', 750.00, 'UAH', 365, 32, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000005', 'ALYOSHA — Live',
  'Сольний концерт ALYOSHA: нові пісні й улюблені хіти у великому live-форматі.',
  'https://picsum.photos/seed/alyosha/800/1000', 'Pop', 'Київ', 'Stereo Plaza',
  now() + interval '26 day', now() + interval '26 day' + interval '2 hour', '6+', 590.00, 'UAH', 900, 540, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000006', 'LP — Live in Kyiv',
  'Світова зірка LP уперше з новою програмою. Потужний вокал і незабутня атмосфера.',
  'https://picsum.photos/seed/lp/800/1000', 'Pop', 'Київ', 'Палац спорту',
  now() + interval '48 day', now() + interval '48 day' + interval '3 hour', '12+', 1200.00, 'UAH', 4000, 2100, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000007', 'PRODIGY Tribute Night',
  'Електронний рейв-трибют The Prodigy. Світло, бас і танці до ранку.',
  'https://picsum.photos/seed/prodigy/800/1000', 'Electronic', 'Київ', 'Closer, вул. Нижньоюрківська 31',
  now() + interval '5 day', now() + interval '5 day' + interval '6 hour', '18+', 450.00, 'UAH', 600, 410, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000008', 'MOZART — Symphony Gala',
  'Вечір класики: симфонічний оркестр виконує найвідоміші твори Моцарта у Львівській філармонії.',
  'https://picsum.photos/seed/mozart/800/1000', 'Classical', 'Львів', 'Львівська філармонія',
  now() + interval '18 day', now() + interval '18 day' + interval '2 hour', '0+', 400.00, 'UAH', 500, 220, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000009', 'RETRO WAVE PARTY',
  'Вечірка у стилі synthwave: неон, ретро-хіти 80-х та діджей-сети до ранку.',
  'https://picsum.photos/seed/retrowave/800/1000', 'Party', 'Київ', 'вул. Глушкова 42',
  now() + interval '15 day', now() + interval '15 day' + interval '7 hour', '18+', 100.00, 'UAH', 250, 50, 'paid', 'published'),

 ('11111111-0000-0000-0000-00000000000a', 'MEDIA COURSE via Media',
  'Прокачайте майстерність: говоримо з журналістами. Практичний медіа-курс із топовими спікерами.',
  'https://picsum.photos/seed/media/800/1000', 'Course', 'Київ', 'KWINS Hall, вул. Антоновича 40',
  now() + interval '10 day', now() + interval '10 day' + interval '5 hour', '0+', 1800.00, 'UAH', 80, 72, 'paid', 'published'),

 ('11111111-0000-0000-0000-00000000000b', 'БІЛІ НОЧІ — Eureka Fest',
  'Літній музичний фестиваль просто неба: кілька сцен, фудкорт і хедлайнери з усієї країни.',
  'https://picsum.photos/seed/eureka/800/1000', 'Festival', 'Одеса', 'Аркадія, відкритий майданчик',
  now() + interval '40 day', now() + interval '40 day' + interval '10 hour', '16+', 850.00, 'UAH', 5000, 3800, 'paid', 'published'),

 ('11111111-0000-0000-0000-00000000000c', 'BOOM BAP — Rap Battle',
  'Андеграунд реп-батл: біти, фрістайл і нові імена сцени. Голосує зал.',
  'https://picsum.photos/seed/boombap/800/1000', 'Hip-Hop', 'Харків', 'Bar Hider, вул. Сумська 23',
  now() + interval '9 day', now() + interval '9 day' + interval '4 hour', '18+', 200.00, 'UAH', 220, 140, 'paid', 'published'),

 ('11111111-0000-0000-0000-00000000000d', 'Jazz у підвалі',
  'Інтимний джазовий вечір при свічках: тріо наживо, келих вина та імпровізації.',
  'https://picsum.photos/seed/jazz/800/1000', 'Jazz', 'Львів', 'Підвал на Вірменській 12',
  now() + interval '6 day', now() + interval '6 day' + interval '3 hour', '16+', 320.00, 'UAH', 60, 18, 'paid', 'published'),

 ('11111111-0000-0000-0000-00000000000e', 'Поетичний відкритий мікрофон',
  'Читай своє або слухай інших. Вільний вхід, тепла атмосфера й чай. Андеграунд-поезія без цензури.',
  'https://picsum.photos/seed/poetry/800/1000', 'Poetry', 'Київ', 'Книгарня-кав''ярня, вул. Хрещатик 44',
  now() + interval '4 day', now() + interval '4 day' + interval '2 hour', '12+', 0.00, 'UAH', 50, 27, 'free', 'published'),

 ('11111111-0000-0000-0000-00000000000f', 'Виставка «Андеграунд-арт»',
  'Молоді художники показують стрит-арт, графіку та інсталяції. Зустріч з авторами.',
  'https://picsum.photos/seed/expo/800/1000', 'Exhibition', 'Київ', 'Арт-простір IZONE, вул. Набережно-Лугова 8',
  now() + interval '14 day', now() + interval '14 day' + interval '8 hour', '0+', 150.00, 'UAH', 400, 260, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000010', 'Фото-воркшоп: вулична зйомка',
  'Практичний воркшоп зі стрит-фотографії: світло, композиція, зйомка в місті та розбір кадрів.',
  'https://picsum.photos/seed/photo/800/1000', 'Workshop', 'Дніпро', 'Студія LightRoom, вул. Шевченка 5',
  now() + interval '11 day', now() + interval '11 day' + interval '6 hour', '14+', 500.00, 'UAH', 25, 9, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000011', 'Startup Pitch Night',
  'Вечір пітчів: засновники презентують ідеї, фідбек від менторів та нетворкінг. Вхід вільний.',
  'https://picsum.photos/seed/startup/800/1000', 'Startup', 'Київ', 'UNIT.City, вул. Симиренка 2',
  now() + interval '7 day', now() + interval '7 day' + interval '4 hour', '0+', 0.00, 'UAH', 150, 88, 'free', 'published'),

 ('11111111-0000-0000-0000-000000000012', 'Йога на світанку',
  'Ранкова практика просто неба біля моря: дихання, асани та медитація на сході сонця.',
  'https://picsum.photos/seed/yoga/800/1000', 'Yoga', 'Одеса', 'Ланжерон, пляж',
  now() + interval '3 day', now() + interval '3 day' + interval '2 hour', '0+', 250.00, 'UAH', 40, 22, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000013', 'Трейл-забіг «Карпати»',
  'Гірський трейл на 12 км стежками Карпат: стартовий пакет, медаль і фото з траси.',
  'https://picsum.photos/seed/trail/800/1000', 'Run', 'Львів', 'Славське, база відпочинку',
  now() + interval '30 day', now() + interval '30 day' + interval '5 hour', '16+', 400.00, 'UAH', 500, 310, 'paid', 'published'),

 ('11111111-0000-0000-0000-000000000014', 'Настолки до ранку',
  'Вечір настільних ігор: понад 100 ігор, тематичні турніри та новачкам завжди раді.',
  'https://picsum.photos/seed/boardgames/800/1000', 'Board Games', 'Київ', 'Anticafe Time, вул. Велика Васильківська 5',
  now() + interval '2 day', now() + interval '2 day' + interval '6 hour', '12+', 120.00, 'UAH', 60, 41, 'paid', 'published')
ON CONFLICT (id) DO NOTHING;
