#!/usr/bin/env python3
"""
Сборщик эталонных данных Земли для PlanetSim:
1. Реальный скан батиметрии океана (GEBCO) и топографии суши (NASA SRTM) с Wikimedia Commons / CDN.
2. Реальные границы 14 тектонических плит Peter Bird (PB2002).

Необходимые библиотеки:
    pip install numpy pillow requests urllib3
"""

import io
import json
import os
import struct
import sys

# Явная проверка зависимостей без скрытых фоллбэков
try:
    import numpy as np
    import requests
    import urllib3
    from PIL import Image, ImageDraw
except ImportError as err:
    print(f"❌ Ошибка: не установлена библиотека ({err}).")
    print("Выполните команду:\n    pip install numpy pillow requests urllib3")
    sys.exit(1)

urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

OUT_DIR = "assets/earth"
WIDTH = 3600   # 0.1 градуса по долготе (-180 .. +180)
HEIGHT = 1800  # 0.1 градуса по широте (+90 .. -90)

# Wikimedia Commons API для получения прямой ссылки на скан NASA Topography + GEBCO Bathymetry (3840x1920)
WIKIMEDIA_API_URL = (
    "https://commons.wikimedia.org/w/api.php"
    "?action=query&titles=File:World_elevation_map.png"
    "&prop=imageinfo&iiprop=url&iiurlwidth=3840&format=json"
)

# Прямые ссылки на скан GEBCO + NASA (3840x1920) на Wikimedia
GEBCO_NASA_SCAN_URLS = [
    "https://upload.wikimedia.org/wikipedia/commons/thumb/c/c3/World_elevation_map.png/3840px-World_elevation_map.png",
]

# Точная маска береговой линии и высот суши NASA SRTM
TOPO_URLS = [
    "https://cdn.jsdelivr.net/npm/three-globe/example/img/earth-topology.png",
    "https://unpkg.com/three-globe/example/img/earth-topology.png",
    "https://raw.githubusercontent.com/vasturiano/three-globe/master/example/img/earth-topology.png",
]

WATER_URLS = [
    "https://cdn.jsdelivr.net/npm/three-globe/example/img/earth-water.png",
    "https://unpkg.com/three-globe/example/img/earth-water.png",
    "https://raw.githubusercontent.com/vasturiano/three-globe/master/example/img/earth-water.png",
]

PB2002_PLATES_URLS = [
    "https://cdn.jsdelivr.net/gh/fraxen/tectonicplates@master/GeoJSON/PB2002_plates.json",
    "https://raw.githubusercontent.com/fraxen/tectonicplates/master/GeoJSON/PB2002_plates.json",
]

PLATE_CODE_MAP = {
    "PA": 0,
    "EU": 1, "AM": 1, "OK": 1, "YA": 1, "ti": 1, "AT": 1, "AE": 1, "AS": 1, "SU": 1, "MS": 1, "BS": 1,
    "NA": 2, "GL": 2,
    "SA": 3, "ND": 3, "AP": 3,
    "AF": 4, "SO": 4, "VL": 4, "LW": 4, "MD": 4,
    "AU": 5, "WL": 5, "BH": 5, "CL": 5, "CR": 5, "KE": 5, "MA": 5, "MO": 5,
    "NB": 5, "NH": 5, "NI": 5, "SB": 5, "SL": 5, "SS": 5, "TIM": 5, "TO": 5,
    "IN": 6, "BU": 6,
    "AN": 7, "SH": 7,
    "NZ": 8, "EA": 8, "GP": 8, "JZ": 8,
    "CO": 9, "RI": 9, "JF": 9, "PM": 9,
    "CA": 10,
    "AR": 11,
    "PH": 12, "ON": 12, "PS": 12,
    "SC": 13, "SW": 13,
}

HEADERS = {
    "User-Agent": "PlanetSimBenchmark/1.0 (https://github.com/planetSim; contact@planetsim.local) python-requests"
}


def fetch_with_mirrors(urls: list[str], cache_path: str) -> bytes:
    if os.path.exists(cache_path) and os.path.getsize(cache_path) > 1024:
        with open(cache_path, "rb") as f:
            return f.read()

    last_err = None
    for url in urls:
        for verify_ssl in (True, False):
            try:
                mode_str = "SSL" if verify_ssl else "no-SSL"
                print(f"   -> Скачивание {url} ({mode_str})...")
                resp = requests.get(url, headers=HEADERS, timeout=45, verify=verify_ssl)
                resp.raise_for_status()
                with open(cache_path, "wb") as f:
                    f.write(resp.content)
                return resp.content
            except Exception as e:
                last_err = e
    raise RuntimeError(f"Не удалось скачать {cache_path}: {last_err}")


def fetch_gebco_nasa_scan() -> bytes:
    cache_path = os.path.join(OUT_DIR, "gebco_nasa_world_elevation.png")
    if os.path.exists(cache_path) and os.path.getsize(cache_path) > 100_000:
        with open(cache_path, "rb") as f:
            return f.read()

    urls = list(GEBCO_NASA_SCAN_URLS)
    # Дополнительно запрашиваем актуальный URL превью через API Wikimedia Commons
    try:
        r = requests.get(WIKIMEDIA_API_URL, headers=HEADERS, timeout=20)
        if r.ok:
            pages = r.json().get("query", {}).get("pages", {})
            for page in pages.values():
                info = page.get("imageinfo", [{}])[0]
                thumb_url = info.get("thumburl") or info.get("url")
                if thumb_url and thumb_url not in urls:
                    urls.insert(0, thumb_url)
    except Exception:
        pass

    return fetch_with_mirrors(urls, cache_path)


def rasterize_plates() -> np.ndarray:
    print("1/3 Скачивание и растеризация тектонических плит Peter Bird (PB2002)...")
    raw_plates = fetch_with_mirrors(PB2002_PLATES_URLS, os.path.join(OUT_DIR, "PB2002_plates.json"))
    plates_json = json.loads(raw_plates.decode("utf-8"))

    plate_img = Image.new("L", (WIDTH, HEIGHT), color=0)
    draw_p = ImageDraw.Draw(plate_img)

    for feature in plates_json.get("features", []):
        code = feature.get("properties", {}).get("Code", "PA")
        plate_id = PLATE_CODE_MAP.get(code, 1)
        geom = feature.get("geometry", {})
        coords = geom.get("coordinates", [])
        polygons = coords if geom.get("type") == "MultiPolygon" else [coords]
        for poly in polygons:
            if not poly:
                continue
            pts = [
                ((pt[0] + 180.0) / 360.0 * WIDTH, (90.0 - pt[1]) / 180.0 * HEIGHT)
                for pt in poly[0]
            ]
            if len(pts) >= 3:
                draw_p.polygon(pts, fill=plate_id)

    plates = np.array(plate_img, dtype=np.uint8)
    print(f"   Уникальных плит в растре: {len(np.unique(plates))}")
    return plates


def build_scanned_elevations() -> np.ndarray:
    print("2/3 Скачивание реального скана батиметрии GEBCO и рельефа суши NASA SRTM...")
    scan_bytes = fetch_gebco_nasa_scan()
    topo_bytes = fetch_with_mirrors(TOPO_URLS, os.path.join(OUT_DIR, "earth-topology.png"))
    water_bytes = fetch_with_mirrors(WATER_URLS, os.path.join(OUT_DIR, "earth-water.png"))

    # 1. Полный скан GEBCO + NASA (в нём уровень моря находится на яркости 0.5608 = 143/255,
    #    всё что ниже 0.5608 — реальная батиметрия океана GEBCO от -10900 м до 0 м)
    scan_img = (
        Image.open(io.BytesIO(scan_bytes))
        .convert("L")
        .resize((WIDTH, HEIGHT), Image.Resampling.BILINEAR)
    )
    scan_norm = np.array(scan_img, dtype=np.float32) / 255.0

    # 2. Чистый рельеф суши и маска береговой линии
    topo_img = (
        Image.open(io.BytesIO(topo_bytes))
        .convert("L")
        .resize((WIDTH, HEIGHT), Image.Resampling.BILINEAR)
    )
    water_img = (
        Image.open(io.BytesIO(water_bytes))
        .convert("L")
        .resize((WIDTH, HEIGHT), Image.Resampling.BILINEAR)
    )

    topo_norm = np.array(topo_img, dtype=np.float32) / 255.0
    water_mask = np.array(water_img, dtype=np.float32) / 255.0
    is_ocean = water_mask > 0.50

    # Декодируем реальную глубину океана из скана GEBCO:
    # В World_elevation_map.png базовый уровень моря = 0.56 (143/255), батиметрия наложена через Multiply:
    # scan_norm = 0.56 -> 0 м (шельф у берега), scan_norm = 0.0 -> -10900 м (Марианская впадина)
    sea_level_base = 0.5608
    depth_ratio = np.clip((sea_level_base - scan_norm) / sea_level_base, 0.0, 1.0)
    ocean_meters = -15.0 - (depth_ratio ** 1.12) * 10885.0

    # Декодируем реальную высоту суши из скана NASA SRTM (с подстраховкой из World_elevation_map для Антарктиды)
    scan_land_ratio = np.clip((scan_norm - sea_level_base) / (1.0 - sea_level_base), 0.0, 1.0)
    combined_land_norm = np.maximum(topo_norm, scan_land_ratio * 0.85)
    land_meters = 5.0 + (combined_land_norm ** 1.30) * 8843.0

    elev = np.where(is_ocean, ocean_meters, land_meters).astype(np.int16)
    print(
        f"   Скан готов: океан min={elev[is_ocean].min()} м, макс суша={elev[~is_ocean].max()} м"
    )
    return elev


def save_binary_and_previews(elev: np.ndarray, plates: np.ndarray):
    print("3/3 Упаковка скана в assets/earth/earth_data.bin...")
    bin_path = os.path.join(OUT_DIR, "earth_data.bin")
    with open(bin_path, "wb") as f:
        f.write(b"EAR1")
        f.write(struct.pack("<II", WIDTH, HEIGHT))
        f.write(elev.astype("<i2").tobytes())
        f.write(plates.astype("u1").tobytes())

    norm_elev = ((elev.astype(np.float32) + 11000.0) / (11000.0 + 8848.0) * 255.0).astype(np.uint8)
    Image.fromarray(norm_elev, mode="L").save(os.path.join(OUT_DIR, "preview_elevation.png"))
    Image.fromarray((plates * 18).astype(np.uint8), mode="L").save(
        os.path.join(OUT_DIR, "preview_plates.png")
    )
    size_mb = os.path.getsize(bin_path) / (1024 * 1024)
    print(f"✅ Готово! {bin_path} ({size_mb:.1f} МБ) записан. Проверьте preview_elevation.png!")


if __name__ == "__main__":
    os.makedirs(OUT_DIR, exist_ok=True)
    plates_grid = rasterize_plates()
    elev_grid = build_scanned_elevations()
    save_binary_and_previews(elev_grid, plates_grid)