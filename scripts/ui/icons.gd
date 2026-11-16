extends RefCounted
## Íconos de línea propios (SVG dibujado a mano, 24 × 24), en tinta sepia.
## Se convierten en texturas al vuelo; no usan recursos de terceros.

const INK := "#3d2b1a"

const PATHS := {
	"road": "<path d='M8 2 L5 22 M16 2 L19 22'/><path d='M12 3v3 M12 9v3 M12 15v3 M12 20v2'/>",
	"curve": "<path d='M4 22 C4 10 10 4 22 4'/><path d='M9 22 C9 13 13 9 22 9' stroke-dasharray='2 2'/>",
	"grid": "<rect x='3' y='3' width='18' height='18'/><path d='M9 3v18 M15 3v18 M3 9h18 M3 15h18'/>",
	"upgrade": "<path d='M5 21 L19 21'/><path d='M7 21 L9 9 M17 21 L15 9'/><path d='M12 3 L8 8 M12 3 L16 8 M12 3 V14'/>",
	"zone": "<path d='M3 11 L12 4 L21 11'/><path d='M5 10 V20 H19 V10'/><rect x='10' y='14' width='4' height='6'/>",
	"dezone": "<path d='M3 11 L12 4 L21 11'/><path d='M5 10 V20 H19 V10'/><path d='M8 12 L16 19 M16 12 L8 19'/>",
	"demolish": "<path d='M4 20 H20'/><path d='M6 20 V13 H12 V20'/><path d='M14 4 L20 10 M17 7 L11 13'/>",
	"plaza": "<rect x='3' y='3' width='18' height='18'/><path d='M12 7 V17 M10 17 H14 M10.5 7 H13.5'/>",
	"parcels": "<path d='M3 6 L9 4 L15 6 L21 4 V18 L15 20 L9 18 L3 20 Z'/><path d='M9 4 V18 M15 6 V20'/>",
	"drop": "<path d='M12 3 C8 9 6 12 6 15 a6 6 0 0 0 12 0 C18 12 16 9 12 3 Z'/>",
	"well": "<path d='M5 13 H19 V20 H5 Z'/><path d='M7 13 V5 M17 13 V5 M6 5 H18'/><path d='M12 5 V10'/><rect x='10.5' y='10' width='3' height='3'/>",
	"cistern": "<rect x='6' y='10' width='12' height='10'/><path d='M8 10 V6 Q12 2 16 6 V10'/><path d='M4 20 H20'/>",
	"cart": "<rect x='3' y='8' width='13' height='7'/><circle cx='7' cy='18' r='2.5'/><circle cx='13' cy='18' r='2.5'/><path d='M16 11 H21'/>",
	"lantern": "<path d='M12 2 V5'/><path d='M8 5 H16 L15 15 H9 Z'/><path d='M10 15 V20 H14 V15'/><path d='M12 8 V12'/>",
	"cross": "<rect x='4' y='9' width='16' height='11'/><path d='M12 3 V9 M10 5 H14'/><path d='M12 12 V17 M9.5 14.5 H14.5'/>",
	"cow": "<path d='M4 10 H17 V17 H4 Z'/><path d='M17 11 L21 9 V14 L17 14'/><path d='M6 17 V21 M15 17 V21'/><path d='M18 9 L18 6'/>",
	"barrel": "<path d='M7 3 H17 Q20 12 17 21 H7 Q4 12 7 3 Z'/><path d='M5.5 8 H18.5 M5.5 16 H18.5'/>",
	"kiln": "<path d='M4 21 V12 Q12 4 20 12 V21 Z'/><path d='M10 21 V16 Q12 14 14 16 V21'/><path d='M16 7 V3 H18 V9'/>",
	"mill": "<circle cx='12' cy='13' r='7'/><circle cx='12' cy='13' r='2'/><path d='M12 6 V3 M3 21 L8 18 M21 21 L16 18'/>",
	"rocks": "<path d='M3 20 L7 12 L11 15 L14 9 L21 20 Z'/><path d='M14 9 L16 14'/>",
	"anchor": "<circle cx='12' cy='5' r='2'/><path d='M12 7 V20'/><path d='M5 14 Q6 20 12 20 Q18 20 19 14'/><path d='M8 10 H16'/>",
	"wall": "<path d='M3 21 V9 H6 V6 H9 V9 H12 V6 H15 V9 H18 V6 H21 V21 Z'/><path d='M10 21 V15 Q12 13 14 15 V21'/>",
	"scroll": "<path d='M6 4 H18 V18 Q18 21 15 21 H6'/><path d='M6 4 Q3 4 3 7 H6'/><path d='M9 9 H15 M9 13 H15 M9 17 H13'/>",
	"layers": "<path d='M12 3 L21 8 L12 13 L3 8 Z'/><path d='M3 12 L12 17 L21 12'/><path d='M3 16 L12 21 L21 16'/>",
	"eye": "<path d='M2 12 Q12 3 22 12 Q12 21 2 12 Z'/><circle cx='12' cy='12' r='3'/>",
	"coin": "<circle cx='12' cy='12' r='8'/><path d='M12 7 V17 M15 9.5 Q12 7.5 10 9 Q8.5 11 12 12 Q15.5 13 14 15 Q12 16.5 9 14.5'/>",
	"people": "<circle cx='9' cy='8' r='3'/><circle cx='17' cy='9' r='2.5'/><path d='M3 20 Q3 13 9 13 Q15 13 15 20'/><path d='M15 14 Q21 13 21 20'/>",
	"house": "<path d='M3 11 L12 4 L21 11'/><path d='M5 10 V20 H19 V10'/>",
	"brick": "<rect x='3' y='6' width='18' height='12'/><path d='M3 12 H21 M9 6 V12 M15 12 V18'/>",
	"bread": "<path d='M4 15 Q4 8 12 8 Q20 8 20 15 Z'/><path d='M8 11 L9 13 M12 10 L13 12 M16 11 L17 13'/>",
	"clock": "<circle cx='12' cy='12' r='9'/><path d='M12 7 V12 L15 14'/>",
	"wind": "<path d='M3 9 H15 Q19 9 19 6 Q19 3 16 4'/><path d='M3 14 H19 Q22 14 22 17 Q22 20 19 19'/>",
	"crown": "<path d='M4 18 L3 7 L8 11 L12 5 L16 11 L21 7 L20 18 Z'/>",
}

static var _cache := {}


static func texture(name: String, size := 28, color := INK) -> Texture2D:
	var key := "%s_%d_%s" % [name, size, color]
	if _cache.has(key):
		return _cache[key]
	var body: String = PATHS.get(name, PATHS["eye"])
	var svg := "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='%s' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'>%s</svg>" % [color, body]
	var img := Image.new()
	img.load_svg_from_string(svg, size / 24.0)
	var tex := ImageTexture.create_from_image(img)
	_cache[key] = tex
	return tex
