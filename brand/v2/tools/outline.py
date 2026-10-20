import sys, json
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
def run(text, wght, wdth, tracking=0.0, src=__import__('os').path.join(__import__('os').path.dirname(__file__), '..', 'fonts', 'Archivo-wdth-wght.ttf')):
    f = TTFont(src)
    inst = instantiateVariableFont(f, {'wght': wght, 'wdth': wdth})
    gs = inst.getGlyphSet(); cmap = inst.getBestCmap(); upm = inst['head'].unitsPerEm
    hmtx = inst['hmtx']
    x = 0; parts = []
    for ch in text:
        g = cmap[ord(ch)]
        pen = SVGPathPen(gs)
        tp = TransformPen(pen, (1/upm*100, 0, 0, -1/upm*100, x, 0))
        gs[g].draw(tp)
        parts.append(pen.getCommands())
        x += hmtx[g][0]/upm*100 + tracking
    return {'d': ' '.join(parts), 'advance': x - tracking}
spec = json.loads(sys.argv[1])
print(json.dumps({k: run(**v) for k, v in spec.items()}))
