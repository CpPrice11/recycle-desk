# Recycle Desk: іконка застосунку

2026-10-24. Створено вбудованим `imagegen`, без API-ключа та без запозичених активів Minibin або Microsoft. Це растрова іконка; векторного оригіналу немає.

- `recycle-desk-v1.png`: незмінений PNG від imagegen, 1254×1254, прозорий фон.
- `recycle-desk-v1.ico`: Windows ICO із PNG-кадрами 16, 20, 24, 32, 40, 48, 64, 128 і 256 px.
- `recycle-desk-v1-preview.png`: перевірочна композиція на фонах Catppuccin Mocha/Latte; внизу фактичні малі розміри.

Відтворення ICO: `./scripts/pack-icon.ps1`. Скрипт масштабує та пакує зображення, не ретушує його. Альфа-канал збережено. `resources/app.rc` посилається на новий ICO; попередня геометрична заготовка `resources/app.ico` збережена.

QC: оглянуто силует, геометрію ручки/кришки, цілісність стрілки, світло, краї прозорості та читабельність 16–128 px на світлому й темному фонах. Критичних або помітних дефектів не виявлено; ремонтних генерацій не виконували. Растрові кольори відтворюють напрям Catppuccin, точну відповідність кожного пікселя палітрі не заявлено.

Перевірка Windows LoadImageW: усі дев’ять розмірів, включно з 256×256, завантажилися з правильними розмірами. Компіляція Windows-ресурсу пройшла.

Перевірочна release-збірка: `target/icon-preview/recycle-desk.exe`. Попередній `target/release/recycle-desk.exe` не перезаписано: Windows повернула Access denied. Окрема збірка потрібна лише для перевірки вбудованого ресурсу; це не перевірений готовий реліз застосунку.

Іконку підключено до EXE та системного заголовка через ресурс 1. Динамічні порожній/повний значки кошика в треї зберігають свої системні ресурси.

## Фінальний промпт

```text
Use case: logo-brand.
Asset type: an original production Windows application icon for a lightweight recycle-bin manager, Recycle Desk. This is one app icon, not a presentation or icon sheet.
Primary request: a distinctive, exceptionally simple sculpted wastebasket with a compact closed lid and a bold single counterclockwise restore arrow inset into its front face. The wastebasket silhouette must be unmistakable even at 16 and 32 pixels. Use a slightly tapered rounded-square body, a thick simple lid, and a broad integrated top handle. Near-frontal view with only a little of the top and right side visible. The white restore arrow is one coherent thick shape with one clearly formed arrowhead. No recycling triangle.
Style/medium: polished restrained 3D desktop icon with soft bevels, matte enamel, clean broad surfaces, strong silhouette and very subtle shading; no tiny details. Friendly but precise utility software.
Composition/framing: one centered isolated object, square image, object fills approximately 82% of height, full silhouette entirely inside canvas, visually balanced. No square app tile or enclosing badge.
Color palette: match the existing project's Catppuccin direction: periwinkle blue body (#89b4fa), a little lavender (#b4befe) on the lid, deeper blue side shading, near-white front arrow. Strong edge contrast for light and dark desktops.
Scene/backdrop: genuinely transparent alpha background. No floor, no backdrop, no cast shadow outside the object, no glow or halo.
Constraints: no text, no letters, no numbers, no watermark, no extra objects. Original design; do not reproduce Microsoft Windows or Minibin icon artwork.
```

