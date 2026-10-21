#!/usr/bin/env python3
"""Référencement du site www.kybernow.app (voir site/README.md, « Référencement »).

    scripts/site-seo.py            # tout
    scripts/site-seo.py images     # images de partage et icône iPhone/iPad, rendues par Chrome sans fenêtre
    scripts/site-seo.py faq        # données structurées FAQPage des guides, tirées des <details> de leur FAQ

Images : scripts/site-og/card.html (?lang=fr|en) -> site/assets/og/kybernow-{fr,en}.jpg (1200 × 630),
scripts/site-og/touch-icon.html -> site/apple-touch-icon.png (180 × 180). Il faut Google Chrome et Pillow.
FAQ : réécrit le bloc <script type="application/ld+json" data-faq> de site/guide/ et site/en/guide/
à partir des questions visibles, pour que les deux ne divergent jamais. À relancer après toute
modification de la FAQ.
"""
import html, io, json, os, re, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / 'site'
SRC = ROOT / 'scripts' / 'site-og'
CHROME = os.environ.get('CHROME', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')


def shot(page: Path, query: str, size: tuple[int, int]):
    from PIL import Image
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / 'shot.png'
        subprocess.run([CHROME, '--headless=new', '--disable-gpu', '--hide-scrollbars',
                        '--force-device-scale-factor=1', '--allow-file-access-from-files',
                        '--virtual-time-budget=3000', f'--user-data-dir={tmp}/profile',
                        f'--window-size={size[0]},{size[1]}', f'--screenshot={out}',
                        page.as_uri() + query], check=True, capture_output=True)
        im = Image.open(out).convert('RGB')
        im.load()
    if im.size != size:
        sys.exit(f'{page.name}{query} : {im.size} au lieu de {size}')
    return im


def images():
    og = SITE / 'assets' / 'og'
    og.mkdir(parents=True, exist_ok=True)
    for lang in ('fr', 'en'):
        dst = og / f'kybernow-{lang}.jpg'
        shot(SRC / 'card.html', f'?lang={lang}', (1200, 630)).save(dst, 'JPEG', quality=90, optimize=True, progressive=True)
        print(f'{dst.relative_to(ROOT)} : {dst.stat().st_size // 1024} Ko')
    dst = SITE / 'apple-touch-icon.png'
    shot(SRC / 'touch-icon.html', '', (180, 180)).save(dst, 'PNG', optimize=True)
    print(f'{dst.relative_to(ROOT)} : {dst.stat().st_size // 1024} Ko')


def text(fragment: str) -> str:
    s = re.sub(r'<[^>]+>', '', fragment)
    s = html.unescape(s)
    return re.sub(r'\s+', ' ', s).strip()


def faq():
    for page, lang in ((SITE / 'guide' / 'index.html', 'fr'), (SITE / 'en' / 'guide' / 'index.html', 'en')):
        s = page.read_text(encoding='utf-8')
        block = re.search(r'<div class="faq">(.*?)</div>', s, re.S)
        if not block:
            sys.exit(f'{page} : FAQ introuvable')
        items = re.findall(r'<details><summary>(.*?)</summary>(.*?)</details>', block.group(1), re.S)
        data = {
            '@context': 'https://schema.org',
            '@type': 'FAQPage',
            'inLanguage': lang,
            'mainEntity': [{'@type': 'Question', 'name': text(q),
                            'acceptedAnswer': {'@type': 'Answer', 'text': text(a)}} for q, a in items],
        }
        body = json.dumps(data, ensure_ascii=False, indent=1)
        new, n = re.subn(r'(<script type="application/ld\+json" data-faq>).*?(</script>)',
                         lambda m: m.group(1) + '\n' + body + '\n' + m.group(2), s, flags=re.S)
        if n != 1:
            sys.exit(f'{page} : bloc <script type="application/ld+json" data-faq> absent')
        page.write_text(new, encoding='utf-8')
        print(f'{page.relative_to(ROOT)} : {len(items)} questions')


if __name__ == '__main__':
    what = sys.argv[1:] or ['images', 'faq']
    for w in what:
        {'images': images, 'faq': faq}[w]()
