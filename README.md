# EmzLink

Удобная оболочка для zapret: Discord, YouTube и другие сайты, прокси для Telegram Desktop, подбор рабочего режима в один клик.

Основано на [Flowseal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube) и [bol-van/zapret](https://github.com/bol-van/zapret). Прокси Telegram: [Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy).

Канал: https://t.me/EmzProject

## Установка

1. Скачайте `EmzLink.zip` со страницы [Releases](../../releases/latest).
2. Распакуйте в папку без кириллицы и пробелов.
3. Запустите `EmzLink.exe` (нужны права администратора).

В главной папке лежат только приложение и документы. Файлы zapret (режимы, списки, драйвер) спрятаны в папке `zapret`.

## Сборка

Нужны [Rust](https://rustup.rs) и Windows. Запустите `source\Build-EmzLink.bat`: готовый `EmzLink.exe` появится в главной папке.

## Лицензия

См. `LICENSE.txt`. Файлы в `bin` и списки взяты из проектов, перечисленных выше.
