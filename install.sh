#!/bin/bash

set -e  # Выход при любой ошибке

cargo build --release

echo "Установка sort_pictures сервиса..."

# Создание необходимых директорий
echo "Создание директорий..."
mkdir -p ~/apps/bin/
mkdir -p ~/.config/systemd/user/
mkdir -p ~/.config/sort_pictures/

# Остановка сервиса если запущен
if systemctl --user is-active --quiet sort_pictures.service; then
    echo "Остановка запущенного сервиса..."
    systemctl --user stop sort_pictures.service
fi

# Копирование файлов
echo "Копирование файлов..."
cp $(pwd)/target/release/sort_pictures ~/apps/bin/
cp $(pwd)/systemd/sort_pictures.service ~/.config/systemd/user/sort_pictures.service
cp $(pwd)/systemd/config.toml ~/.config/sort_pictures/config.toml

# Установка прав на выполнение
echo "Установка прав..."
chmod +x ~/apps/bin/sort_pictures

# Перезагрузка systemd
echo "Перезагрузка systemd..."
systemctl --user daemon-reload

# Включение сервиса
echo "Включение сервиса..."
systemctl --user enable sort_pictures.service

# Запуск сервиса
echo "Запуск сервиса..."
systemctl --user start sort_pictures.service

# Проверка статуса
echo "Статус сервиса:"
systemctl --user status sort_pictures.service --no-pager -l

echo ""
echo "Установка завершена!"
echo ""
echo "Полезные команды:"
echo "  systemctl --user status sort_pictures    # статус"
echo "  systemctl --user stop sort_pictures      # остановить"
echo "  systemctl --user restart sort_pictures   # перезапустить"
echo "  journalctl --user -u sort_pictures -f    # логи"
